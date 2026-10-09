//! Bounded native-PDF profile. Preflight runs BEFORE lopdf: classic xref only, exact entry counts,
//! no revision/encryption links, bounded direct-object complexity and depth. Stream data is skipped
//! by its direct Length, never tokenized or inflated. Foreign profiles fail closed.
use std::collections::HashSet;
use std::path::Path;

use lopdf::{Dictionary, Document, LoadOptions, Object, ObjectId};
use varos_core::format::{decode_model, read_bounded, LimitKind, Limits, LoadError, Loaded};

fn malformed(reason: &str) -> LoadError {
    LoadError::MalformedPdf(reason.into())
}
fn unsupported(reason: &str) -> LoadError {
    LoadError::UnsupportedPdf(reason.into())
}
fn bound(kind: LimitKind, found: usize, max: usize) -> Result<(), LoadError> {
    if found > max {
        Err(LoadError::TooLarge { limit: kind, found: found as u64, max: max as u64 })
    } else {
        Ok(())
    }
}

pub fn load_vrs_checked(path: &Path, limits: &Limits) -> Result<Loaded, LoadError> {
    let mut loaded = load_vrs_bytes(&read_bounded(path, limits)?, limits)?;
    loaded.blobs.document_dir = path.parent().map(Path::to_path_buf);
    for image in &loaded.doc.images {
        if image.placement == varos_core::images::PlacementMode::Link {
            if let Ok(source) = varos_core::images::links::resolve(image, path.parent(), None) {
                if let Ok(bytes) = varos_core::images::links::read_original(&source) {
                    if let Ok(decoded) = varos_core::images::codec::decode(&bytes) {
                        if loaded.doc.assets.contains(&decoded.blob.meta) {
                            let _ = loaded.blobs.insert(decoded.blob);
                        }
                    }
                }
            }
        }
    }
    Ok(loaded)
}
pub fn load_vrs_bytes(bytes: &[u8], limits: &Limits) -> Result<Loaded, LoadError> {
    if bytes.len() as u64 > limits.max_file_bytes {
        return Err(LoadError::TooLarge {
            limit: LimitKind::FileBytes,
            found: bytes.len() as u64,
            max: limits.max_file_bytes,
        });
    }
    if !bytes.starts_with(b"%PDF-") {
        return decode_model(bytes, None, limits);
    }
    let pdf = parse_pdf(bytes, limits)?;
    let catalog = pdf.catalog().map_err(|_| malformed("missing catalog"))?;
    let version = match catalog.get(b"VAROS_SchemaVersion") {
        Err(_) => None,
        Ok(o) => match resolve(&pdf, o)?.as_i64() {
            Ok(v) if v > 0 && v <= u32::MAX as i64 => Some(v as u32),
            _ => return Err(LoadError::InvalidVersion("PDF catalog".into())),
        },
    };
    // ---- Lane F: preview keys are versioned even when optional ----
    if catalog.has(b"VAROS_Preview") || catalog.has(b"VAROS_PreviewVersion") {
        if version.is_none_or(|v| v < varos_core::format::PREVIEW_FORMAT_VERSION) {
            return Err(unsupported("preview keys require native format 9"));
        }
        crate::quicklook::preview(bytes).map_err(|e| unsupported(&e))?;
    }
    let model = if let Ok(o) = catalog.get(b"VAROS_Model") { o } else { find_model(&pdf, catalog, limits)? };
    let stream = resolve(&pdf, model)?.as_stream().map_err(|_| malformed("editable model is not a stream"))?;
    if stream.dict.has(b"Filter") {
        return Err(unsupported("model encoding"));
    }
    bound(LimitKind::ModelBytes, stream.content.len(), limits.max_model_bytes)?;
    bound(LimitKind::DecodedStreams, stream.content.len(), limits.max_decoded_stream_bytes)?;
    // ---- w2-images ----
    let mut loaded = decode_model(&stream.content, version, limits)?;
    crate::images::load_assets(&pdf, catalog, &mut loaded, limits)?;
    Ok(loaded)
}

fn resolve<'a>(pdf: &'a Document, o: &'a Object) -> Result<&'a Object, LoadError> {
    pdf.dereference(o).map(|(_, o)| o).map_err(|_| malformed("broken or cyclic object reference"))
}
fn dictionary<'a>(pdf: &'a Document, o: &'a Object) -> Result<&'a Dictionary, LoadError> {
    resolve(pdf, o)?.as_dict().map_err(|_| malformed("expected a dictionary"))
}
fn find_model<'a>(pdf: &'a Document, catalog: &'a Dictionary, limits: &Limits) -> Result<&'a Object, LoadError> {
    let names = dictionary(pdf, catalog.get(b"Names").map_err(|_| LoadError::NoEmbeddedModel)?)?;
    let root = names.get(b"EmbeddedFiles").map_err(|_| LoadError::NoEmbeddedModel)?;
    let mut stack = vec![(root, 1usize)];
    let mut visited = HashSet::new();
    let mut budget = 0usize;
    let mut found = None;
    while let Some((node, depth)) = stack.pop() {
        bound(LimitKind::PdfDepth, depth, limits.max_pdf_depth)?;
        budget += 1;
        bound(LimitKind::PdfObjects, budget, limits.max_pdf_objects)?;
        if let Object::Reference(id) = node {
            if !visited.insert(*id) {
                return Err(malformed("cyclic or shared embedded-file tree"));
            }
        }
        let d = dictionary(pdf, node)?;
        if d.has(b"Names") && d.has(b"Kids") {
            return Err(malformed("ambiguous embedded-file tree"));
        }
        if let Ok(pairs) = d.get(b"Names") {
            let pairs = resolve(pdf, pairs)?.as_array().map_err(|_| malformed("invalid embedded-file names"))?;
            if pairs.len() % 2 != 0 {
                return Err(malformed("incomplete embedded-file name pair"));
            }
            budget = budget.saturating_add(pairs.len() / 2);
            bound(LimitKind::PdfObjects, budget, limits.max_pdf_objects)?;
            for pair in pairs.as_chunks::<2>().0 {
                let key = resolve(pdf, &pair[0])?.as_str().map_err(|_| malformed("invalid embedded-file name"))?;
                if key != b"model.varos.json" {
                    continue;
                }
                if found.is_some() {
                    return Err(malformed("duplicate editable models"));
                }
                let spec = dictionary(pdf, &pair[1])?;
                let ef = dictionary(pdf, spec.get(b"EF").map_err(|_| malformed("missing embedded-file stream"))?)?;
                found = Some(
                    ef.get(b"F").or_else(|_| ef.get(b"UF")).map_err(|_| malformed("missing editable model stream"))?,
                );
            }
        } else if let Ok(kids) = d.get(b"Kids") {
            let kids = resolve(pdf, kids)?.as_array().map_err(|_| malformed("invalid embedded-file children"))?;
            bound(
                LimitKind::PdfObjects,
                budget.saturating_add(stack.len()).saturating_add(kids.len()),
                limits.max_pdf_objects,
            )?;
            stack.extend(kids.iter().rev().map(|kid| (kid, depth + 1)));
        }
    }
    found.ok_or(LoadError::NoEmbeddedModel)
}

pub(crate) fn parse_pdf(bytes: &[u8], limits: &Limits) -> Result<Document, LoadError> {
    preflight(bytes, limits)?;
    let pdf = Document::load_mem_with_options(
        bytes,
        LoadOptions { filter: Some(drop_object_streams), strict: true, ..Default::default() },
    )
    .map_err(|_| malformed("invalid native PDF structure"))?;
    bound(LimitKind::PdfObjects, pdf.objects.len(), limits.max_pdf_objects)?;
    Ok(pdf)
}
fn drop_object_streams(id: ObjectId, object: &mut Object) -> Option<(ObjectId, Object)> {
    if object.as_stream().is_ok_and(|s| s.dict.has_type(b"ObjStm")) {
        None
    } else {
        // lopdf 0.43's outer load_objects_raw checks only Some/None and retains the original object.
        // Object streams never reach its inner filter. Avoid cloning every large content stream.
        Some((id, Object::Null))
    }
}

fn ws(b: u8) -> bool {
    matches!(b, 0 | b'\t' | b'\n' | 12 | b'\r' | b' ')
}
fn delimiter(b: u8) -> bool {
    ws(b) || b"()<>[]{}/%".contains(&b)
}
fn number(word: &[u8]) -> Result<usize, LoadError> {
    if word.is_empty() || !word.iter().all(u8::is_ascii_digit) {
        return Err(malformed("invalid PDF integer"));
    }
    word.iter().try_fold(0usize, |n, b| {
        n.checked_mul(10)
            .and_then(|n| n.checked_add((b - b'0') as usize))
            .ok_or_else(|| malformed("PDF integer overflow"))
    })
}
fn line<'a>(bytes: &'a [u8], pos: &mut usize) -> Result<&'a [u8], LoadError> {
    let start = *pos;
    let end = bytes[start..]
        .iter()
        .position(|b| *b == b'\n' || *b == b'\r')
        .map(|n| start + n)
        .ok_or_else(|| malformed("unterminated xref line"))?;
    if end - start > 64 {
        return Err(malformed("oversized xref line"));
    }
    *pos = end + 1;
    if bytes[end] == b'\r' && bytes.get(*pos) == Some(&b'\n') {
        *pos += 1;
    }
    Ok(&bytes[start..end])
}
fn words(line: &[u8]) -> Vec<&[u8]> {
    line.split(|b| ws(*b)).filter(|w| !w.is_empty()).collect()
}

/// The preflight's direct-object token count of `bytes` (the same lexer and rules the reader applies),
/// with the object/token budgets lifted so the caller compares against its own limits. No lopdf
/// parse, no model decode: a linear scan of the dictionaries, skipping stream data by /Length.
pub(crate) fn pdf_tokens(bytes: &[u8], limits: &Limits) -> Result<usize, LoadError> {
    let open = Limits { max_pdf_objects: usize::MAX / (2 * Limits::PDF_TOKENS_PER_OBJECT), ..*limits };
    preflight(bytes, &open)
}

/// Returns the direct-object token count (trailer + every indirect object, stream data excluded).
fn preflight(bytes: &[u8], limits: &Limits) -> Result<usize, LoadError> {
    // Require one unambiguous terminal footer. This agrees with lopdf's last-EOF/25-byte search;
    // a stray startxref after EOF or earlier alternative cannot evade our gate.
    let end = bytes.iter().rposition(|b| !ws(*b)).map(|i| i + 1).ok_or_else(|| malformed("empty PDF"))?;
    if end < 5 || &bytes[end - 5..end] != b"%%EOF" || bytes.len() - end > 500 {
        return Err(malformed("missing terminal PDF footer"));
    }
    let eof = end - 5;
    let from = eof.saturating_sub(25);
    let footer = bytes[from..eof]
        .windows(9)
        .rposition(|w| w == b"startxref")
        .map(|i| from + i)
        .ok_or_else(|| malformed("missing startxref"))?;
    let parts = words(&bytes[footer..end]);
    if parts.len() != 3 || parts[0] != b"startxref" || parts[2] != b"%%EOF" {
        return Err(malformed("invalid PDF footer"));
    }
    let xref = number(parts[1])?;
    if xref >= footer {
        return Err(malformed("xref offset outside the object area"));
    }
    if !bytes[xref..].starts_with(b"xref") {
        return Err(unsupported("compressed cross-reference"));
    }
    let mut pos = xref;
    if words(line(bytes, &mut pos)?) != [b"xref".as_slice()] {
        return Err(malformed("invalid xref header"));
    }
    let mut entries = Vec::new();
    let mut ids = HashSet::new();
    let mut total = 0usize;
    loop {
        if bytes[pos..].starts_with(b"trailer") && bytes.get(pos + 7).is_some_and(|b| delimiter(*b)) {
            pos += 7;
            break;
        }
        let section = words(line(bytes, &mut pos)?);
        if section.len() != 2 {
            return Err(malformed("invalid xref subsection"));
        }
        let start = number(section[0])?;
        let count = number(section[1])?;
        total = total.checked_add(count).ok_or_else(|| malformed("xref count overflow"))?;
        bound(LimitKind::PdfObjects, total, limits.max_pdf_objects)?;
        let last = start.checked_add(count).ok_or_else(|| malformed("xref id overflow"))?;
        if last > u32::MAX as usize {
            return Err(malformed("xref id overflow"));
        }
        for id in start..last {
            if !ids.insert(id) {
                return Err(malformed("overlapping xref subsections"));
            }
            let entry = words(line(bytes, &mut pos)?);
            if entry.len() != 3 {
                return Err(malformed("xref count does not match its entries"));
            }
            let offset = number(entry[0])?;
            let generation = number(entry[1])?;
            if generation > u16::MAX as usize {
                return Err(malformed("xref generation overflow"));
            }
            match entry[2] {
                b"n" if offset < xref => entries.push((offset, id, generation)),
                b"f" => {}
                _ => return Err(malformed("invalid xref entry")),
            }
        }
    }
    if total == 0 {
        return Err(malformed("empty cross-reference table"));
    }
    if pos >= footer || footer - pos > 64 * 1024 {
        return Err(unsupported("oversized trailer"));
    }
    let mut lex = Lexer::new(&bytes[pos..footer], limits.max_pdf_depth);
    let mut token_count = 0usize;
    scan_value(&mut lex, limits, &mut token_count, true)?;
    if lex.next()?.is_some() {
        return Err(malformed("extra trailer content"));
    }
    entries.sort_unstable();
    for (i, &(offset, id, generation)) in entries.iter().enumerate() {
        let stop = entries.get(i + 1).map(|e| e.0).unwrap_or(xref);
        if offset >= stop {
            return Err(malformed("duplicate object offsets"));
        }
        let mut lex = Lexer::new(&bytes[offset..stop], limits.max_pdf_depth);
        if lex.next()? != Some(Token::Word(id.to_string().as_bytes()))
            || lex.next()? != Some(Token::Word(generation.to_string().as_bytes()))
            || lex.next()? != Some(Token::Word(b"obj"))
        {
            return Err(malformed("xref and object header disagree"));
        }
        scan_value(&mut lex, limits, &mut token_count, false)?;
    }
    Ok(token_count)
}

#[derive(Debug, PartialEq)]
enum Token<'a> {
    Open(u8),
    Close(u8),
    Name(Vec<u8>),
    Word(&'a [u8]),
    String,
}
struct Lexer<'a> {
    bytes: &'a [u8],
    pos: usize,
    max_depth: usize,
}
impl<'a> Lexer<'a> {
    fn new(bytes: &'a [u8], max_depth: usize) -> Self {
        Self { bytes, pos: 0, max_depth }
    }
    fn skip(&mut self) {
        while let Some(&b) = self.bytes.get(self.pos) {
            if ws(b) {
                self.pos += 1;
            } else if b == b'%' {
                while self.bytes.get(self.pos).is_some_and(|b| *b != b'\n' && *b != b'\r') {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }
    fn next(&mut self) -> Result<Option<Token<'a>>, LoadError> {
        self.skip();
        let Some(&b) = self.bytes.get(self.pos) else {
            return Ok(None);
        };
        self.pos += 1;
        let token = match b {
            b'[' => Token::Open(b'['),
            b']' => Token::Close(b'['),
            b'<' if self.bytes.get(self.pos) == Some(&b'<') => {
                self.pos += 1;
                Token::Open(b'<')
            }
            b'>' if self.bytes.get(self.pos) == Some(&b'>') => {
                self.pos += 1;
                Token::Close(b'<')
            }
            b'(' => {
                let mut depth = 1;
                while depth > 0 {
                    bound(LimitKind::PdfDepth, depth, self.max_depth)?;
                    let b = *self.bytes.get(self.pos).ok_or_else(|| malformed("unterminated PDF string"))?;
                    self.pos += 1;
                    match b {
                        b'\\' => {
                            if self.pos >= self.bytes.len() {
                                return Err(malformed("unterminated string escape"));
                            }
                            self.pos += 1;
                        }
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        _ => {}
                    }
                }
                Token::String
            }
            b'<' => {
                while self.bytes.get(self.pos).is_some_and(|b| *b != b'>') {
                    let b = self.bytes[self.pos];
                    if !ws(b) && !b.is_ascii_hexdigit() {
                        return Err(malformed("invalid hexadecimal string"));
                    }
                    self.pos += 1;
                }
                if self.bytes.get(self.pos) != Some(&b'>') {
                    return Err(malformed("unterminated hexadecimal string"));
                }
                self.pos += 1;
                Token::String
            }
            b'/' => {
                let mut name = Vec::new();
                while self.bytes.get(self.pos).is_some_and(|b| !delimiter(*b)) {
                    let b = self.bytes[self.pos];
                    self.pos += 1;
                    if b == b'#' {
                        let hex =
                            self.bytes.get(self.pos..self.pos + 2).ok_or_else(|| malformed("invalid name escape"))?;
                        let h = |b: u8| {
                            (b as char).to_digit(16).map(|v| v as u8).ok_or_else(|| malformed("invalid name escape"))
                        };
                        name.push(h(hex[0])? * 16 + h(hex[1])?);
                        self.pos += 2;
                    } else {
                        name.push(b);
                    }
                }
                Token::Name(name)
            }
            b')' | b'>' | b'{' | b'}' => return Err(malformed("unexpected PDF delimiter")),
            _ => {
                let start = self.pos - 1;
                while self.bytes.get(self.pos).is_some_and(|b| !delimiter(*b)) {
                    self.pos += 1;
                }
                Token::Word(&self.bytes[start..self.pos])
            }
        };
        Ok(Some(token))
    }
}

/// Token budget applies across all indirect objects, independently of xref count. It prevents a
/// tiny-number array from amplifying one declared object into millions of allocated lopdf Objects.
fn scan_value(lex: &mut Lexer<'_>, limits: &Limits, count: &mut usize, trailer: bool) -> Result<(), LoadError> {
    let mut stack = Vec::new();
    let mut length = None;
    let mut first = true;
    while let Some(t) = lex.next()? {
        *count = count.saturating_add(1);
        if *count > limits.max_pdf_tokens() {
            return Err(unsupported("direct object complexity limit"));
        }
        if trailer && first && t != Token::Open(b'<') {
            return Err(malformed("invalid trailer dictionary"));
        }
        first = false;
        match t {
            Token::Open(kind) => {
                stack.push(kind);
                bound(LimitKind::PdfDepth, stack.len(), limits.max_pdf_depth)?;
            }
            Token::Close(kind) => {
                if stack.pop() != Some(kind) {
                    return Err(malformed("unbalanced PDF container"));
                }
                if trailer && stack.is_empty() {
                    return Ok(());
                }
            }
            Token::Name(name) => {
                if trailer {
                    match name.as_slice() {
                        b"Prev" => return Err(unsupported("incremental update")),
                        b"XRefStm" => return Err(unsupported("hybrid xref")),
                        b"Encrypt" => return Err(unsupported("encrypted")),
                        _ => {}
                    }
                } else if name == b"Length" && stack == b"<" {
                    let Some(Token::Word(n)) = lex.next()? else {
                        return Err(unsupported("indirect stream length"));
                    };
                    *count = count.saturating_add(1);
                    length = Some(number(n)?);
                    let saved = lex.pos;
                    if matches!(lex.next()?, Some(Token::Word(_))) {
                        return Err(unsupported("indirect stream length"));
                    }
                    lex.pos = saved;
                }
            }
            Token::Word(b"stream") if !trailer && stack.is_empty() => {
                let n = length.ok_or_else(|| unsupported("missing direct stream length"))?;
                while lex.bytes.get(lex.pos) == Some(&b' ') {
                    lex.pos += 1;
                }
                match lex.bytes.get(lex.pos) {
                    Some(b'\r') => {
                        lex.pos += 1;
                        if lex.bytes.get(lex.pos) == Some(&b'\n') {
                            lex.pos += 1;
                        }
                    }
                    Some(b'\n') => lex.pos += 1,
                    _ => return Err(malformed("invalid stream boundary")),
                }
                lex.pos = lex
                    .pos
                    .checked_add(n)
                    .filter(|n| *n <= lex.bytes.len())
                    .ok_or_else(|| malformed("stream length exceeds object boundary"))?;
                if lex.next()? != Some(Token::Word(b"endstream")) {
                    return Err(malformed("stream length does not match its boundary"));
                }
            }
            Token::Word(b"endobj") if !trailer && stack.is_empty() => {
                if lex.next()?.is_some() {
                    return Err(malformed("extra content between PDF objects"));
                }
                return Ok(());
            }
            _ => {}
        }
    }
    Err(malformed("unterminated PDF object"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    #[test]
    fn object_stream_filter_runs_before_inflation_and_preserves_other_streams() {
        let mut stream = Object::Stream(Stream::new(
            dictionary! { "Type" => "ObjStm", "Filter" => "FlateDecode" },
            b"not deflate".to_vec(),
        ));
        assert!(drop_object_streams((99, 0), &mut stream).is_none());
        let mut pdf = Document::load_mem(&crate::write_pdf(&varos_core::model::Document::default()).unwrap()).unwrap();
        let raw = b"not deflate; leave compressed page bytes alone".to_vec();
        let normal = pdf.add_object(Stream::new(dictionary! { "Filter" => "FlateDecode" }, raw.clone()));
        // The lopdf writer deliberately omits actual ObjStm objects; use a same-length type name
        // while writing, then restore it in place without disturbing any xref offsets.
        let dropped = pdf.add_object(Stream::new(
            dictionary! { "Type" => "ObjXtm", "Filter" => "FlateDecode" },
            b"not deflate".to_vec(),
        ));
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        let at = bytes.windows(6).position(|w| w == b"ObjXtm").unwrap();
        bytes[at + 3] = b'S';
        let loaded = parse_pdf(&bytes, &Limits::DEFAULT).unwrap();
        assert!(!loaded.objects.contains_key(&dropped));
        assert_eq!(loaded.get_object(normal).unwrap().as_stream().unwrap().content, raw);
    }
}
