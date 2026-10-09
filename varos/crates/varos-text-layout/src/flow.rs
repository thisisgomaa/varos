//! Lane H: shared CPU shape-frame composition and arc-length glyph placement.
use super::*;
use varos_core::typography::{Binding, Frame, PathEffect};

/// Horizontal even-odd intervals through all contours, including holes/disjoint islands.
pub fn intervals(rings: &[Vec<[f32; 2]>], y: f32) -> Vec<[f32; 2]> {
    let mut xs = Vec::new();
    for ring in rings {
        if ring.len() < 3 {
            continue;
        }
        for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()) {
            if (a[1] <= y && b[1] > y) || (b[1] <= y && a[1] > y) {
                xs.push(a[0] + (y - a[1]) * (b[0] - a[0]) / (b[1] - a[1]));
            }
        }
    }
    xs.sort_by(f32::total_cmp);
    xs.as_chunks::<2>().0.iter().filter_map(|p| (p[1] > p[0]).then_some([p[0], p[1]])).collect()
}
fn intersect(a: &[[f32; 2]], b: &[[f32; 2]]) -> Vec<[f32; 2]> {
    a.iter()
        .flat_map(|a| {
            b.iter().filter_map(move |b| {
                let l = a[0].max(b[0]);
                let r = a[1].min(b[1]);
                (r > l).then_some([l, r])
            })
        })
        .collect()
}
/// Conservative full ink-band containment, checking every polygon slope transition.
pub fn band(rings: &[Vec<[f32; 2]>], top: f32, bottom: f32, inset: f32) -> Vec<[f32; 2]> {
    let top = top - inset;
    let bottom = bottom + inset;
    let mut ys = vec![top, bottom];
    for p in rings.iter().flatten() {
        if p[1] > top && p[1] < bottom {
            ys.extend([p[1] - 0.0001, p[1] + 0.0001]);
        }
    }
    let mut out = intervals(rings, ys[0]);
    for y in ys.into_iter().skip(1) {
        out = intersect(&out, &intervals(rings, y));
    }
    out.into_iter().filter_map(|[l, r]| (r - l > 2. * inset).then_some([l + inset, r - inset])).collect()
}
#[derive(Clone, Debug)]
pub struct ArcPath {
    points: Vec<[f32; 2]>,
    lengths: Vec<f32>,
    pub length: f32,
}
impl ArcPath {
    pub fn new(mut points: Vec<[f32; 2]>, closed: bool) -> Result<Self, String> {
        if points.len() > 65536 || points.iter().flatten().any(|v| !v.is_finite()) {
            return Err("invalid baseline geometry".into());
        }
        points.dedup();
        if closed && points.len() > 1 && points.first() != points.last() {
            if let Some(p) = points.first().copied() {
                points.push(p);
            }
        }
        let mut lengths = vec![0.];
        let mut length = 0.;
        for p in points.windows(2) {
            length += varos_core::geom::dist(p[0], p[1]);
            lengths.push(length);
        }
        if !length.is_finite() {
            return Err("invalid baseline length".into());
        }
        Ok(Self { points, lengths, length })
    }
    pub fn nearest(&self, point: [f32; 2]) -> f32 {
        let mut best = (f32::INFINITY, 0.);
        for (i, pair) in self.points.windows(2).enumerate() {
            let a = pair[0];
            let b = pair[1];
            let dx = b[0] - a[0];
            let dy = b[1] - a[1];
            let len = dx * dx + dy * dy;
            if len <= 0. {
                continue;
            }
            let t = (((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / len).clamp(0., 1.);
            let distance = varos_core::geom::dist(point, [a[0] + t * dx, a[1] + t * dy]);
            if distance < best.0 {
                best = (distance, self.lengths[i] + t * len.sqrt());
            }
        }
        best.1
    }
    pub fn sample(&self, d: f32) -> Option<([f32; 2], [f32; 2])> {
        if self.length <= 0. || !d.is_finite() || d < 0. || d > self.length {
            return None;
        }
        let i = self.lengths.partition_point(|x| *x <= d).saturating_sub(1).min(self.points.len().saturating_sub(2));
        let a = *self.points.get(i)?;
        let b = *self.points.get(i + 1)?;
        let len = self.lengths[i + 1] - self.lengths[i];
        if len <= 0. {
            return None;
        }
        let t = (d - self.lengths[i]) / len;
        Some(([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])], [(b[0] - a[0]) / len, (b[1] - a[1]) / len]))
    }
}
fn geometry(doc: &Document, id: u32) -> Result<Vec<Vec<[f32; 2]>>, String> {
    let i = doc.pidx(id).ok_or("missing text boundary")?;
    let g = varos_core::flatten::flatten_path(doc, i, 4.);
    let mut rings = vec![g.outline];
    rings.extend(g.holes);
    if rings.iter().map(Vec::len).sum::<usize>() > 65536 {
        return Err("text boundary budget exceeded".into());
    }
    Ok(rings)
}
pub(crate) fn tail(text: &TextBox, start: usize) -> TextBox {
    let mut result = text.clone();
    let mut at = 0;
    result.runs = text
        .runs
        .iter()
        .filter_map(|r| {
            let end = at + r.text.len();
            let offset = start.saturating_sub(at);
            at = end;
            (offset < r.text.len()).then(|| Run { text: r.text[offset..].into(), style: r.style.clone() })
        })
        .collect();
    if result.runs.is_empty() {
        if let Some(r) = text.runs.first() {
            result.runs.push(Run { text: String::new(), style: r.style.clone() });
        }
    }
    result
}
pub(crate) struct Cache {
    doc: Document,
    bucket: i32,
    outputs: HashMap<u32, (TextBox, Composed)>,
}
impl TextLayout {
    /// Resolves live path geometry/styles and walks a thread from its one logical source.
    pub fn compose_document(&mut self, doc: &Document, text: &TextBox, zoom: f32) -> Result<Composed, String> {
        if doc.typography.is_empty() {
            return Ok(self.compose(text, zoom)?.clone());
        }
        if !zoom.is_finite() || zoom <= 0. {
            return Err("invalid text zoom".into());
        }
        let bucket = (zoom.log2().ceil() as i32).clamp(-8, 12);
        if self.flow_cache.as_ref().is_none_or(|c| c.doc != *doc || c.bucket != bucket) {
            self.flow_cache = Some(Cache { doc: doc.clone(), bucket, outputs: HashMap::new() });
        }
        if let Some((_, output)) =
            self.flow_cache.as_ref().and_then(|c| c.outputs.get(&text.id)).filter(|(t, _)| t == text)
        {
            return Ok(output.clone());
        }
        let output = self.compose_document_uncached(doc, text, 2f32.powi(bucket))?;
        if let Some(cache) = &mut self.flow_cache {
            let count = |c: &Composed| c.paths.iter().map(path_vertices).sum::<usize>();
            if cache.outputs.len() >= 256
                || cache.outputs.values().map(|(_, c)| count(c)).sum::<usize>() + count(&output) > 500_000
            {
                cache.outputs.clear();
            }
            cache.outputs.insert(text.id, (text.clone(), output.clone()));
        }
        Ok(output)
    }
    fn compose_document_uncached(&mut self, doc: &Document, text: &TextBox, zoom: f32) -> Result<Composed, String> {
        if doc.typography.is_empty() {
            return Ok(self.compose(text, zoom)?.clone());
        }
        if !zoom.is_finite() || zoom <= 0. {
            return Err("invalid text zoom".into());
        }
        let mut root = text.id;
        let mut seen = std::collections::BTreeSet::new();
        while let Some((&id, _)) = doc.typography.frames.iter().find(|(_, f)| f.next == Some(root)) {
            if !seen.insert(root) {
                return Err("text thread cycle".into());
            }
            root = id;
        }
        let original = if root == text.id {
            text
        } else {
            doc.text_boxes.iter().find(|t| t.id == root).ok_or("missing text source")?
        };
        let source = doc.typography.resolved(original)?;
        let mut id = root;
        let mut consumed = 0;
        seen.clear();
        loop {
            if !seen.insert(id) {
                return Err("text thread cycle".into());
            }
            let frame_text = if id == text.id {
                text
            } else {
                doc.text_boxes.iter().find(|t| t.id == id).ok_or("missing text frame")?
            };
            let mut frame = doc.typography.frames.get(&id).cloned().unwrap_or_default();
            if frame.features.is_empty() {
                frame.features = doc.typography.frames.get(&root).map(|f| f.features.clone()).unwrap_or_default();
            }
            let mut remaining = tail(&source, consumed);
            remaining.id = id;
            remaining.box_kind = frame_text.box_kind;
            remaining.frame = frame_text.frame;
            let (mut output, mut used) = self.compose_frame(doc, &remaining, &frame, zoom)?;
            self.constrain_split(doc, &remaining, &frame, &mut output, &mut used, zoom)?;
            // Every frame keeps source-byte identity in the single authored story.
            for line in &mut output.layout.lines {
                line.range.start += consumed;
                line.range.end += consumed;
                for g in &mut line.glyphs {
                    g.cluster.start += consumed;
                    g.cluster.end += consumed;
                }
            }
            for caret in &mut output.layout.carets {
                caret.byte += consumed;
            }
            output.layout.source = source.source();
            if id == text.id {
                return Ok(output);
            }
            consumed += used;
            id = frame.next.ok_or("broken text thread")?;
        }
    }
    pub(crate) fn compose_frame(
        &mut self,
        doc: &Document,
        text: &TextBox,
        frame: &Frame,
        zoom: f32,
    ) -> Result<(Composed, usize), String> {
        match frame.binding {
            Some(Binding::Area { path, inset }) => self.area(text, frame, geometry(doc, path)?, inset, zoom),
            Some(Binding::Path { path, start, end, offset, flip, effect }) => {
                let rings = geometry(doc, path)?;
                let closed = doc.paths.iter().find(|p| p.id == path).is_some_and(|p| p.closed);
                let arc = ArcPath::new(rings.into_iter().next().unwrap_or_default(), closed)?;
                self.on_path(text, frame, &arc, [start, end, offset], flip, effect, zoom)
            }
            None => {
                if let TextBoxKind::Area([x, y, w, h]) = text.box_kind {
                    return self.area(
                        text,
                        frame,
                        vec![vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]]],
                        0.,
                        zoom,
                    );
                }
                let layout = self.shape_features(text, &frame.features)?;
                let origin = [text.frame[0], text.frame[1] - layout.lines.first().map_or(0., |l| l.baseline)];
                let paths = self.flow_outlines(text, &layout, origin, zoom)?;
                Ok((Composed { layout, paths, origin, overset: false }, text.source().len()))
            }
        }
    }
    pub(crate) fn flow_outlines(
        &self,
        text: &TextBox,
        layout: &Layout,
        origin: [f32; 2],
        zoom: f32,
    ) -> Result<Vec<Path>, String> {
        let mut paths = Vec::new();
        let mut vertices = 0;
        for glyph in layout.lines.iter().flat_map(|l| &l.glyphs) {
            let outline = varos_text::outlines::glyph_outline(self.fonts(), glyph)?;
            let out = outline_paths(&outline.commands, origin, fill_at(text, glyph.cluster.start), 0.1 / zoom)?;
            vertices += out.iter().map(path_vertices).sum::<usize>();
            if vertices > 250_000 {
                return Err("text outline budget exceeded".into());
            }
            paths.extend(out);
        }
        Ok(paths)
    }
    fn area(
        &mut self,
        text: &TextBox,
        frame: &Frame,
        rings: Vec<Vec<[f32; 2]>>,
        inset: f32,
        zoom: f32,
    ) -> Result<(Composed, usize), String> {
        let mut probe = text.clone();
        probe.box_kind = TextBoxKind::Point;
        probe.para.align = Alignment::Left;
        let measured = self.shape_features(&probe, &frame.features)?;
        let ascent = measured.lines.iter().map(|l| l.ascent).fold(0., f32::max);
        let descent = measured.lines.iter().map(|l| l.descent).fold(0., f32::max);
        let em = text.runs.iter().map(|r| r.style.size).fold(0., f32::max);
        let leading = (em * text.para.line_height).max(ascent + descent).max(0.1);
        let min = rings.iter().flatten().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max = rings.iter().flatten().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
        let mut out = measured.clone();
        out.lines.clear();
        out.carets.clear();
        out.issues.clear();
        let source = text.source();
        let mut consumed = 0;
        let mut baseline = min + inset + ascent;
        let mut count = 0;
        while baseline + descent + inset <= max && consumed < source.len() {
            count += 1;
            if count > 4096 {
                return Err("area text line budget exceeded".into());
            }
            let mut spans = band(&rings, baseline - ascent, baseline + descent, inset);
            if text.para.direction != Direction::Ltr {
                spans.reverse();
            }
            for [left, right] in spans {
                if consumed == source.len() {
                    break;
                }
                let mut rest = tail(text, consumed);
                rest.box_kind = TextBoxKind::Area([0., 0., right - left, 1e6]);
                let layout = self.shape_features(&rest, &frame.features)?;
                let Some(mut line) = layout.lines.first().cloned() else {
                    continue;
                };
                if line.width > right - left + 0.01 {
                    continue;
                }
                let used = layout.lines.get(1).map_or(rest.source().len(), |l| l.range.start);
                if used == 0 {
                    continue;
                }
                let dy = baseline - line.baseline;
                line.baseline = baseline;
                line.empty_caret_x += left;
                line.range.start += consumed;
                line.range.end += consumed;
                for g in &mut line.glyphs {
                    g.x += left;
                    g.y += dy;
                    g.cluster.start += consumed;
                    g.cluster.end += consumed;
                }
                let index = out.lines.len();
                for c in layout.carets.iter().filter(|c| c.line == 0) {
                    let mut c = c.clone();
                    c.line = index;
                    c.x += left;
                    c.byte += consumed;
                    out.carets.push(c);
                }
                for issue in &layout.issues {
                    use varos_text::Issue;
                    match issue {
                        Issue::KashidaInserted { byte, count } if *byte < used => {
                            out.issues.push(Issue::KashidaInserted { byte: byte + consumed, count: *count })
                        }
                        Issue::JustificationResidual { line: 0, residual } => {
                            out.issues.push(Issue::JustificationResidual { line: index, residual: *residual })
                        }
                        Issue::UnsupportedCluster(range) if range.start < used => {
                            out.issues.push(Issue::UnsupportedCluster(range.start + consumed..range.end + consumed))
                        }
                        _ => {}
                    }
                }
                out.lines.push(line);
                consumed += used;
            }
            baseline += leading;
        }
        let paths = self.flow_outlines(text, &out, [0., 0.], zoom)?;
        out.ink_bounds = bounds(&paths);
        Ok((Composed { layout: out, paths, origin: [0., 0.], overset: consumed < source.len() }, consumed))
    }
    #[allow(clippy::too_many_arguments)]
    fn on_path(
        &mut self,
        text: &TextBox,
        frame: &Frame,
        arc: &ArcPath,
        brackets: [f32; 3],
        flip: bool,
        effect: PathEffect,
        zoom: f32,
    ) -> Result<(Composed, usize), String> {
        let [start, end, offset] = brackets;
        let end = (arc.length - end).max(start);
        let width = end - start;
        let mut probe = text.clone();
        probe.box_kind = TextBoxKind::Point;
        probe.para.align = Alignment::Left;
        let mut layout = self.shape_features(&probe, &frame.features)?;
        let mut paths = Vec::new();
        let mut overset = layout.lines.len() > 1;
        let mut used = 0;
        let mut vertices = 0;
        if let Some(line) = layout.lines.first() {
            let align = match text.para.align {
                Alignment::Right => width - line.width,
                Alignment::Centre => (width - line.width) * 0.5,
                _ => 0.,
            }
            .max(0.);
            let map = crate::path_mapping::PathMap {
                arc: arc.clone(),
                start,
                end,
                align,
                baseline: line.baseline,
                offset,
                flip,
                effect,
            };
            for g in &line.glyphs {
                let center = g.x + g.advance * 0.5 + align;
                if center - g.advance * 0.5 < 0. || center + g.advance * 0.5 > width {
                    overset = true;
                    continue;
                }
                let anchor = line
                    .glyphs
                    .iter()
                    .find(|base| base.cluster == g.cluster && base.advance > 0.)
                    .map_or(g.x + g.advance * 0.5, |base| base.x + base.advance * 0.5);
                if map.at_anchor([anchor, line.baseline], anchor).is_none() {
                    overset = true;
                    continue;
                }
                let transform = |q| map.at_anchor(q, anchor).unwrap_or(q);
                let outline = varos_text::outlines::glyph_outline(self.fonts(), g)?;
                let commands: Vec<_> = outline
                    .commands
                    .iter()
                    .map(|c| match *c {
                        Command::Move(p) => Command::Move(transform(p)),
                        Command::Line(p) => Command::Line(transform(p)),
                        Command::Cubic(a, b, p) => Command::Cubic(transform(a), transform(b), transform(p)),
                        Command::Close => Command::Close,
                    })
                    .collect();
                let out = outline_paths(&commands, [0., 0.], fill_at(text, g.cluster.start), 0.1 / zoom)?;
                vertices += out.iter().map(path_vertices).sum::<usize>();
                if vertices > 250_000 {
                    return Err("path text outline budget exceeded".into());
                }
                paths.extend(out);
                used = used.max(g.cluster.end);
            }
        }
        layout.ink_bounds = bounds(&paths);
        Ok((Composed { layout, paths, origin: [0., 0.], overset }, used))
    }
}
fn fill_at(text: &TextBox, byte: usize) -> [f32; 4] {
    let mut at = 0;
    for r in &text.runs {
        at += r.text.len();
        if byte < at {
            return r.style.fill;
        }
    }
    text.runs.first().map_or([0., 0., 0., 1.], |r| r.style.fill)
}
pub(crate) fn bounds(paths: &[Path]) -> Option<[f32; 4]> {
    paths.iter().flat_map(|p| p.anchors.iter().chain(p.holes.iter().flatten())).map(|a| a.p).fold(None, |b, p| {
        Some(match b {
            None => [p[0], p[1], p[0], p[1]],
            Some([a, b, c, d]) => [a.min(p[0]), b.min(p[1]), c.max(p[0]), d.max(p[1])],
        })
    })
}
