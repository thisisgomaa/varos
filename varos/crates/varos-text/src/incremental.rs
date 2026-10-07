//! Bounded paragraph cache with stable prefix/suffix identities. Each paragraph
//! keeps a text-revision analysis and fitted lines (`converge`); an edited
//! paragraph re-fits only from its first affected line until line starts converge.
use crate::converge::{Cancel, ParagraphState, WorkCounters};
use crate::*;
use std::ops::Range;
use std::rc::Rc;
#[derive(Clone, Debug, Default)]
pub struct Counters {
    /// Paragraphs whose cached layout was reused unchanged.
    pub hits: usize,
    /// Paragraphs that needed any layout work (incremental, width refit or full).
    pub reshaped: usize,
    pub evicted: usize,
    pub cache_bytes: usize,
    /// Fine-grained work inside the updated paragraphs.
    pub work: WorkCounters,
}
#[derive(Clone, PartialEq)]
struct Key {
    revision: u64,
    settings: String,
}
struct Entry {
    id: u64,
    revision: u64,
    text: String,
    key: Option<Key>,
    state: Option<Rc<ParagraphState>>,
    bytes: usize,
}
pub struct Incremental {
    engine: Engine,
    entries: Vec<Entry>,
    next_id: u64,
    limit: usize,
    pub counters: Counters,
    source: String,
}
impl Incremental {
    pub fn new(fonts: FontSet, limit: usize) -> Result<Self, &'static str> {
        Ok(Self {
            engine: Engine::new(fonts)?,
            entries: vec![],
            next_id: 0,
            limit,
            counters: Counters::default(),
            source: String::new(),
        })
    }
    pub fn shape_span_cache_stats(&self) -> (usize, usize) {
        self.engine.shape_span_cache_stats()
    }
    pub fn engine_cache_bytes(&self) -> usize {
        self.engine.cache_bytes()
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn identities(&self) -> Vec<u64> {
        self.entries.iter().map(|e| e.id).collect()
    }
    /// Cancellation/refusal leaves source and paragraph cache identities unchanged.
    /// `cancel` is polled cooperatively inside analysis, shaping, line fitting and
    /// output assembly (see `Counters::work.cancel_checks`); nothing is published
    /// and no cached state is mutated before the final commit.
    pub fn layout(&mut self, r: &Request<'_>, cancel: impl Fn() -> bool) -> Result<Layout, &'static str> {
        let mut cancel = Cancel::new(&cancel);
        let result = self.layout_inner(r, &mut cancel);
        cancel.finish();
        self.counters.work.cancel_checks = cancel.checks;
        (self.counters.work.max_poll_gap_ms, self.counters.work.max_poll_gap_at) = cancel.max_gap();
        self.counters.work.stretches = std::mem::take(&mut cancel.stretches);
        result
    }
    fn layout_inner(&mut self, r: &Request<'_>, cancel: &mut Cancel<'_>) -> Result<Layout, &'static str> {
        cancel.check("doc_entry")?;
        if r.text.len() > 1_048_576 {
            return Err("resource limit");
        }
        // Validate complete source ranges before clipping them to paragraphs.
        // Invalid remote style/language ranges must not disappear on a cache hit.
        self.engine.validate_faces(r)?;
        crate::engine::validate_request(r)?;
        cancel.check("doc_validate")?;
        let texts = paragraphs(r.text);
        cancel.check("doc_paragraphs")?;
        let mut prefix = 0;
        while prefix < texts.len() && prefix < self.entries.len() && texts[prefix] == self.entries[prefix].text {
            prefix += 1;
        }
        let mut suffix = 0;
        while suffix < texts.len() - prefix
            && suffix < self.entries.len() - prefix
            && texts[texts.len() - 1 - suffix] == self.entries[self.entries.len() - 1 - suffix].text
        {
            suffix += 1;
        }
        // Build requests and all misses first, so cancellation cannot publish partial output.
        let mut pending = Vec::new();
        let mut offset = 0;
        self.counters = Counters::default();
        for (i, text) in texts.iter().enumerate() {
            cancel.check("paragraph")?;
            let old_index = if i < prefix {
                Some(i)
            } else if i >= texts.len() - suffix {
                Some(self.entries.len() - (texts.len() - i))
            } else if texts.len() == self.entries.len() {
                Some(i)
            } else {
                None
            };
            let mut req = r.clone();
            req.text = text;
            req.styles = r
                .styles
                .iter()
                .filter_map(|s| {
                    let start = s.range.start.max(offset);
                    let end = s.range.end.min(offset + text.len());
                    if start >= end {
                        return None;
                    }
                    let mut s = s.clone();
                    s.range = start - offset..end - offset;
                    s.paint = 0;
                    Some(s)
                })
                .collect();
            req.language_runs = r
                .language_runs
                .iter()
                .filter_map(|run| {
                    let start = run.range.start.max(offset);
                    let end = run.range.end.min(offset + text.len());
                    if start >= end {
                        return None;
                    }
                    let mut run = run.clone();
                    run.range = start - offset..end - offset;
                    Some(run)
                })
                .collect();
            let settings = format!(
                "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
                req.direction,
                req.language,
                req.script,
                req.size.to_bits(),
                req.width.map(f32::to_bits),
                req.face,
                req.end_align,
                (&req.styles, &req.features, &req.language_runs)
            );
            let rev = old_index.map_or(0, |j| self.entries[j].revision + u64::from(self.entries[j].text != *text));
            let key = Key { revision: rev, settings };
            let hit =
                old_index.filter(|j| self.entries[*j].key.as_ref() == Some(&key) && self.entries[*j].state.is_some());
            let (state, bytes) = if let Some(j) = hit {
                self.counters.hits += 1;
                (self.entries[j].state.clone().unwrap(), self.entries[j].bytes)
            } else {
                self.counters.reshaped += 1;
                let old = old_index.and_then(|j| self.entries[j].state.as_deref());
                let state = ParagraphState::layout(&mut self.engine, old, &req, cancel, &mut self.counters.work)?;
                let bytes = state.bytes();
                (Rc::new(state), bytes)
            };
            pending.push((old_index, key, state, bytes));
            offset += text.len();
        }
        cancel.check("combine")?;
        let mut combined = Layout {
            source: r.text.into(),
            lines: vec![],
            carets: vec![],
            issues: vec![],
            levels: vec![],
            ink_bounds: None,
        };
        let mut offset = 0;
        let mut top = 0.;
        let mut new_entries = Vec::new();
        let count = pending.len();
        for (i, (old, key, state, state_bytes)) in pending.into_iter().enumerate() {
            let mut local = state.output(cancel)?;
            if i + 1 < count && local.lines.last().is_some_and(|l| l.range.is_empty()) {
                local.lines.pop();
                local.carets.retain(|c| c.line < local.lines.len());
            }
            let line_offset = combined.lines.len();
            for mut line in local.lines {
                let local_baseline = line.baseline;
                line.baseline = top + line.ascent;
                for g in &mut line.glyphs {
                    g.cluster.start += offset;
                    g.cluster.end += offset;
                    g.y = line.baseline + (g.y - local_baseline);
                }
                line.range.start += offset;
                line.range.end += offset;
                top += (line.ascent + line.descent).max(r.size * 1.4);
                combined.lines.push(line);
            }
            cancel.check("combine_lines")?;
            for (k, mut c) in local.carets.into_iter().enumerate() {
                c.byte += offset;
                c.line += line_offset;
                combined.carets.push(c);
                if k % 65536 == 65535 {
                    cancel.check("combine_carets")?;
                }
            }
            for issue in local.issues {
                combined.issues.push(match issue {
                    Issue::Substituted { range, requested, actual } => {
                        Issue::Substituted { range: shift(range, offset), requested, actual }
                    }
                    Issue::UnsupportedCluster(range) => Issue::UnsupportedCluster(shift(range, offset)),
                    Issue::Overflow(line) => Issue::Overflow(line + line_offset),
                    other => other,
                });
            }
            combined.levels.extend(local.levels);
            cancel.check("combine_levels")?;
            let id = old.map_or_else(
                || {
                    let id = self.next_id;
                    self.next_id += 1;
                    id
                },
                |j| self.entries[j].id,
            );
            let bytes = state_bytes + key.settings.capacity() + std::mem::size_of::<Key>();
            let keep = self.counters.cache_bytes + bytes <= self.limit;
            if keep {
                self.counters.cache_bytes += bytes;
            } else {
                self.counters.evicted += 1;
            }
            let state = if keep {
                Some(state)
            } else {
                // Dropping an over-budget paragraph state is itself sliced work.
                drop(state);
                cancel.check("evict_drop")?;
                None
            };
            new_entries.push(Entry {
                id,
                revision: key.revision,
                text: texts[i].into(),
                key: keep.then_some(key),
                state,
                bytes: state_bytes,
            });
            offset += texts[i].len();
        }
        cancel.check("bounds")?;
        // Bounds use the engine's outline cache; translated placement is authoritative.
        combined.ink_bounds = self.engine.bounds_for(&combined.lines)?;
        self.entries = new_entries;
        self.source = r.text.into();
        Ok(combined)
    }
}
fn shift(r: Range<usize>, n: usize) -> Range<usize> {
    r.start + n..r.end + n
}
pub fn layout_bytes(l: &Layout) -> usize {
    std::mem::size_of::<Layout>()
        + l.source.capacity()
        + l.levels.capacity()
        + l.lines.capacity() * std::mem::size_of::<Line>()
        + l.lines.iter().map(|line| line.glyphs.capacity() * std::mem::size_of::<Glyph>()).sum::<usize>()
        + l.carets.capacity() * std::mem::size_of::<Caret>()
        + l.issues.capacity() * std::mem::size_of::<Issue>()
}
