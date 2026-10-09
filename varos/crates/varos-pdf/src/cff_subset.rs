//! Lane H: CID-keyed CFF subset rebuilt from exact unhinted cubic outlines.
//! Reencoding avoids copying unused charstrings/subroutines and preserves shaped glyph identities
//! through the caller's explicit CID map. Coordinates are normalized to 1000 units/em.
use varos_text_layout::font_export::{glyph_outline, Command, FaceId, FontSet, Glyph};
fn integer(out: &mut Vec<u8>, n: i32) {
    out.push(29);
    out.extend_from_slice(&n.to_be_bytes());
}
fn index(items: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let n = u16::try_from(items.len()).map_err(|_| "CFF INDEX count exceeded")?;
    let mut out = n.to_be_bytes().to_vec();
    if n == 0 {
        return Ok(out);
    }
    out.push(4);
    let mut offset = 1u32;
    out.extend_from_slice(&offset.to_be_bytes());
    for item in items {
        offset = offset.checked_add(item.len() as u32).ok_or("CFF INDEX overflow")?;
        out.extend_from_slice(&offset.to_be_bytes());
    }
    for item in items {
        out.extend(item);
    }
    Ok(out)
}
fn number(out: &mut Vec<u8>, value: f32) -> Result<(), String> {
    if !value.is_finite() || value.abs() > 32767. {
        return Err("CFF coordinate out of range".into());
    }
    out.push(255);
    out.extend_from_slice(&((f64::from(value) * 65536.).round() as i32).to_be_bytes());
    Ok(())
}
fn charstring(commands: &[Command]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut at = [0., 0.];
    for c in commands {
        match *c {
            Command::Move(p) | Command::Line(p) => {
                let p = [p[0], -p[1]];
                number(&mut out, p[0] - at[0])?;
                number(&mut out, p[1] - at[1])?;
                out.push(if matches!(c, Command::Move(_)) { 21 } else { 5 });
                at = p;
            }
            Command::Cubic(a, b, p) => {
                let a = [a[0], -a[1]];
                let b = [b[0], -b[1]];
                let p = [p[0], -p[1]];
                for d in [a[0] - at[0], a[1] - at[1], b[0] - a[0], b[1] - a[1], p[0] - b[0], p[1] - b[1]] {
                    number(&mut out, d)?;
                }
                out.push(8);
                at = p;
            }
            Command::Close => {}
        }
    }
    out.push(14);
    Ok(out)
}
fn top(count: usize, charset: usize, strings: usize, fdarray: usize, fdselect: usize) -> Vec<u8> {
    let mut d = Vec::new();
    for n in [391, 392, 0] {
        integer(&mut d, n);
    }
    d.extend([12, 30]);
    integer(&mut d, count as i32);
    d.extend([12, 34]);
    for (value, op) in [(charset, vec![15]), (strings, vec![17]), (fdarray, vec![12, 36]), (fdselect, vec![12, 37])] {
        integer(&mut d, value as i32);
        d.extend(op);
    }
    d
}
pub(crate) fn subset(fonts: &FontSet, face: FaceId, glyphs: &[u16], font_name: &str) -> Result<Vec<u8>, String> {
    if glyphs.len() > 16384 {
        return Err("CFF glyph budget exceeded".into());
    }
    let mut strings = vec![vec![14]];
    for &id in glyphs {
        let glyph =
            Glyph { id, face, cluster: 0..0, level: 0, x: 0., y: 0., advance: 0., offset: [0., 0.], size: 1000. };
        strings.push(charstring(&glyph_outline(fonts, &glyph)?.commands)?);
    }
    let name = index(&[font_name.as_bytes().to_vec()])?;
    let sid = index(&[b"Adobe".to_vec(), b"Identity".to_vec()])?;
    let count = strings.len();
    let charstrings = index(&strings)?;
    let top_size = index(&[top(count, 0, 0, 0, 0)])?.len();
    let charset_pos = 4 + name.len() + top_size + sid.len() + 2;
    let mut charset = vec![0];
    for i in 1..count {
        charset.extend_from_slice(&(i as u16).to_be_bytes());
    }
    let strings_pos = charset_pos + charset.len();
    let select_pos = strings_pos + charstrings.len();
    let mut select = vec![3, 0, 1, 0, 0, 0];
    select.extend_from_slice(&(count as u16).to_be_bytes());
    let fd_pos = select_pos + select.len();
    let mut private = Vec::new();
    integer(&mut private, 0);
    integer(&mut private, 0);
    private.push(18);
    let fd = index(&[private])?;
    let dictionary = index(&[top(count, charset_pos, strings_pos, fd_pos, select_pos)])?;
    let mut out = vec![1, 0, 4, 4];
    out.extend(name);
    out.extend(dictionary);
    out.extend(sid);
    out.extend([0, 0]);
    out.extend(charset);
    out.extend(charstrings);
    out.extend(select);
    out.extend(fd);
    if out.len() > 32 * 1024 * 1024 {
        return Err("CFF subset resource budget exceeded".into());
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cid_subset_parses_and_preserves_glyph_outline() {
        let fonts = varos_text_layout::bundled_fonts().unwrap();
        let font = ttf_parser::Face::parse(&fonts.faces()[0].bytes, 0).unwrap();
        let id = font.glyph_index('A').unwrap().0;
        let bytes = subset(&fonts, FaceId(0), &[id], "AAAAAA+Varos").unwrap();
        let cff = ttf_parser::cff::Table::parse(&bytes).unwrap();
        struct Count(usize);
        impl ttf_parser::OutlineBuilder for Count {
            fn move_to(&mut self, _: f32, _: f32) {
                self.0 += 1;
            }
            fn line_to(&mut self, _: f32, _: f32) {
                self.0 += 1;
            }
            fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {
                self.0 += 1;
            }
            fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
                self.0 += 1;
            }
            fn close(&mut self) {}
        }
        let mut count = Count(0);
        cff.outline(ttf_parser::GlyphId(1), &mut count).unwrap();
        assert!(count.0 > 5);
    }
}
