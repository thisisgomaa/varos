//! v5 project envelope. Documents remain the independent canvases they were in v4.
use super::*;

#[derive(Clone, Debug)]
pub struct Page {
    pub id: String,
    pub name: String,
    pub doc: Document,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub pages: Vec<Page>,
    pub active: usize,
}

/// Pure, deterministic v4 → v5 migration (also used after earlier document migrations).
pub fn migrate_v4_to_v5_pages(doc: Document) -> Project {
    Project { pages: vec![Page { id: "page-1".into(), name: "Page 1".into(), doc }], active: 0 }
}

#[derive(Deserialize)]
pub(super) struct FilePage {
    pub id: String,
    pub name: String,
    pub doc: Document,
    #[serde(default)]
    pub images: BTreeMap<String, Image>,
}

pub(super) fn validate_pages<'a>(pages: impl Iterator<Item = (&'a str, &'a str)>, active: usize) -> Result<(), FormatError> {
    let mut ids = HashSet::new();
    for (id, name) in pages {
        if ids.len() >= 10_000 || id.is_empty() || id.len() > 256 || name.trim().is_empty() || name.len() > 4096 || !ids.insert(id) {
            return Err(FormatError::NotVectorcraft("invalid, duplicate or excessive pages".into()));
        }
    }
    if active >= ids.len() {
        return Err(FormatError::NotVectorcraft("active_page is outside pages".into()));
    }
    Ok(())
}

pub fn save_project(project: &Project, options: &SaveOptions) -> Result<Vec<u8>, FormatError> {
    let mut bytes = Vec::new();
    save_project_to(project, options, &mut bytes)?;
    Ok(bytes)
}

/// Stream all pages and their independently keyed resources into one atomic file.
pub fn save_project_to(project: &Project, o: &SaveOptions, w: impl Write) -> Result<(), FormatError> {
    o.check()?;
    validate_pages(project.pages.iter().map(|p| (p.id.as_str(), p.name.as_str())), project.active)?;
    if o.version < 5 {
        if project.pages.len() != 1 {
            return Err(FormatError::Encode("versions 1–4 cannot store multiple pages".into()));
        }
        let page = project.pages.first().ok_or_else(|| FormatError::Encode("missing page".into()))?;
        return save_to(&page.doc, o, w);
    }
    let bodies = project.pages.iter().map(|p| body(&p.doc, o)).collect::<Result<Vec<_>, _>>()?;
    let mut blobs = Blobs { tail: vec![], at: 0, outside: true };
    #[derive(Serialize)]
    struct PageBody<'a> {
        id: &'a str,
        name: &'a str,
        doc: &'a Document,
        images: BTreeMap<&'a str, Image>,
    }
    let mut pages = Vec::with_capacity(bodies.len());
    for (page, b) in project.pages.iter().zip(&bodies) {
        let mut scan = KeyScan::new(std::io::sink());
        write_body(&mut scan, None, &b.d, false)?;
        let entries = Entries { blobs: &b.blobs, linked: &b.linked, include_linked: o.include_linked };
        pages.push(PageBody { id: &page.id, name: &page.name, doc: &b.d, images: entries.images(&scan.keys, &mut blobs) });
    }
    let profiles: BTreeMap<&str, Image> = o.profiles.iter().map(|(name, icc)| (name.as_str(), blobs.image(ICC_MIME, icc, false))).collect();
    let pdf = o.pdf.as_deref().map(|p| blobs.image("application/pdf", p, false));
    let mut out = if o.compress { Out::Gz(Box::new(Gz::new(w).map_err(encode_err)?)) } else { Out::Plain(w) };
    let mut env = Envelope { pretty: o.pretty && !o.compress, first: true };
    env.field(&mut out, "format", &UPSTREAM_EXTENSION)?;
    env.field(&mut out, "version", &o.version)?;
    env.field(&mut out, "generator", &VAROS_IDENTITY)?;
    if let Some(png) = &o.preview {
        env.field(&mut out, "preview", &Image { mime: "image/png".into(), data: base64_encode(png), blob: None, proxy: false })?;
    }
    if !profiles.is_empty() {
        env.field(&mut out, "profiles", &profiles)?;
    }
    if let Some(pdf) = pdf {
        env.field(&mut out, "pdf", &pdf)?;
    }
    env.field(&mut out, "pages", &pages)?;
    env.field(&mut out, "active_page", &project.active)?;
    env.finish(&mut out)?;
    if o.compress && (out.written().unwrap_or(0) > MAX_JSON || blobs.at > MAX_BLOBS || blobs.tail.iter().any(|(b, _)| b.len() as u64 > MAX_BLOB)) {
        return Err(FormatError::TooBigToCompress("project exceeds reader limits".into()));
    }
    if !blobs.tail.is_empty() {
        out.write_all(BLOB_MAGIC).map_err(encode_err)?;
        for (bytes, packed) in blobs.tail {
            out.packed(packed).map_err(encode_err)?;
            out.write_all(bytes).map_err(encode_err)?;
        }
    }
    out.finish().map_err(encode_err)
}
