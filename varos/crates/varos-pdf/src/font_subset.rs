//! Lane H: bounded, deterministic sparse TrueType subset; glyph IDs stay stable.
//! Composite closure is retained. No shaping tables are needed by a PDF's positioned glyphs.
use std::collections::{BTreeMap, BTreeSet};
fn u16at(b: &[u8], p: usize) -> Result<u16, String> {
    Ok(u16::from_be_bytes(b.get(p..p + 2).ok_or("truncated font")?.try_into().map_err(|_| "truncated font")?))
}
fn u32at(b: &[u8], p: usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(b.get(p..p + 4).ok_or("truncated font")?.try_into().map_err(|_| "truncated font")?))
}
pub fn tables(bytes: &[u8]) -> Result<BTreeMap<[u8; 4], &[u8]>, String> {
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("font resource exceeds 32 MiB".into());
    }
    let n = usize::from(u16at(bytes, 4)?);
    if n > 256 {
        return Err("font table limit".into());
    }
    let mut tables = BTreeMap::new();
    for i in 0..n {
        let p = 12 + i * 16;
        let tag = bytes.get(p..p + 4).ok_or("truncated font directory")?.try_into().map_err(|_| "invalid table tag")?;
        let off = u32at(bytes, p + 8)? as usize;
        let len = u32at(bytes, p + 12)? as usize;
        let data =
            bytes.get(off..off.checked_add(len).ok_or("font length overflow")?).ok_or("font table outside bytes")?;
        if tables.insert(tag, data).is_some() {
            return Err("duplicate font table".into());
        }
    }
    Ok(tables)
}
pub fn embedding_allowed(bytes: &[u8], subset: bool) -> Result<(), String> {
    let t = tables(bytes)?;
    let os2 = t.get(b"OS/2").ok_or("font has no embedding permission table")?;
    let bits = u16at(os2, 8)?;
    if bits & 0x0002 != 0 || bits & 0x0200 != 0 {
        return Err("font licence forbids outline font embedding".into());
    }
    if subset && bits & 0x0100 != 0 {
        return Err("font licence forbids subsetting".into());
    }
    Ok(())
}
pub fn subset(bytes: &[u8], glyphs: &BTreeSet<u16>) -> Result<Vec<u8>, String> {
    embedding_allowed(bytes, true)?;
    let t = tables(bytes)?;
    if t.contains_key(b"fvar") {
        return Err("variable font PDF embedding requires a static instance".into());
    }
    let head = *t.get(b"head").ok_or("font has no head")?;
    let maxp = *t.get(b"maxp").ok_or("font has no maxp")?;
    let glyf = *t.get(b"glyf").ok_or("CFF font subsetting unavailable; outline fallback")?;
    let loca = *t.get(b"loca").ok_or("font has no loca")?;
    let n = usize::from(u16at(maxp, 4)?);
    let long = u16at(head, 50)? == 1;
    let offset = |i: usize| -> Result<usize, String> {
        if long {
            Ok(u32at(loca, i * 4)? as usize)
        } else {
            Ok(usize::from(u16at(loca, i * 2)?) * 2)
        }
    };
    let mut keep = glyphs.clone();
    keep.insert(0);
    let mut todo: Vec<_> = keep.iter().copied().collect();
    while let Some(id) = todo.pop() {
        if usize::from(id) >= n {
            return Err("glyph outside font".into());
        }
        let data = glyf.get(offset(usize::from(id))?..offset(usize::from(id) + 1)?).ok_or("invalid glyph offsets")?;
        if data.is_empty() {
            continue;
        }
        if (u16at(data, 0)? as i16) >= 0 {
            continue;
        }
        let mut p = 10;
        let mut components = 0;
        loop {
            components += 1;
            if components > 4096 {
                return Err("composite glyph budget".into());
            }
            let flags = u16at(data, p)?;
            let child = u16at(data, p + 2)?;
            if keep.insert(child) {
                todo.push(child);
            }
            p += 4;
            p += if flags & 1 != 0 { 4 } else { 2 };
            p += if flags & 8 != 0 {
                2
            } else if flags & 64 != 0 {
                4
            } else if flags & 128 != 0 {
                8
            } else {
                0
            };
            if p > data.len() {
                return Err("truncated composite glyph".into());
            }
            if flags & 32 == 0 {
                break;
            }
        }
    }
    let mut new_glyf = Vec::new();
    let mut new_loca = Vec::new();
    for i in 0..n {
        new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());
        if keep.contains(&(i as u16)) {
            new_glyf.extend_from_slice(glyf.get(offset(i)?..offset(i + 1)?).ok_or("invalid glyph offsets")?);
            while !new_glyf.len().is_multiple_of(4) {
                new_glyf.push(0);
            }
        }
    }
    new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());
    let mut out: BTreeMap<[u8; 4], Vec<u8>> = t
        .iter()
        .filter(|(tag, _)| {
            [b"head", b"hhea", b"maxp", b"hmtx", b"cmap", b"name", b"OS/2", b"post", b"cvt ", b"fpgm", b"prep", b"gasp"]
                .contains(tag)
        })
        .map(|(tag, b)| (*tag, b.to_vec()))
        .collect();
    let head = out.get_mut(b"head").ok_or("missing head")?;
    head.get_mut(8..12).ok_or("short head")?.fill(0);
    head.get_mut(50..52).ok_or("short head")?.copy_from_slice(&1u16.to_be_bytes());
    out.insert(*b"glyf", new_glyf);
    out.insert(*b"loca", new_loca);
    let n = out.len() as u16;
    let power = 1u16 << n.ilog2();
    let mut result = vec![0u8; 12 + 16 * usize::from(n)];
    result[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    result[4..6].copy_from_slice(&n.to_be_bytes());
    result[6..8].copy_from_slice(&(power * 16).to_be_bytes());
    result[8..10].copy_from_slice(&(power.ilog2() as u16).to_be_bytes());
    result[10..12].copy_from_slice(&((n - power) * 16).to_be_bytes());
    let mut head_offset = 0;
    for (i, (tag, data)) in out.into_iter().enumerate() {
        let p = 12 + i * 16;
        let offset = result.len();
        result[p..p + 4].copy_from_slice(&tag);
        result[p + 4..p + 8].copy_from_slice(&checksum(&data).to_be_bytes());
        result[p + 8..p + 12].copy_from_slice(&(offset as u32).to_be_bytes());
        result[p + 12..p + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
        if &tag == b"head" {
            head_offset = offset;
        }
        result.extend(data);
        while !result.len().is_multiple_of(4) {
            result.push(0);
        }
    }
    let adjust = 0xb1b0afbau32.wrapping_sub(checksum(&result));
    result[head_offset + 8..head_offset + 12].copy_from_slice(&adjust.to_be_bytes());
    Ok(result)
}
fn checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0u32, |sum, c| {
        let mut b = [0; 4];
        b[..c.len()].copy_from_slice(c);
        sum.wrapping_add(u32::from_be_bytes(b))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn true_type_subset_preserves_composites_metrics_and_checksum() {
        let bytes = include_bytes!("../../varos-text/assets/fonts/Inter-Regular.ttf");
        let original = ttf_parser::Face::parse(bytes, 0).unwrap();
        let glyph = original.glyph_index('é').unwrap();
        let small = subset(bytes, &[glyph.0].into()).unwrap();
        assert!(small.len() < bytes.len());
        assert_eq!(checksum(&small), 0xb1b0afba);
        let parsed = ttf_parser::Face::parse(&small, 0).unwrap();
        assert_eq!(parsed.glyph_bounding_box(glyph), original.glyph_bounding_box(glyph));
        assert_eq!(parsed.glyph_hor_advance(glyph), original.glyph_hor_advance(glyph));
        let a = original.glyph_index('A').unwrap();
        assert!(parsed.glyph_bounding_box(a).is_none());
    }
    #[test]
    fn malformed_font_directory_is_bounded_and_refused() {
        for bytes in [vec![], vec![0; 12], vec![255; 24]] {
            assert!(subset(&bytes, &[0].into()).is_err());
        }
    }
}
