use crate::{design::apply_design_op, dto::*, API, MAX_OPS, MAX_PAGE, MAX_TARGETS, MAX_TEXT, MCP_VERSION, TOOLS};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use varos_core::{
    bridge::{self, TargetErrorCode},
    editor::Editor,
    model::{Document, NodeKind},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoardInfo {
    pub board: String,
    pub name: String,
    pub rev: u64,
    pub dirty: bool,
    pub active: bool,
}
pub struct BoardAccess<'a> {
    pub editor: &'a mut Editor,
    pub dirty: bool,
}
/// Owned, revision-pinned CPU work. Capture on the owning thread, render on a host worker.
pub struct SnapshotJob {
    pub document: Document,
    pub rev: u64,
    pub size: [u32; 2],
    /// Persistent page id resolved against the owned document; None renders the fitted board.
    pub artboard: Option<u32>,
}
impl SnapshotJob {
    pub fn render(self, cancelled: &AtomicBool) -> Reply {
        let result = (|| {
            use base64::Engine;
            let checkpoint = || {
                if cancelled.load(Ordering::Acquire) {
                    Err(Error::new("cancelled", "snapshot cancelled"))
                } else {
                    Ok(())
                }
            };
            checkpoint()?;
            let raster = match self.artboard {
                None => varos_raster::rasterize(std::sync::Arc::new(self.document), self.size),
                Some(id) => {
                    let index = self
                        .document
                        .artboard_index(id)
                        .ok_or_else(|| Error::new("not_found", format!("unknown artboard:{id}")))?;
                    varos_raster::rasterize_artboard(std::sync::Arc::new(self.document), index, self.size)
                        .ok_or_else(|| Error::new("invalid_argument", "artboard cannot be rendered"))?
                }
            };
            checkpoint()?;
            let png = raster.encode_png().map_err(|e| Error::new("invalid_argument", e))?;
            checkpoint()?;
            if png.len() > (crate::MAX_FRAME - 4096) * 3 / 4 {
                return Err(Error::new("limit_exceeded", "PNG exceeds transport image budget"));
            }
            let mut out = json!({"rev":self.rev,"width":raster.width,"height":raster.height,"mime_type":"image/png","png":base64::engine::general_purpose::STANDARD.encode(png),"preview":"CPU preview"});
            if let Some(id) = self.artboard {
                out["artboard"] = json!(format!("artboard:{id}"));
                out["preview"] = json!("CPU page preview");
            }
            Ok(Reply::success(out))
        })();
        result.unwrap_or_else(Reply::failure)
    }
}
/// Only the desktop host supplies owning-thread mutable access. No transport knows an Editor.
pub trait Host {
    /// Synchronous headless default. Desktop overrides to defer work beyond the owning thread.
    fn snapshot(&mut self, job: SnapshotJob, cancelled: &AtomicBool) -> Reply {
        job.render(cancelled)
    }
    fn build(&self) -> &str {
        "host-unreported"
    }
    fn boards(&self) -> Vec<BoardInfo>;
    fn observation_ids(&self) -> Vec<String> {
        self.boards().into_iter().map(|b| b.board).collect()
    }
    fn observation_access(&mut self, board: &str) -> Result<BoardAccess<'_>, Error> {
        self.access(board)
    }
    fn prepare(&mut self, board: &str, mutation: bool) -> Result<(), Error>;
    fn access(&mut self, board: &str) -> Result<BoardAccess<'_>, Error>;
}
#[derive(Clone, Debug)]
pub struct Context {
    pub client: String,
    pub epoch: String,
    pub read: bool,
    pub edit: bool,
    pub allow_history: bool,
    pub allow_destructive: bool,
}
// Fixed-size settings that can change outside the normal document revision path.
#[derive(Clone, Copy, PartialEq)]
struct Settings {
    snap: varos_core::model::SnapConfig,
    units: varos_core::units::DocUnits,
    origin: [f32; 2],
    guides_locked: bool,
    move_art: bool,
    /// The active artboard's stable id: a navigation preference outside `Editor.rev` (slice 3).
    active_artboard: Option<u32>,
}
impl Settings {
    fn of(doc: &Document) -> Self {
        Self {
            snap: doc.snap,
            units: doc.units,
            origin: doc.ruler_origin,
            guides_locked: doc.guides_locked,
            move_art: doc.move_art_with_ab,
            active_artboard: doc.active_artboard().map(|a| a.id),
        }
    }
}
struct Observed {
    settings: Settings,
    dirty: bool,
    rev: u64,
    fingerprint: String,
    objects: BTreeMap<String, Value>,
    header: Value,
    order: Vec<String>,
    selection: Vec<String>,
    selection_rev: u64,
    journal: VecDeque<Value>,
    bytes: usize,
}
#[derive(Default)]
struct Client {
    high: u64,
    receipts: VecDeque<(String, String, Reply)>,
}
struct Grant {
    digest: String,
    expires: Instant,
}
pub struct Service {
    pub epoch: String,
    #[cfg(test)]
    fingerprints: usize,
    boards: HashMap<String, Observed>,
    clients: HashMap<String, Client>,
    grants: HashMap<String, Grant>,
}
impl Service {
    pub fn new(epoch: String) -> Self {
        Self {
            epoch,
            #[cfg(test)]
            fingerprints: 0,
            boards: HashMap::new(),
            clients: HashMap::new(),
            grants: HashMap::new(),
        }
    }
    /// Observe every committed human revision too; never journal a picker or held drag preview.
    pub fn observe(&mut self, host: &mut dyn Host) {
        let listed = host.observation_ids();
        self.grants.retain(|_, g| g.expires > Instant::now());
        self.boards.retain(|id, _| listed.contains(id));
        for b in listed {
            let Ok(a) = host.observation_access(&b) else { continue };
            let ed = a.editor;
            if ed.transaction_open() {
                continue;
            }
            let settings = Settings::of(&ed.doc);
            if let Some(old) = self.boards.get_mut(&b) {
                if old.rev == ed.rev && old.settings == settings && old.dirty == ed.dirty {
                    continue;
                }
            }
            #[cfg(test)]
            {
                self.fingerprints += 1;
            }
            let selection = selection(ed);
            let fingerprint = authored_fingerprint(&ed.doc);
            let (objects, header, order) = projection(&ed.doc);
            if let Some(old) = self.boards.get_mut(&b) {
                if old.fingerprint != fingerprint {
                    // Several document preferences still bypass the normal command revision path.
                    if ed.rev == old.rev {
                        ed.rev += 1;
                    }
                    let diff = changes(old.rev, ed.rev, &old.objects, &objects, &old.header, &header);
                    old.bytes += diff.to_string().len();
                    old.journal.push_back(diff);
                    while old.journal.len() > 128 || old.bytes > 8 * 1024 * 1024 {
                        if let Some(v) = old.journal.pop_front() {
                            old.bytes -= v.to_string().len();
                        } else {
                            break;
                        }
                    }
                } else if old.rev != ed.rev {
                    let diff = changes(old.rev, ed.rev, &old.objects, &objects, &old.header, &header);
                    old.bytes += diff.to_string().len();
                    old.journal.push_back(diff);
                    while old.journal.len() > 128 || old.bytes > 8 * 1024 * 1024 {
                        if let Some(v) = old.journal.pop_front() {
                            old.bytes -= v.to_string().len();
                        } else {
                            break;
                        }
                    }
                }
                if old.selection != selection {
                    old.selection_rev += 1;
                }
                old.settings = settings;
                old.dirty = ed.dirty;
                old.rev = ed.rev;
                old.fingerprint = fingerprint;
                old.objects = objects;
                old.header = header;
                old.order = order;
                old.selection = selection;
            } else {
                self.boards.insert(
                    b,
                    Observed {
                        settings,
                        dirty: ed.dirty,
                        rev: ed.rev,
                        fingerprint,
                        objects,
                        header,
                        order,
                        selection,
                        selection_rev: 0,
                        journal: VecDeque::new(),
                        bytes: 0,
                    },
                );
            }
        }
    }
    // Selection is transient and may change without Editor.rev. Read it at request boundaries,
    // never scan selected anchors/path arenas on an unchanged event-loop observation.
    fn observe_selection(&mut self, host: &mut dyn Host, board: &str) {
        if let (Some(old), Ok(a)) = (self.boards.get_mut(board), host.observation_access(board)) {
            let selection = selection(a.editor);
            if old.selection != selection {
                old.selection_rev += 1;
                old.selection = selection;
            }
        }
    }
    pub fn handle(&mut self, host: &mut dyn Host, ctx: &Context, req: Request, cancelled: &AtomicBool) -> Reply {
        let mut reply = self.handle_inner(host, ctx, &req, cancelled);
        if let Some((id, _)) = req.mutation() {
            if sequence(id).is_ok() {
                reply.request_id = Some(id.into());
            }
        }
        if let Some(b) = req.board() {
            if canonical_board(b) {
                reply.board = Some(b.into());
            }
            if reply.rev.is_none() {
                reply.rev = self.boards.get(b).map(|b| b.rev);
            }
        }
        if compact(&reply).len() > MAX_TEXT {
            if reply.ok && req.mutation().is_some() {
                // The receipt still proves commit; a large selection/detail must be read in pages.
                reply.result = Some(
                    json!({"from":req.mutation().unwrap().1,"rev":reply.rev,"more":true,"resync_required":true,"detail_request":{"board":reply.board,"rev":reply.rev,"fields":["bounds","paint","parent","name","state"],"limit":20}}),
                );
            } else {
                let mut error =
                    Reply::failure(Error::new("limit_exceeded", "response exceeds the 16 KiB page budget; request fewer objects or omit geometry; an individual oversized path cannot be paginated"));
                error.board = reply.board;
                error.rev = reply.rev;
                reply = error;
            }
        }
        reply
    }
    fn handle_inner(&mut self, host: &mut dyn Host, ctx: &Context, req: &Request, cancelled: &AtomicBool) -> Reply {
        if req.api() != API {
            return Reply::failure(Error::new("unsupported", "Bridge API must be 1.0"));
        }
        if ctx.epoch != self.epoch {
            return Reply::failure(Error::new("not_found", "attachment epoch expired"));
        }
        if !ctx.read || (req.mutation().is_some() && !ctx.edit) {
            return Reply::failure(Error::new("scope_refused", "attachment has no required grant"));
        }
        if req.board().is_some_and(|board| !canonical_board(board)) {
            return Reply::failure(Error::new("invalid_argument", "board must be a canonical session handle bN"));
        }
        let payload = digest(&serde_json::to_value(req).expect("DTO serialization"));
        if let Some((id, _)) = req.mutation() {
            let Ok(seq) = sequence(id) else {
                return Reply::failure(Error::new(
                    "invalid_argument",
                    "request_id must be r followed by a positive integer",
                ));
            };
            if self.clients.len() >= 64 && !self.clients.contains_key(&ctx.client) {
                return Reply::failure(Error::new("limit_exceeded", "too many authorized clients this launch"));
            }
            let client = self.clients.entry(ctx.client.clone()).or_default();
            if let Some((_, hash, reply)) = client.receipts.iter().find(|(key, _, _)| key == id) {
                return if hash == &payload {
                    reply.clone()
                } else {
                    Reply::failure(Error::new("invalid_argument", "request_id reused with a different payload"))
                };
            }
            if seq <= client.high {
                return Reply::failure(Error::new(
                    "not_found",
                    "request_id older than retained receipt window; inspect revision",
                ));
            }
        }
        self.observe(host);
        let outcome = (|| -> Result<Reply, Error> {
            if cancelled.load(Ordering::Acquire) {
                return Err(Error::new("cancelled", "cancelled before commit"));
            }
            if let Some(board) = req.board() {
                host.prepare(board, req.mutation().is_some())?;
                self.observe(host); // field commit is a separate human undo step BEFORE revision compare
                self.observe_selection(host, board);
                let current = self
                    .boards
                    .get(board)
                    .ok_or_else(|| Error::new("not_found", "board closed or not authorized"))?
                    .rev;
                let expected = req.mutation().map(|(_, rev)| rev).or({
                    match req {
                        Request::Describe(v) => v.rev,
                        Request::Snapshot(v) => Some(v.rev),
                        _ => None,
                    }
                });
                if let Some(expected) = expected {
                    if expected != current {
                        let mut e = Error::new("revision_conflict", "describe the fresh revision before retrying");
                        e.expected_rev = Some(expected);
                        e.actual_rev = Some(current);
                        return Err(e);
                    }
                }
            }
            match req {
                Request::Capabilities(_) => Ok(Reply::success(
                    json!({"api":API,"mcp":MCP_VERSION,"epoch":self.epoch,"client":ctx.client,"app_build":host.build(),"readable_vrs":[1,2,3,4],"writable_vrs":[4],"mode":"attached","tools":TOOLS,"edit_verbs":crate::EDIT_VERBS,"ids":"path:N/node:N/artboard:N; path/node ids are scoped to epoch, artboard ids are persistent (format 4)","deprecated":{"aN@rev":"revision-bound artboard reference; use artboard:N (removed after slice 4)"},"artboard_presets":{"square":[1080,1080],"portrait":[1080,1350],"story":[1080,1920],"a4":[595,842]},"limits":{"request_bytes":crate::MAX_FRAME,"operations":MAX_OPS,"targets":MAX_TARGETS,"page":MAX_PAGE,"text_bytes":MAX_TEXT,"geometry_anchors_per_object":1000,"geometry_page_bytes":MAX_TEXT,"geometry_typical_anchors_per_page":300,"geometry_anchor_pagination":false,"snapshot_max_dimension":1024,"journal_revisions":128,"journal_bytes":8*1024*1024},"read":ctx.read,"edit":ctx.edit,"history_owner_grant":ctx.allow_history,"destructive_owner_grant":ctx.allow_destructive,"detail_fields":["bounds","paint","parent","name","state","metadata","artboards","geometry"],"unsupported":["files","headless","add_path","corner_radius","flip","pathfinder","group_distribution","gap_distribution","reparent","artboard_reorder","artboard_duplicate","artboard_paint"]}),
                )),
                Request::ListBoards(v) => {
                    check_page(v.limit)?;
                    let boards = host.boards();
                    let sig = digest(&json!({"boards":boards,"limit":v.limit}));
                    let offset = cursor_offset(v.cursor.as_deref(), &sig)?;
                    let end = (offset + v.limit).min(boards.len());
                    if offset > boards.len() {
                        return Err(Error::new("invalid_argument", "cursor offset out of range"));
                    }
                    Ok(Reply::success(
                        json!({"epoch":self.epoch,"boards":boards[offset..end],"more":end<boards.len(),"cursor":(end<boards.len()).then(||cursor(&sig,end))}),
                    ))
                }
                Request::Describe(v) => self.describe(v, host),
                Request::Snapshot(v) => {
                    let (width, height) = v.size();
                    if width == 0 || height == 0 || width > 1024 || height > 1024 {
                        return Err(Error::new("limit_exceeded", "snapshot dimensions must be 1..1024"));
                    }
                    let document = host.access(&v.board)?.editor.doc.clone();
                    let artboard = match &v.artboard {
                        None => None,
                        Some(id) => {
                            let n = id
                                .strip_prefix("artboard:")
                                .and_then(|n| n.parse::<u32>().ok())
                                .filter(|n| *n > 0 && format!("artboard:{n}") == *id)
                                .ok_or_else(|| Error::new("invalid_argument", "artboard must be artboard:N"))?;
                            document
                                .artboard_index(n)
                                .ok_or_else(|| Error::new("not_found", format!("unknown {id}")))?;
                            Some(n)
                        }
                    };
                    Ok(host.snapshot(SnapshotJob { document, rev: v.rev, size: [width, height], artboard }, cancelled))
                }
                Request::Select(v) => {
                    if v.ids.len() > MAX_TARGETS {
                        return Err(Error::new("limit_exceeded", "too many targets"));
                    }
                    let a = host.access(&v.board)?;
                    let paths = resolve(&a.editor.doc, &v.ids, true)?;
                    a.editor.bridge_select(paths).map_err(|reason| Error::new("invalid_argument", reason))?;
                    self.observe(host);
                    self.observe_selection(host, &v.board);
                    let b = &self.boards[&v.board];
                    Ok(Reply::success(
                        json!({"selection":b.selection.iter().take(100).collect::<Vec<_>>(),"selection_count":b.selection.len(),"selection_more":b.selection.len()>100,"selection_rev":b.selection_rev}),
                    ))
                }
                Request::Edit(v) => {
                    if v.ops.is_empty() || v.ops.len() > MAX_OPS {
                        return Err(Error::new("limit_exceeded", "edit needs 1..100 operations"));
                    }
                    let a = host.access(&v.board)?;
                    if v.ops.iter().map(|op| op.ids().len()).sum::<usize>() > MAX_TARGETS {
                        return Err(Error::new("limit_exceeded", "edit exceeds 1000 explicit targets"));
                    }
                    // Review P2 (slice 3): a deprecated `aN@rev` alias names a page by its index AT `rev`.
                    // Page verbs in the same batch can shift indices in the stage, so the alias could
                    // silently retarget another page: refuse the combination instead of guessing.
                    if v.ops.iter().any(Operation::is_page_verb) {
                        if let Some(index) = v.ops.iter().position(Operation::uses_legacy_artboard_alias) {
                            return Err(Error::new(
                                "invalid_argument",
                                "the deprecated aN@rev artboard alias cannot be combined with page verbs in one \
                                 batch; use artboard:N",
                            )
                            .at(index));
                        }
                    }
                    let from = a.editor.rev;
                    let mut locals = BTreeMap::new();
                    let mut expanded = 0usize;
                    let mut affected = std::collections::BTreeSet::new();
                    let batch = a.editor.prepare_design_batch(
                        v.ops.len(),
                        |staged, index| {
                            if index == 0 {
                                locals.clear();
                                expanded = 0;
                                affected.clear();
                            }
                            apply_design_op(
                                staged,
                                &v.ops[index],
                                v.expected_rev,
                                &mut locals,
                                &mut expanded,
                                &mut affected,
                            )
                            .map_err(|e| e.at(index))
                        },
                        |index, reason| {
                            Error::new(
                                if reason == "active gesture" {
                                    "busy"
                                } else if reason.starts_with("cancelled") {
                                    "cancelled"
                                } else {
                                    "invalid_argument"
                                },
                                reason,
                            )
                            .at(index)
                        },
                        || cancelled.load(Ordering::Acquire),
                    )?;
                    if v.ops.iter().any(Operation::destructive) {
                        if affected.len() > MAX_TARGETS
                            || serde_json::to_string(&affected).expect("affected ids").len() > MAX_TEXT - 2048
                        {
                            return Err(Error::new(
                                "limit_exceeded",
                                "destructive affected IDs exceed confirmation budget",
                            ));
                        }
                        let mut exact = serde_json::to_value(v).expect("DTO");
                        exact.as_object_mut().unwrap().remove("digest");
                        let hash = digest(
                            &json!({"epoch":self.epoch,"edit":exact,"target":authored_fingerprint(batch.document()),"affected":affected,"source":authored_fingerprint(&a.editor.doc)}),
                        );
                        let key = format!("destructive:{}:{}", ctx.client, v.board);
                        if v.digest.as_deref() != Some(&hash)
                            || !self.grants.get(&key).is_some_and(|g| g.digest == hash && g.expires > Instant::now())
                        {
                            if ctx.allow_destructive {
                                if self.grants.len() >= 128 && !self.grants.contains_key(&key) {
                                    return Err(Error::new("limit_exceeded", "too many outstanding confirmations"));
                                }
                                self.grants.insert(
                                    key,
                                    Grant { digest: hash.clone(), expires: Instant::now() + Duration::from_secs(60) },
                                );
                            }
                            let mut e = Error::new("confirmation_required", "delete/ungroup requires an exact owner-issued grant; desktop VAROS_BRIDGE_ALLOW_DESTRUCTIVE=1 enables the temporary grant policy");
                            e.digest = Some(hash);
                            e.ids = affected.into_iter().collect();
                            e.expected_rev = Some(v.expected_rev);
                            e.actual_rev = Some(from);
                            return Err(e);
                        }
                        let grant = self
                            .grants
                            .remove(&key)
                            .ok_or_else(|| Error::new("confirmation_required", "no owner-issued destructive grant"))?;
                        if grant.digest != hash || grant.expires < Instant::now() {
                            return Err(Error::new("confirmation_required", "destructive grant expired"));
                        }
                    }
                    if cancelled.load(Ordering::Acquire) {
                        return Err(Error::new("cancelled", "cancelled before commit"));
                    }
                    a.editor.publish_design_batch(batch).map_err(|reason| Error::new("busy", reason))?;
                    self.observe(host);
                    let mut reply = self.edit_receipt_reserved(
                        &v.board,
                        from,
                        serde_json::to_string(&locals).expect("locals").len(),
                    );
                    if !locals.is_empty() {
                        reply.result.as_mut().unwrap()["locals"] = json!(locals);
                    }
                    Ok(reply)
                }
                Request::History(v) => {
                    let a = host.access(&v.board)?;
                    let redo = v.action == HistoryAction::Redo;
                    let preview = a.editor.history_preview(redo).ok_or_else(|| {
                        Error::new(if redo { "nothing_to_redo" } else { "nothing_to_undo" }, "history is empty")
                    })?;
                    let (target, target_header, _) = projection(preview);
                    let b = &self.boards[&v.board];
                    let affected = changes(b.rev, b.rev + 1, &b.objects, &target, &b.header, &target_header);
                    let affected_ids: Vec<String> = affected["changed"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .chain(affected["created"].as_array().unwrap().iter())
                        .map(|v| v["id"].as_str().unwrap().to_owned())
                        .chain(affected["removed"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()))
                        .collect();
                    let target_digest = authored_fingerprint(preview);
                    let key = format!("{}:{}", ctx.client, v.board);
                    let mut exact = serde_json::to_value(v).expect("DTO");
                    exact.as_object_mut().unwrap().remove("digest");
                    let hash = digest(
                        &json!({"epoch":self.epoch,"history":exact,"target":target_digest,"objects":self.boards[&v.board].objects}),
                    );
                    if v.digest.as_deref() != Some(&hash)
                        || !self.grants.get(&key).is_some_and(|g| g.digest == hash && g.expires > Instant::now())
                    {
                        if ctx.allow_history {
                            if self.grants.len() >= 128 && !self.grants.contains_key(&key) {
                                return Err(Error::new("limit_exceeded", "too many outstanding confirmations"));
                            }
                            self.grants.insert(
                                key,
                                Grant { digest: hash.clone(), expires: Instant::now() + Duration::from_secs(60) },
                            );
                        }
                        let mut e=Error::new("confirmation_required","shared history may undo a human edit; retry the exact payload with this digest only when the owner launched the desktop with VAROS_BRIDGE_ALLOW_HISTORY=1");
                        e.digest = Some(hash);
                        e.ids = affected_ids;
                        e.expected_rev = Some(v.expected_rev);
                        e.actual_rev = Some(b.rev);
                        return Err(e);
                    }
                    let grant = self.grants.remove(&key).ok_or_else(|| {
                        Error::new("confirmation_required", "no owner-issued single-use history grant")
                    })?;
                    if grant.digest != hash || grant.expires < Instant::now() {
                        return Err(Error::new("confirmation_required", "history grant expired"));
                    }
                    let a = host.access(&v.board)?;
                    let redo = v.action == HistoryAction::Redo;
                    if !a.editor.history_available(redo) {
                        return Err(Error::new(
                            if redo { "nothing_to_redo" } else { "nothing_to_undo" },
                            "history is empty",
                        ));
                    }
                    if cancelled.load(Ordering::Acquire) {
                        return Err(Error::new("cancelled", "cancelled before history commit"));
                    }
                    let from = a.editor.rev;
                    if redo {
                        a.editor.redo();
                    } else {
                        a.editor.undo();
                    }
                    self.observe(host);
                    let mut r = self.edit_receipt(&v.board, from);
                    r.undo_steps = 0;
                    r.result.as_mut().unwrap()["history"] = json!(v.action);
                    r.result.as_mut().unwrap()["history_steps"] = json!(1);
                    Ok(r)
                }
                Request::RequestStatus(v) => self
                    .clients
                    .get(&ctx.client)
                    .and_then(|c| c.receipts.iter().find(|(id, _, _)| id == &v.request_id))
                    .map(|(_, _, r)| Reply::success(json!({"status":"completed","receipt":r})))
                    .ok_or_else(|| Error::new("not_found", "receipt not retained for this client")),
            }
        })();
        let mut reply = outcome.unwrap_or_else(Reply::failure);
        if let Some(b) = req.board() {
            if canonical_board(b) {
                reply.board = Some(b.into());
            }
            reply.rev = self.boards.get(b).map(|b| b.rev);
        }
        if let Some((id, _)) = req.mutation() {
            if sequence(id).is_ok() {
                reply.request_id = Some(id.into());
            }
            // Only completed mutations consume IDs. Pre-commit failures may be retried.
            if reply.ok {
                let client = self.clients.entry(ctx.client.clone()).or_default();
                client.high = sequence(id).unwrap();
                client.receipts.push_back((id.into(), payload, reply.clone()));
                while client.receipts.len() > 128 {
                    client.receipts.pop_front();
                }
            }
        }
        reply
    }
    fn edit_receipt(&self, board: &str, from: u64) -> Reply {
        self.edit_receipt_reserved(board, from, 0)
    }
    fn edit_receipt_reserved(&self, board: &str, from: u64, reserved: usize) -> Reply {
        let b = &self.boards[board];
        let diff = b
            .journal
            .back()
            .filter(|d| d["from"] == from && d["rev"] == b.rev)
            .cloned()
            .unwrap_or_else(|| json!({"from":from,"rev":b.rev,"changed":[],"created":[],"removed":[]}));
        let mut value = diff;
        strip_journal(&mut value);
        if value.to_string().len() > MAX_TEXT - 4096 - reserved
            || (b.rev != from && b.journal.back().is_none_or(|d| d["from"] != from))
        {
            let counts = json!({"changed":value["changed"].as_array().map(Vec::len),"created":value["created"].as_array().map(Vec::len),"removed":value["removed"].as_array().map(Vec::len)});
            let fields = json!(["bounds", "paint", "parent", "name", "state"]);
            let sig =
                digest(&json!({"epoch":self.epoch,"board":board,"rev":b.rev,"ids":b.order,"fields":fields,"limit":20}));
            value = json!({"from":from,"rev":b.rev,"counts":counts,"more":true,"cursor":cursor(&sig,0),"detail_request":{"board":board,"rev":b.rev,"fields":fields,"limit":20,"cursor":cursor(&sig,0)}});
        }
        value["selection"] = json!(b.selection.iter().take(100).collect::<Vec<_>>());
        value["selection_count"] = json!(b.selection.len());
        value["selection_more"] = json!(b.selection.len() > 100);
        value["selection_rev"] = json!(b.selection_rev);
        value["changed_document"] = json!(b.rev != from);
        if !b.header["active_artboard"].is_null() {
            // additive (slice 3): only boards with pages carry it, so frozen page-less receipts are unchanged
            value["active_artboard"] = b.header["active_artboard"].clone();
        }
        let mut r = Reply::success(value);
        r.undo_steps = u8::from(b.rev != from);
        r
    }
    fn describe(&self, v: &Describe, host: &mut dyn Host) -> Result<Reply, Error> {
        check_page(v.limit)?;
        let b = &self.boards[&v.board];
        if let Some(since) = v.since {
            if v.ids.is_some() || v.fields.is_some() || v.cursor.is_some() {
                return Err(Error::new("invalid_argument", "since cannot be combined with object details or cursor"));
            }
            if since == b.rev {
                return Ok(Reply::success(
                    json!({"from":since,"rev":b.rev,"changed":[],"created":[],"removed":[],"selection":b.selection.iter().take(100).collect::<Vec<_>>(),"selection_count":b.selection.len(),"selection_more":b.selection.len()>100,"selection_rev":b.selection_rev}),
                ));
            }
            let start = b.journal.iter().position(|d| d["from"] == since).ok_or_else(|| {
                Error::new("resync_required", "revision is outside the retained journal; request a summary")
            })?;
            // Reconstruct the exact origin from the bounded journal, then compute a net diff.
            // Undo followed by redo (or a later paint update) must not lose earlier fields.
            let mut origin = b.objects.clone();
            let mut origin_header = b.header.clone();
            for d in b.journal.iter().skip(start).rev() {
                for o in d["created"].as_array().unwrap() {
                    origin.remove(o["id"].as_str().unwrap());
                }
                for (id, o) in d["before"].as_object().unwrap() {
                    origin.insert(id.clone(), o.clone());
                }
                if !d["before_header"].is_null() {
                    origin_header = d["before_header"].clone();
                }
            }
            let mut value = changes(since, b.rev, &origin, &b.objects, &origin_header, &b.header);
            strip_journal(&mut value);
            value["selection"] = json!(b.selection);
            value["selection_rev"] = json!(b.selection_rev);
            if value.to_string().len() > MAX_TEXT {
                return Err(Error::new("resync_required", "net diff exceeds page budget; request paginated details"));
            }
            return Ok(Reply::success(value));
        }
        if let Some(fields) = &v.fields {
            if fields.iter().any(|f| f == "metadata" || f == "artboards") {
                if v.ids.is_some() || fields.iter().any(|f| f != "metadata" && f != "artboards") {
                    return Err(Error::new(
                        "invalid_argument",
                        "board detail fields cannot be mixed with object ids or fields",
                    ));
                }
                let sig =
                    digest(&json!({"epoch":self.epoch,"board":v.board,"rev":b.rev,"fields":fields,"limit":v.limit}));
                let offset = cursor_offset(v.cursor.as_deref(), &sig)?;
                let mut out = json!({"rev":b.rev,"more":false,"cursor":null});
                if fields.iter().any(|f| f == "metadata") {
                    out["metadata"] = b.header["metadata"].clone();
                }
                if fields.iter().any(|f| f == "artboards") {
                    let all = b.header["artboards"].as_array().unwrap();
                    if offset > all.len() {
                        return Err(Error::new("invalid_argument", "cursor offset out of range"));
                    }
                    let mut page = vec![];
                    let mut end = offset;
                    while end < all.len() && page.len() < v.limit {
                        let mut a = all[end].clone();
                        a["ref"] = json!(format!("a{end}@{}", b.rev));
                        if out.to_string().len() + json!(&page).to_string().len() + a.to_string().len()
                            > MAX_TEXT - 2048
                        {
                            break;
                        }
                        page.push(a);
                        end += 1;
                    }
                    if end == offset && end < all.len() {
                        return Err(Error::new("limit_exceeded", "artboard exceeds detail budget"));
                    }
                    out["artboards"] = json!(page);
                    out["more"] = json!(end < all.len());
                    out["cursor"] = json!((end < all.len()).then(|| cursor(&sig, end)));
                } else if v.cursor.is_some() {
                    return Err(Error::new("invalid_argument", "metadata has no cursor"));
                }
                if out.to_string().len() > MAX_TEXT - 512 {
                    return Err(Error::new("limit_exceeded", "metadata exceeds detail budget"));
                }
                return Ok(Reply::success(out));
            }
        }
        if v.ids.is_none() && v.fields.is_none() && v.cursor.is_none() {
            let mut h = b.header.clone();
            h.as_object_mut().unwrap().remove("metadata");
            h["artboards"] = json!(h["artboards"].as_array().unwrap().iter().take(20).collect::<Vec<_>>());
            h["rev"] = json!(b.rev);
            h["selection"] = json!(b.selection.iter().take(100).collect::<Vec<_>>());
            h["selection_count"] = json!(b.selection.len());
            h["selection_more"] = json!(b.selection.len() > 100);
            for a in h["artboards"].as_array_mut().unwrap() {
                a["ref"] = json!(format!("a{}@{}", a["index"], b.rev));
            }
            h["selection_rev"] = json!(b.selection_rev);
            h["dirty"] = json!(host.access(&v.board)?.dirty);
            return Ok(Reply::success(h));
        }
        let fields =
            v.fields.clone().unwrap_or_else(|| vec!["bounds".into(), "paint".into(), "parent".into(), "name".into()]);
        if fields.iter().any(|f| !["bounds", "paint", "parent", "name", "state", "geometry"].contains(&f.as_str())) {
            return Err(Error::new(
                "unsupported",
                "supported detail fields: bounds, paint, parent, name, state, geometry",
            ));
        }
        let all = b.order.clone();
        let ids = v.ids.clone().unwrap_or(all);
        if v.ids.as_ref().is_some_and(|ids| ids.len() > MAX_TARGETS) {
            return Err(Error::new("limit_exceeded", "detail query exceeds 1000 ids"));
        }
        let sig =
            digest(&json!({"epoch":self.epoch,"board":v.board,"rev":b.rev,"ids":ids,"fields":fields,"limit":v.limit}));
        let offset = cursor_offset(v.cursor.as_deref(), &sig)?;
        if offset > ids.len() {
            return Err(Error::new("invalid_argument", "cursor offset out of range"));
        }
        let geometry = fields.iter().any(|f| f == "geometry");
        let doc = geometry.then(|| host.access(&v.board).map(|a| a.editor.doc.clone())).transpose()?;
        let mut objects = vec![];
        let mut end = offset;
        while end < ids.len() && objects.len() < v.limit {
            let id = &ids[end];
            let source = b.objects.get(id).ok_or_else(|| Error::new("not_found", format!("unknown {id}")))?;
            let mut out = json!({"id":id,"kind":source["kind"]});
            for field in &fields {
                match field.as_str() {
                    "geometry" => {
                        out["geometry"] = Value::Null;
                        if let Some(pid) = id.strip_prefix("path:").and_then(|n| n.parse::<u32>().ok()) {
                            let doc = doc.as_ref().expect("geometry requested");
                            let p = &doc.paths[doc.pidx(pid).expect("observed id")];
                            // A single oversized object is explicitly refused; object pages stay within the text budget.
                            if p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>() > 1000 {
                                return Err(Error::new(
                                    "limit_exceeded",
                                    "geometry exceeds 1000 anchors; no partial geometry returned",
                                ));
                            }
                            out["geometry"] = json!({"closed":p.closed,"anchors":p.anchors.iter().map(|a| json!({"point":a.p,"hin":a.hin,"hout":a.hout})).collect::<Vec<_>>(),"holes":p.holes.iter().map(|ring| ring.iter().map(|a| json!({"point":a.p,"hin":a.hin,"hout":a.hout})).collect::<Vec<_>>()).collect::<Vec<_>>(),"world_transform":doc.unit_xform(pid)});
                        }
                    }
                    "paint" => {
                        for key in ["fill", "stroke", "stroke_width", "opacity"] {
                            out[key] = source[key].clone();
                        }
                    }
                    "state" => {
                        for key in ["hidden", "locked"] {
                            out[key] = source[key].clone();
                        }
                    }
                    field => out[field] = source[field].clone(),
                }
            }
            if json!(&objects).to_string().len() + out.to_string().len() > MAX_TEXT - 2048 {
                break;
            }
            objects.push(out);
            end += 1;
        }
        if end == offset && end < ids.len() {
            return Err(Error::new("limit_exceeded", "individual object exceeds the 16 KiB page budget (including response overhead); omit geometry or request other fields; within-path anchor pagination is not supported"));
        }
        Ok(Reply::success(
            json!({"rev":b.rev,"objects":objects,"more":end<ids.len(),"cursor":(end<ids.len()).then(||cursor(&sig,end))}),
        ))
    }
}
fn canonical_board(board: &str) -> bool {
    board.strip_prefix('b').and_then(|n| n.parse::<u64>().ok()).is_some_and(|n| n > 0 && format!("b{n}") == board)
}
fn sequence(id: &str) -> Result<u64, ()> {
    id.strip_prefix('r').and_then(|s| s.parse::<u64>().ok()).filter(|s| *s > 0 && format!("r{s}") == id).ok_or(())
}
pub fn digest(value: &Value) -> String {
    Sha256::digest(value.to_string().as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}
fn authored_fingerprint(doc: &Document) -> String {
    let mut v = json!(doc);
    for key in ["ids", "active", "active_layer"] {
        v.as_object_mut().unwrap().remove(key);
    }
    digest(&v)
}
fn check_page(n: usize) -> Result<(), Error> {
    if (1..=MAX_PAGE).contains(&n) {
        Ok(())
    } else {
        Err(Error::new("invalid_argument", "limit must be 1..100"))
    }
}
fn cursor(sig: &str, offset: usize) -> String {
    format!("{sig}:{offset}")
}
fn cursor_offset(value: Option<&str>, sig: &str) -> Result<usize, Error> {
    match value {
        None => Ok(0),
        Some(v) => v
            .split_once(':')
            .filter(|(s, _)| *s == sig)
            .and_then(|(_, n)| n.parse().ok())
            .ok_or_else(|| Error::new("resync_required", "cursor query or revision expired")),
    }
}
fn selection(ed: &Editor) -> Vec<String> {
    let mut ids: Vec<_> = ed.selected_pids().iter().map(|id| format!("path:{id}")).collect();
    ids.sort();
    ids
}
pub(crate) fn paint(p: &Paint) -> Result<Option<Option<[f32; 4]>>, Error> {
    match p {
        Paint::Unchanged => Ok(None),
        Paint::None => Ok(Some(None)),
        Paint::Solid(s) => {
            if s.len() != 9 || !s.starts_with('#') || !s[1..].bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(Error::new("invalid_argument", "solid color must be #RRGGBBAA"));
            }
            let mut c = [0.0; 4];
            for i in 0..4 {
                c[i] = u8::from_str_radix(&s[1 + i * 2..3 + i * 2], 16).unwrap() as f32 / 255.0;
            }
            Ok(Some(Some(c)))
        }
    }
}
pub(crate) fn resolve(doc: &Document, ids: &[String], empty: bool) -> Result<Vec<u32>, Error> {
    if ids.is_empty() && !empty {
        return Err(Error::new("invalid_argument", "explicit targets must not be empty"));
    }
    let mut out = std::collections::BTreeSet::new();
    for id in ids {
        let (namespace, num) =
            id.split_once(':').ok_or_else(|| Error::new("invalid_argument", "use path:N or node:N"))?;
        let n = num.parse::<u32>().map_err(|_| Error::new("invalid_argument", "id suffix must be u32"))?;
        let paths = match namespace {
            "path" => {
                if doc.pidx(n).is_none() {
                    return Err(Error::new("not_found", format!("unknown {id}")));
                }
                vec![n]
            }
            "node" => {
                let node = doc.node(n).ok_or_else(|| Error::new("not_found", format!("unknown {id}")))?;
                if node.locked || node.hidden {
                    return Err(Error::new(
                        if node.locked { "locked_target" } else { "hidden_target" },
                        format!("{id} is inert"),
                    ));
                }
                doc.node_paths(n)
            }
            "artboard" => {
                return Err(Error::new(
                    "invalid_argument",
                    format!("{id} is an artboard, not an object; use it in a page verb, align target or snapshot"),
                ))
            }
            _ => return Err(Error::new("invalid_argument", "unknown id namespace")),
        };
        if paths.is_empty() {
            return Err(Error::new("unsupported", "target contains no paths"));
        }
        for pid in paths {
            if doc.eff_locked(pid) || doc.eff_hidden(pid) {
                let mut e = Error::new(
                    if doc.eff_locked(pid) { "locked_target" } else { "hidden_target" },
                    format!("path:{pid} is inert"),
                );
                e.ids = vec![format!("path:{pid}")];
                return Err(e);
            }
            out.insert(pid);
        }
        if out.len() > MAX_TARGETS {
            return Err(Error::new("limit_exceeded", "expanded target set exceeds 1000 paths"));
        }
    }
    Ok(out.into_iter().collect())
}
fn color(v: &Value) -> Value {
    let Some(c) = v.as_array() else {
        return Value::Null;
    };
    Value::String(format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        (c[0].as_f64().unwrap() * 255.0).round() as u8,
        (c[1].as_f64().unwrap() * 255.0).round() as u8,
        (c[2].as_f64().unwrap() * 255.0).round() as u8,
        (c[3].as_f64().unwrap() * 255.0).round() as u8
    ))
}
fn projection(doc: &Document) -> (BTreeMap<String, Value>, Value, Vec<String>) {
    let details = bridge::elements(doc, true);
    let source = bridge::describe(doc, None).expect("core summary projection");
    let mut objects = BTreeMap::new();
    let mut bounds: Option<[f64; 4]> = None;
    for v in source["elements"].as_array().unwrap() {
        let id = v["id"].as_str().unwrap();
        let mut o = v.clone();
        if let Some(b) = v["bounds"].as_array() {
            let a = [b[0].as_f64().unwrap(), b[1].as_f64().unwrap(), b[2].as_f64().unwrap(), b[3].as_f64().unwrap()];
            o["bounds"] = json!([a[0], a[1], round(a[2] - a[0]), round(a[3] - a[1])]);
            if v["kind"] == "path" {
                bounds = Some(match bounds {
                    None => a,
                    Some(p) => [p[0].min(a[0]), p[1].min(a[1]), p[2].max(a[2]), p[3].max(a[3])],
                });
            }
        }
        o["fill"] = color(&v["fill"]);
        o["stroke"] = color(&v["stroke"]["paint"]);
        o["stroke_width"] = v["stroke"]["width"].clone();
        // A hash detects geometry edits that keep the same bounds; geometry itself never leaks.
        o["geometry_digest"] = json!(geometry_digest(&details[id]));
        objects.insert(id.into(), o);
    }
    let counts = json!({"paths":doc.paths.len(),"groups":doc.nodes.iter().filter(|n|n.kind==NodeKind::Group).count(),"layers":doc.nodes.iter().filter(|n|n.kind==NodeKind::Layer).count(),"artboards":doc.artboards.len()});
    let artboards:Vec<_>=doc.artboards.iter().enumerate().map(|(i,a)|json!({"id":format!("artboard:{}",a.id),"index":i,"name":a.name,"bounds":[round(a.x as f64),round(a.y as f64),round(a.w as f64),round(a.h as f64)],"bleed":round(a.bleed as f64),"fill":color(&json!(a.page_color)),"clip":a.clip,"hidden":a.hidden,"locked":a.locked})).collect();
    let mut settings = json!(doc);
    for k in [
        "paths",
        "nodes",
        "groups",
        "group_of",
        "ids",
        "active",
        "active_layer",
        "name",
        "description",
        "tags",
        "artboards",
    ] {
        settings.as_object_mut().unwrap().remove(k);
    }
    let metadata = json!({"description":doc.description,"tags":doc.tags,"display_units":doc.units.display.suffix(),"ppi":round(doc.units.ppi as f64),"settings_digest":digest(&settings)});
    let header = json!({"name":doc.name,"units":"pt","counts":counts,"active_artboard":doc.active_artboard().map(|a|format!("artboard:{}",a.id)),"bounds":bounds.map(|b|[b[0],b[1],round(b[2]-b[0]),round(b[3]-b[1])]),"artboards":artboards,"artboards_more":doc.artboards.len()>20,"metadata":metadata,"detail":["objects","artboards","paint","metadata","parent","name","state"]});
    let order = source["elements"].as_array().unwrap().iter().map(|v| v["id"].as_str().unwrap().to_owned()).collect();
    (objects, header, order)
}
fn round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
fn geometry_digest(v: &Value) -> String {
    if v["kind"] == "path" {
        digest(
            &json!({"anchors":v["geometry"]["anchors"],"holes":v["geometry"]["holes"],"closed":v["geometry"]["closed"],"transform":v["world_transform"]}),
        )
    } else {
        digest(&v["geometry"])
    }
}
fn strip_journal(v: &mut Value) {
    if let Some(o) = v.as_object_mut() {
        o.remove("before");
        o.remove("before_header");
    }
}
fn public(mut v: Value) -> Value {
    v.as_object_mut().unwrap().remove("geometry_digest");
    v
}
fn changes(
    from: u64,
    rev: u64,
    a: &BTreeMap<String, Value>,
    b: &BTreeMap<String, Value>,
    ah: &Value,
    bh: &Value,
) -> Value {
    let created: Vec<_> = b.iter().filter(|(id, _)| !a.contains_key(*id)).map(|(_, v)| public(v.clone())).collect();
    let removed: Vec<_> = a.keys().filter(|id| !b.contains_key(*id)).collect();
    let changed: Vec<_> = b
        .iter()
        .filter(|(id, v)| a.get(*id).is_some_and(|old| old != *v))
        .map(|(id, v)| {
            let mut fields = json!({"id":id});
            for (k, val) in v.as_object().unwrap() {
                if a[id][k] != *val {
                    fields[k] = val.clone();
                }
            }
            if fields.as_object_mut().unwrap().remove("geometry_digest").is_some() {
                fields["geometry_changed"] = json!(true);
            }
            fields
        })
        .collect();
    let before: BTreeMap<_, _> =
        a.iter().filter(|(id, v)| b.get(*id) != Some(*v)).map(|(id, v)| (id.clone(), v.clone())).collect();
    let board_changes: serde_json::Map<String, Value> =
        bh.as_object().unwrap().iter().filter(|(k, v)| ah[*k] != **v).map(|(k, v)| (k.clone(), v.clone())).collect();
    // pages are not objects; their identity changes are listed beside the object lists (slice 3)
    let page_ids = |h: &Value| -> Vec<String> {
        h["artboards"].as_array().into_iter().flatten().filter_map(|a| a["id"].as_str().map(str::to_owned)).collect()
    };
    let (pa, pb) = (page_ids(ah), page_ids(bh));
    let artboards_created: Vec<_> = pb.iter().filter(|id| !pa.contains(id)).collect();
    let artboards_removed: Vec<_> = pa.iter().filter(|id| !pb.contains(id)).collect();
    let mut out = json!({"before":before,"before_header":(ah!=bh).then_some(ah),"from":from,"rev":rev,"created":created,"removed":removed,"changed":changed,"board_changes":(!board_changes.is_empty()).then_some(board_changes)});
    if !artboards_created.is_empty() {
        out["artboards_created"] = json!(artboards_created);
    }
    if !artboards_removed.is_empty() {
        out["artboards_removed"] = json!(artboards_removed);
    }
    out
}
/// The sole readable projection consumed by both CLI and MCP. All document text is JSON escaped.
pub fn compact(r: &Reply) -> String {
    let v = r.result.as_ref();
    if let Some(e) = &r.error {
        let mut out = format!("error {}", e.code);
        if let Some(id) = &r.request_id {
            out.push_str(&format!(" request_id={id}"));
        }
        if let Some(board) = &r.board {
            out.push_str(&format!(" board={board}"));
        }
        if let Some(rev) = r.rev {
            out.push_str(&format!(" rev={rev}"));
        }
        if let Some(index) = e.op_index {
            out.push_str(&format!(" op_index={index}"));
        }
        if !e.ids.is_empty() {
            out.push_str(&format!(" ids={}", json!(e.ids)));
        }
        if let Some(rev) = e.expected_rev {
            out.push_str(&format!(" expected_rev={rev}"));
        }
        if let Some(rev) = e.actual_rev {
            out.push_str(&format!(" actual_rev={rev}"));
        }
        if let Some(digest) = &e.digest {
            out.push_str(&format!(" digest={digest}"));
        }
        out.push_str(&format!(" retryable={} reason={} undo_steps=0", e.retryable, json!(e.reason)));
        return out;
    }
    let v = v.expect("successful reply has structured result");
    let Some(board) = &r.board else {
        return text_value(v);
    };
    let mut out = if let Some(id) = &r.request_id {
        let from = v.get("from").map(|from| format!(" from={}", text_value(from))).unwrap_or_default();
        format!("ok {id} board={board}{from} rev={} undo_steps={}\n", r.rev.unwrap_or(0), r.undo_steps)
    } else {
        format!("board {board} rev={}", r.rev.unwrap_or(0))
    };
    if v.get("name").is_some() && v.get("counts").is_some() {
        out.push_str(&format!(" name={} units=pt dirty={}\ncounts paths={} groups={} layers={} artboards={}\nbounds={} selection={} selection_rev={}\nartboards={}\ndetail={}\n",v["name"],v["dirty"],v["counts"]["paths"],v["counts"]["groups"],v["counts"]["layers"],v["counts"]["artboards"],text_value(&v["bounds"]),text_value(&v["selection"]),v["selection_rev"],text_value(&v["artboards"]),v["detail"].as_array().unwrap().iter().filter_map(Value::as_str).collect::<Vec<_>>().join(",")));
    } else if !out.ends_with('\n') {
        out.push('\n');
    }
    for key in ["objects", "changed", "created"] {
        if let Some(items) = v[key].as_array() {
            for o in items {
                let id = o["id"].as_str().unwrap_or("?");
                if key == "objects" {
                    out.push_str(&format!("{id} {}", o["kind"].as_str().unwrap_or("object")));
                } else {
                    out.push_str(&format!("{key} {id}"));
                }
                for field in [
                    "parent",
                    "bounds",
                    "fill",
                    "stroke",
                    "stroke_width",
                    "opacity",
                    "name",
                    "hidden",
                    "locked",
                    "geometry_changed",
                    "geometry",
                ] {
                    if let Some(value) = o.get(field) {
                        let value = if ["fill", "stroke", "parent"].contains(&field) {
                            value.as_str().unwrap_or("none").to_owned()
                        } else {
                            text_value(value)
                        };
                        out.push_str(&format!(" {field}={value}"));
                    }
                }
                out.push('\n');
            }
        }
    }
    if let Some(created) = v["created"].as_array() {
        out.push_str(&format!(
            "created={}\n",
            json!(created.iter().filter_map(|o| o["id"].as_str()).collect::<Vec<_>>())
        ));
    }
    for key in [
        "metadata",
        "artboards",
        "board_changes",
        "removed",
        "artboards_created",
        "artboards_removed",
        "active_artboard",
        "artboard",
        "selection",
        "selection_count",
        "selection_more",
        "selection_rev",
        "more",
        "cursor",
        "counts",
        "detail_request",
        "resync_required",
        "history",
        "history_steps",
        "locals",
        "width",
        "height",
        "mime_type",
        "preview",
        "path",
    ] {
        if let Some(value) = v.get(key) {
            if (key == "cursor" && value.is_null())
                || (key == "selection_more" && value == false)
                || (key == "board_changes" && value.is_null())
                || (key == "counts" && v.get("name").is_some())
                || ((key == "selection" || key == "selection_rev" || key == "artboards") && v.get("name").is_some())
            {
                continue;
            }
            out.push_str(&format!("{key}={}\n", text_value(value)));
        }
    }
    out
}
fn text_value(v: &Value) -> String {
    fn integers(v: &mut Value) {
        match v {
            Value::Number(n) if n.is_f64() => {
                let f = n.as_f64().unwrap();
                if f.fract() == 0.0 && f.abs() < 1.0e15 {
                    *v = json!(f as i64);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(integers),
            Value::Object(o) => o.values_mut().for_each(integers),
            _ => {}
        }
    }
    let mut v = v.clone();
    integers(&mut v);
    v.to_string()
}

pub(crate) fn target_error(e: varos_core::bridge::TargetError) -> Error {
    let code = match e.code {
        TargetErrorCode::NotFound => "not_found",
        TargetErrorCode::LockedTarget => "locked_target",
        TargetErrorCode::HiddenTarget => "hidden_target",
        TargetErrorCode::InvalidArgument => "invalid_argument",
        TargetErrorCode::Busy => "busy",
        TargetErrorCode::Cancelled => "cancelled",
    };
    let mut error = Error::new(code, e.reason).at(e.index);
    error.ids = e.ids.iter().map(|id| format!("path:{id}")).collect();
    error
}

#[cfg(test)]
mod observation_tests {
    use super::*;
    struct Fake(Editor);
    impl Host for Fake {
        fn boards(&self) -> Vec<BoardInfo> {
            vec![BoardInfo { board: "b1".into(), name: String::new(), rev: self.0.rev, dirty: false, active: true }]
        }
        fn prepare(&mut self, _: &str, _: bool) -> Result<(), Error> {
            Ok(())
        }
        fn access(&mut self, _: &str) -> Result<BoardAccess<'_>, Error> {
            Ok(BoardAccess { editor: &mut self.0, dirty: false })
        }
    }
    #[test]
    fn expired_digest_resend_refreshes_edit_and_history_grants() {
        for history in [false, true] {
            let mut host = Fake(Editor::new());
            let pid = host
                .0
                .try_execute_created(varos_core::EditCommand::AddShape {
                    kind: varos_core::model::ShapeKind::Rect,
                    bounds: [0.0, 0.0, 20.0, 10.0],
                    parent: None,
                    fill: Some([1.0, 0.0, 0.0, 1.0]),
                    stroke: None,
                    stroke_width: 0.0,
                    opacity: 1.0,
                    name: None,
                })
                .unwrap();
            let context = Context {
                client: "test".into(),
                epoch: "test".into(),
                read: true,
                edit: true,
                allow_history: true,
                allow_destructive: true,
            };
            let mut service = Service::new("test".into());
            let args = if history {
                json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":host.0.rev,"action":"undo"})
            } else {
                json!({"api":"1.0","request_id":"r1","board":"b1","expected_rev":host.0.rev,"ops":[{"verb":"delete","ids":[format!("path:{pid}")]}]})
            };
            let mut request = crate::mcp::decode_tool(if history { "history" } else { "edit" }, args).unwrap();
            let cancelled = AtomicBool::new(false);
            let challenge = service.handle(&mut host, &context, request.clone(), &cancelled);
            let digest = challenge.error.unwrap().digest.clone();
            match &mut request {
                Request::History(v) => v.digest = digest,
                Request::Edit(v) => v.digest = digest,
                _ => unreachable!(),
            }
            for grant in service.grants.values_mut() {
                grant.expires = Instant::now() - Duration::from_secs(1);
            }
            let before = host.0.doc.clone();
            let fresh = service.handle(&mut host, &context, request.clone(), &cancelled).error.unwrap();
            assert_eq!(fresh.code, "confirmation_required");
            assert!(fresh.digest.is_some());
            assert!(!fresh.ids.is_empty());
            assert_eq!(fresh.actual_rev, Some(host.0.rev));
            assert_eq!(host.0.doc, before);
            match &mut request {
                Request::History(v) => v.digest = fresh.digest.clone(),
                Request::Edit(v) => v.digest = fresh.digest.clone(),
                _ => unreachable!(),
            }
            assert!(service.handle(&mut host, &context, request, &cancelled).ok);
        }
    }

    #[test]
    fn snapshot_worker_checkpoints_cancel_and_pin_owned_revision() {
        let job = SnapshotJob { document: Document::default(), rev: 7, size: [80, 40], artboard: None };
        let reply = job.render(&AtomicBool::new(true));
        assert_eq!(reply.error.unwrap().code, "cancelled");
        let job = SnapshotJob { document: Document::default(), rev: 7, size: [80, 40], artboard: None };
        let reply = job.render(&AtomicBool::new(false));
        assert_eq!(reply.result.unwrap()["rev"], 7);
    }

    #[test]
    fn unchanged_observation_does_not_fingerprint() {
        let mut host = Fake(Editor::new());
        let mut service = Service::new("test".into());
        service.observe(&mut host);
        assert_eq!(service.fingerprints, 1);
        for _ in 0..100 {
            service.observe(&mut host);
        }
        assert_eq!(service.fingerprints, 1);
        host.0.execute(varos_core::EditCommand::ToggleSnapping);
        service.observe(&mut host);
        assert_eq!(service.fingerprints, 2, "settings outside Editor.rev must still be observed");
        host.0.execute(varos_core::EditCommand::SetBoardName("changed".into()));
        service.observe(&mut host);
        assert_eq!(service.fingerprints, 3);
    }
}
