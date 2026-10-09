//! Adapted GPL groups/library shape from VectorCraft color/src/{palette_io,libraries}.rs@a469568
//! (MIT OR Apache-2.0). Varos ASE codec refuses spot/non-RGB blocks instead of losing colour data.
use crate::{model::Paint, swatches::Swatch};
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteFormat {
    Native,
    Gpl,
    Ase,
}
pub fn encode(entries: &[Swatch], format: PaletteFormat) -> Result<Vec<u8>, String> {
    crate::swatches::validate_document(&crate::model::Document { swatches: entries.to_vec(), ..Default::default() })?;
    if matches!(format, PaletteFormat::Gpl) && entries.iter().any(|s| s.global) {
        return Err("GPL cannot preserve global swatches; use ASE or JSON".into());
    }
    if matches!(format, PaletteFormat::Native) {
        return serde_json::to_vec(entries).map_err(|e| e.to_string());
    }
    if entries.iter().any(|s| s.paint.solid().is_none_or(|c| c[3] != 1.0)) {
        return Err("GPL/ASE require opaque solid swatches; use native JSON to preserve gradients/alpha".into());
    }
    if matches!(format, PaletteFormat::Gpl) {
        let mut out = String::from("GIMP Palette\nName: Varos\nColumns: 8\n");
        let mut group = String::new();
        for s in entries {
            if group != s.group {
                group = s.group.clone();
                out.push_str(&format!("# Group: {group}\n"));
            }
            if let Some(c) = s.paint.solid() {
                out.push_str(&format!(
                    "{} {} {}\t{}\n",
                    (c[0] * 255.).round() as u8,
                    (c[1] * 255.).round() as u8,
                    (c[2] * 255.).round() as u8,
                    s.name
                ));
            }
        }
        return Ok(out.into_bytes());
    }
    fn name(s: &str) -> Result<Vec<u8>, String> {
        let v: Vec<_> = s.encode_utf16().chain([0]).collect();
        let n = u16::try_from(v.len()).map_err(|_| "ASE name too long")?;
        let mut b = n.to_be_bytes().to_vec();
        for c in v {
            b.extend(c.to_be_bytes());
        }
        Ok(b)
    }
    let mut blocks = vec![];
    let mut group = String::new();
    for s in entries {
        if group != s.group {
            if !group.is_empty() {
                blocks.push((0xc002_u16, vec![]));
            }
            group = s.group.clone();
            if !group.is_empty() {
                blocks.push((0xc001, name(&group)?));
            }
        }
        let mut b = name(&s.name)?;
        b.extend(b"RGB ");
        if let Some(c) = s.paint.solid() {
            for v in &c[..3] {
                b.extend(v.to_be_bytes());
            }
        }
        b.extend((if s.global { 0_u16 } else { 2_u16 }).to_be_bytes());
        blocks.push((1, b));
    }
    if !group.is_empty() {
        blocks.push((0xc002, vec![]));
    }
    let mut out = b"ASEF\0\x01\0\0".to_vec();
    out.extend((blocks.len() as u32).to_be_bytes());
    for (kind, b) in blocks {
        out.extend(kind.to_be_bytes());
        out.extend((b.len() as u32).to_be_bytes());
        out.extend(b);
    }
    Ok(out)
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if n > self.0.len() {
            return Err("truncated ASE".into());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn name(&mut self) -> Result<String, String> {
        let n = self.u16()?;
        if n == 0 {
            return Err("empty ASE name".into());
        }
        let mut v = vec![];
        for _ in 0..n {
            v.push(self.u16()?);
        }
        if v.pop() != Some(0) {
            return Err("ASE name not terminated".into());
        }
        String::from_utf16(&v).map_err(|e| e.to_string())
    }
}
pub fn decode(bytes: &[u8], format: PaletteFormat) -> Result<Vec<Swatch>, String> {
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("palette too large".into());
    }
    let mut entries = vec![];
    if matches!(format, PaletteFormat::Native) {
        entries = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    } else if matches!(format, PaletteFormat::Gpl) {
        let s = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        if !s.starts_with("GIMP Palette\n") {
            return Err("invalid GPL header".into());
        }
        let mut group = String::new();
        for line in s.lines().skip(1) {
            if let Some(g) = line.strip_prefix("# Group:") {
                group = g.trim().into();
            } else if line.starts_with('#')
                || line.starts_with("Name:")
                || line.starts_with("Columns:")
                || line.trim().is_empty()
            {
                continue;
            } else {
                let mut w = line.split_whitespace();
                let mut c = [0., 0., 0., 1.];
                for ch in &mut c[..3] {
                    *ch = w.next().ok_or("missing GPL channel")?.parse::<u8>().map_err(|_| "invalid GPL channel")?
                        as f32
                        / 255.;
                }
                entries.push(Swatch {
                    id: entries.len() as u32 + 1,
                    name: w.collect::<Vec<_>>().join(" "),
                    paint: Paint::Solid(c),
                    global: false,
                    group: group.clone(),
                });
            }
        }
    } else {
        let mut r = Reader(bytes);
        if r.take(8)? != b"ASEF\0\x01\0\0" {
            return Err("unsupported ASE version".into());
        }
        let n = r.u32()?;
        if n > 8192 {
            return Err("too many ASE blocks".into());
        }
        let mut group = String::new();
        for _ in 0..n {
            let kind = r.u16()?;
            let len = r.u32()? as usize;
            let mut b = Reader(r.take(len)?);
            match kind {
                0xc001 => {
                    if !group.is_empty() {
                        return Err("nested ASE groups unsupported".into());
                    }
                    group = b.name()?;
                }
                0xc002 => group.clear(),
                1 => {
                    let name = b.name()?;
                    if b.take(4)? != b"RGB " {
                        return Err("only RGB ASE supported".into());
                    }
                    let mut c = [0., 0., 0., 1.];
                    for ch in &mut c[..3] {
                        *ch = f32::from_bits(b.u32()?);
                    }
                    let t = b.u16()?;
                    if t == 1 || t > 2 {
                        return Err("spot ASE unsupported".into());
                    }
                    entries.push(Swatch {
                        id: entries.len() as u32 + 1,
                        name,
                        paint: Paint::Solid(c),
                        global: t == 0,
                        group: group.clone(),
                    });
                }
                _ => return Err("unknown ASE block".into()),
            }
            if !b.0.is_empty() {
                return Err("extra ASE block data".into());
            }
        }
        if !r.0.is_empty() {
            return Err("extra ASE data".into());
        }
    }
    let d = crate::model::Document { swatches: entries.clone(), ..Default::default() };
    crate::swatches::validate_document(&d)?;
    Ok(entries)
}
pub fn library() -> Vec<Swatch> {
    [[1., 0., 0., 1.], [0., 1., 0., 1.], [0., 0., 1., 1.], [1., 1., 1., 1.], [0., 0., 0., 1.]]
        .into_iter()
        .enumerate()
        .map(|(i, c)| Swatch {
            id: i as u32 + 1,
            name: ["Red", "Green", "Blue", "White", "Black"][i].into(),
            paint: Paint::Solid(c),
            global: false,
            group: "Basics".into(),
        })
        .collect()
}
