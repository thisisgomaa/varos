//! The in-app object clipboard — Edit ▸ Cut / Copy / Paste / Paste in Place (Astra F04).
//!
//! **In-app only.** This holds deep copies of `varos-core` model data inside the running `Editor`;
//! it is NOT the operating-system clipboard. Copying between Varos windows or to/from other apps
//! (Illustrator, Figma, a text editor) needs a decision on which flavours to publish (PDF / SVG /
//! Varos JSON) — that is a later, separate piece. Nothing here may be read as a promise of that.
//!
//! The clipboard is never part of undo history and never serialized: copying does not touch the
//! document (no `rev` bump, no dirty). It survives File ▸ Open, like every desktop editor's clipboard.
//!
//! Structure is preserved the same way the Alt-drag duplicate (`Document::dup_paths`) preserves it:
//! every Group ancestor of a copied path (up to its Layer) is mirrored, so groups stay groups, clip
//! groups keep their mask, and each top-level item keeps its live transform (A7).

use std::collections::{HashMap, HashSet};

use crate::geom::Pt;
use crate::model::{Anchor, Document, GroupRole, Node, NodeKind, Path};

/// A detached copy of some artwork: paths + the tree nodes that give them structure. Ids inside are
/// the ORIGINAL document's ids and serve only as keys between `paths` and `nodes`; every paste mints
/// fresh ids, so pasting the same clipboard twice gives two fully independent copies.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clipboard {
    /// Copied paths, back → front (z order at copy time).
    paths: Vec<Path>,
    /// Copied leaf + Group nodes. `parent` is `None` for a top-level item; `children` only list copied
    /// nodes; a clip group whose mask was not copied is demoted to a plain group.
    nodes: Vec<Node>,
    /// The top-level copied nodes, FRONT-first — the order they land at the front of the target layer.
    roots: Vec<u32>,
    /// World-space AABB `(x0, y0, x1, y1)` of the copied art at copy time (live transforms composed).
    bounds: Option<(f32, f32, f32, f32)>,
}

impl Clipboard {
    /// Nothing copied yet (Paste is then a no-op).
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
    /// Number of copied paths.
    pub fn len(&self) -> usize {
        self.paths.len()
    }
    /// World AABB of the copied art as it was when copied.
    pub fn bounds(&self) -> Option<(f32, f32, f32, f32)> {
        self.bounds
    }
    /// Centre of `bounds` — what a view-centred paste lines up with the centre of the view.
    pub fn center(&self) -> Option<Pt> {
        self.bounds.map(|(x0, y0, x1, y1)| [(x0 + x1) * 0.5, (y0 + y1) * 0.5])
    }

    /// Copy `pids` (whole paths) out of `doc` together with their Group ancestry. The document is only
    /// read. Paths that are not in the tree (not yet adopted by `sync_tree`) are copied as top-level items.
    pub fn capture(doc: &Document, pids: &[u32]) -> Clipboard {
        // the copied paths, deduped, back → front
        let mut seen = HashSet::new();
        let mut sel: Vec<(usize, u32)> =
            pids.iter().copied().filter(|p| seen.insert(*p)).filter_map(|p| doc.pidx(p).map(|i| (i, p))).collect();
        sel.sort_by_key(|(i, _)| *i);
        if sel.is_empty() {
            return Clipboard::default();
        }
        // the copied node set: each path's leaf + every Group ancestor up to (not incl.) its Layer
        let mut copied: Vec<u32> = vec![];
        let mut in_copy: HashSet<u32> = HashSet::new();
        let mut loose: Vec<u32> = vec![]; // paths with no leaf node yet
        for &(_, pid) in &sel {
            let Some(leaf) = doc.node_of_path(pid) else {
                loose.push(pid);
                continue;
            };
            let mut cur = Some(leaf);
            while let Some(n) = cur.and_then(|c| doc.node(c)) {
                if matches!(n.kind, NodeKind::Layer) {
                    break;
                }
                if in_copy.insert(n.id) {
                    copied.push(n.id);
                }
                cur = n.parent;
            }
        }
        let mut nodes: Vec<Node> = vec![];
        let mut roots: Vec<(usize, u32)> = vec![]; // (front-most storage index, node) for z ordering
        for &id in &copied {
            let Some(src) = doc.node(id) else { continue };
            let mut n = src.clone();
            n.children.retain(|c| in_copy.contains(c));
            if n.role == GroupRole::Clip && !n.mask_child.is_some_and(|m| n.children.contains(&m)) {
                // the mask was left behind → the copy is an ordinary group (as `sync_tree` would demote it)
                n.role = GroupRole::Normal;
                n.mask_child = None;
            }
            if n.parent.is_some_and(|p| !in_copy.contains(&p)) {
                n.parent = None;
                let z = doc.node_paths(id).iter().filter_map(|&p| doc.pidx(p)).max().unwrap_or(0);
                roots.push((z, id));
            }
            nodes.push(n);
        }
        // a loose path becomes its own top-level leaf; its key is an id no copied node uses
        let mut spare = doc.ids.max(nodes.iter().map(|n| n.id).max().unwrap_or(0));
        for pid in loose {
            spare += 1;
            nodes.push(Node {
                id: spare,
                kind: NodeKind::Path(pid),
                name: String::new(),
                parent: None,
                children: vec![],
                hidden: false,
                locked: false,
                color: None,
                clip_exempt: false,
                xform: Default::default(),
                role: GroupRole::Normal,
                mask_child: None,
            });
            roots.push((doc.pidx(pid).unwrap_or(0), spare));
        }
        roots.sort_by_key(|&(z, _)| std::cmp::Reverse(z));
        // world bounds, live transforms composed (the same AABB the selection frame / align use)
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(pi, _) in &sel {
            if doc.paths[pi].anchors.is_empty() {
                continue;
            }
            let (a, b, c, d) = doc.outline_bbox(pi);
            x0 = x0.min(a);
            y0 = y0.min(b);
            x1 = x1.max(c);
            y1 = y1.max(d);
        }
        Clipboard {
            paths: sel.iter().map(|&(pi, _)| doc.paths[pi].clone()).collect(),
            nodes,
            roots: roots.into_iter().map(|(_, id)| id).collect(),
            bounds: (x0 <= x1).then_some((x0, y0, x1, y1)),
        }
    }

    /// Insert a fresh copy into `doc`'s ACTIVE layer (at its front, the clipboard's own z order kept),
    /// moved by `offset` (`[0, 0]` = paste in place). Every path, anchor and node gets a new id. Returns
    /// the new path ids, back → front. The caller owns undo (`begin`/`commit`) and the selection.
    pub fn paste_into(&self, doc: &mut Document, offset: Pt) -> Vec<u32> {
        if self.is_empty() {
            return vec![];
        }
        doc.sync_tree(); // a valid active Layer to land on
        let host = doc.active_layer;
        let moved = |p: Pt| [p[0] + offset[0], p[1] + offset[1]];
        let mut pmap: HashMap<u32, u32> = HashMap::new();
        let mut new_paths: Vec<Path> = Vec::with_capacity(self.paths.len());
        for src in &self.paths {
            let id = doc.nid();
            let mut fresh = |a: &Anchor| {
                doc.ids += 1;
                Anchor { id: doc.ids, p: moved(a.p), hin: a.hin.map(moved), hout: a.hout.map(moved), smooth: a.smooth }
            };
            let anchors: Vec<Anchor> = src.anchors.iter().map(&mut fresh).collect();
            let holes: Vec<Vec<Anchor>> = src.holes.iter().map(|h| h.iter().map(&mut fresh).collect()).collect();
            pmap.insert(src.id, id);
            new_paths.push(Path { id, anchors, holes, ..src.clone() });
        }
        let mut nmap: HashMap<u32, u32> = HashMap::new();
        for n in &self.nodes {
            let id = doc.nid();
            nmap.insert(n.id, id);
        }
        for n in &self.nodes {
            let kind = match n.kind {
                NodeKind::Path(p) => match pmap.get(&p) {
                    Some(&np) => NodeKind::Path(np),
                    None => continue, // defensive: a leaf without its path is never pasted
                },
                k => k,
            };
            let xform = if n.xform.is_identity() { n.xform } else { n.xform.translated(offset) };
            doc.nodes.push(Node {
                id: nmap[&n.id],
                kind,
                parent: Some(n.parent.and_then(|p| nmap.get(&p).copied()).unwrap_or(host)),
                children: n.children.iter().filter_map(|c| nmap.get(c).copied()).collect(),
                mask_child: n.mask_child.and_then(|m| nmap.get(&m).copied()),
                xform,
                ..n.clone()
            });
        }
        let new_roots: Vec<u32> = self.roots.iter().filter_map(|r| nmap.get(r).copied()).collect();
        if let Some(h) = doc.nodes.iter_mut().find(|n| n.id == host) {
            for (i, r) in new_roots.into_iter().enumerate() {
                h.children.insert(i, r);
            }
        }
        let ids: Vec<u32> = new_paths.iter().map(|p| p.id).collect();
        doc.paths.extend(new_paths);
        doc.flatten();
        ids
    }
}
