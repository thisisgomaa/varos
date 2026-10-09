//! EXIF orientation parser copied from PhotoCraft codecs/orientation.rs:161-168 and supporting helpers@4cb7cf3.
// Copyright 2026 ArtCraft Team. MIT OR Apache-2.0; see NOTICE.
/// The TIFF/EXIF Orientation tag.
const TAG_ORIENTATION: u16 = 274;
/// Field types accepted for the Orientation tag: SHORT (the spec) and LONG
/// (written by some cameras and tools).
const TYPE_SHORT: u16 = 3;
const TYPE_LONG: u16 = 4;
/// Bytes per IFD entry.
const ENTRY: usize = 12;
/// The TIFF header is 8 bytes, so no IFD can start before it.
const HEADER: usize = 8;
/// BigTIFF uses a 16-byte header, 20-byte entries, and 8-byte offsets.
const BIG_ENTRY: usize = 20;
const BIG_HEADER: usize = 16;

/// Byte order of a TIFF structure.
#[derive(Clone, Copy)]
enum Order {
    Little,
    Big,
}

impl Order {
    fn u16(self, b: &[u8], at: usize) -> Option<u16> {
        let s: [u8; 2] = b.get(at..at.checked_add(2)?)?.try_into().ok()?;
        Some(match self {
            Order::Little => u16::from_le_bytes(s),
            Order::Big => u16::from_be_bytes(s),
        })
    }

    fn u32(self, b: &[u8], at: usize) -> Option<u32> {
        let s: [u8; 4] = b.get(at..at.checked_add(4)?)?.try_into().ok()?;
        Some(match self {
            Order::Little => u32::from_le_bytes(s),
            Order::Big => u32::from_be_bytes(s),
        })
    }

    fn u64(self, b: &[u8], at: usize) -> Option<u64> {
        let s: [u8; 8] = b.get(at..at.checked_add(8)?)?.try_into().ok()?;
        Some(match self {
            Order::Little => u64::from_le_bytes(s),
            Order::Big => u64::from_be_bytes(s),
        })
    }
}

/// The TIFF payload of an EXIF block (a JPEG-style `Exif\0\0` prefix is skipped).
fn tiff_body(b: &[u8]) -> &[u8] {
    b.strip_prefix(b"Exif\0\0").unwrap_or(b)
}

/// IFD0 of a TIFF structure: byte order, entry layout, and entry offsets.
///
/// The full table and next-IFD pointer must be present, and IFD0 must start
/// after the corresponding TIFF or BigTIFF header.
fn ifd0_entries(b: &[u8]) -> Option<(Order, usize, usize, usize, bool)> {
    let (order, big_tiff) = match b.get(0..4)? {
        [b'I', b'I', 42, 0] => (Order::Little, false),
        [b'M', b'M', 0, 42] => (Order::Big, false),
        [b'I', b'I', 43, 0] => (Order::Little, true),
        [b'M', b'M', 0, 43] => (Order::Big, true),
        _ => return None,
    };
    let (ifd, count_bytes, entry_bytes, next_ifd_bytes, header) = if big_tiff {
        if order.u16(b, 4)? != 8 || order.u16(b, 6)? != 0 {
            return None;
        }
        (usize::try_from(order.u64(b, 8)?).ok()?, 8usize, BIG_ENTRY, 8usize, BIG_HEADER)
    } else {
        (usize::try_from(order.u32(b, 4)?).ok()?, 2usize, ENTRY, 4usize, HEADER)
    };
    if ifd < header {
        return None;
    }
    let count = if big_tiff { usize::try_from(order.u64(b, ifd)?).ok()? } else { usize::from(order.u16(b, ifd)?) };
    let first = ifd.checked_add(count_bytes)?;
    let next_ifd = first.checked_add(count.checked_mul(entry_bytes)?)?;
    if next_ifd.checked_add(next_ifd_bytes)? > b.len() {
        return None;
    }
    // In bounds: every entry ends at or before `next_ifd`.
    Some((order, first, count, entry_bytes, big_tiff))
}

/// A well-formed Orientation entry's value: where it sits and its field type.
#[derive(Clone, Copy)]
struct Value {
    at: usize,
    ty: u16,
}

impl Value {
    /// The stored value (a SHORT or LONG, left-justified in the 4-byte field).
    fn read(self, order: Order, b: &[u8]) -> Option<u32> {
        match self.ty {
            TYPE_SHORT => order.u16(b, self.at).map(u32::from),
            _ => order.u32(b, self.at),
        }
    }
}

/// The value of the entry at `e` if it is a well-formed Orientation entry:
/// exactly one SHORT or LONG.
fn orientation_value(order: Order, b: &[u8], e: usize, big_tiff: bool) -> Option<Value> {
    let count = if big_tiff { order.u64(b, e.checked_add(4)?)? } else { u64::from(order.u32(b, e.checked_add(4)?)?) };
    if order.u16(b, e)? != TAG_ORIENTATION || count != 1 {
        return None;
    }
    let ty = order.u16(b, e.checked_add(2)?)?;
    let value_offset = if big_tiff { 12 } else { 8 };
    matches!(ty, TYPE_SHORT | TYPE_LONG).then_some(Value { at: e.checked_add(value_offset)?, ty })
}

/// Locates the Orientation value in IFD0.
///
/// The **first** Orientation entry wins (like libexif and Photoshop); if that
/// one is malformed (count ≠ 1, or neither SHORT nor LONG) the tag is ignored,
/// even when a later duplicate is well formed.
fn find_entry(b: &[u8]) -> Option<(Order, Value)> {
    let (order, first, count, entry_bytes, big_tiff) = ifd0_entries(b)?;
    let e = (0..count)
        .filter_map(|i| first.checked_add(i.checked_mul(entry_bytes)?))
        .find(|&e| order.u16(b, e) == Some(TAG_ORIENTATION))?;
    Some((order, orientation_value(order, b, e, big_tiff)?))
}

/// The orientation (1–8) recorded in a TIFF-structured block: an EXIF payload
/// (with or without the `Exif\0\0` prefix) or a whole TIFF file. Missing,
/// malformed or out-of-range values give 1.
pub fn exif_orientation(exif: &[u8]) -> u16 {
    let b = tiff_body(exif);
    find_entry(b)
        .and_then(|(order, v)| v.read(order, b))
        .and_then(|v| u16::try_from(v).ok())
        .filter(|v| (1..=8).contains(v))
        .unwrap_or(1)
}

/// Source resolution tags share the bounded IFD0 parser used for orientation.
pub fn tiff_ppi(bytes: &[u8]) -> Option<[f32; 2]> {
    let b = tiff_body(bytes);
    let (order, start, count, entry, big) = ifd0_entries(b)?;
    let mut axes = [None, None];
    let mut unit = 2u16;
    for n in 0..count {
        let at = start.checked_add(n.checked_mul(entry)?)?;
        let tag = order.u16(b, at)?;
        let ty = order.u16(b, at + 2)?;
        let count = if big { order.u64(b, at + 4)? } else { u64::from(order.u32(b, at + 4)?) };
        if count != 1 {
            continue;
        }
        let value = at + if big { 12 } else { 8 };
        if tag == 296 && ty == 3 {
            unit = order.u16(b, value)?;
        }
        if (tag == 282 || tag == 283) && ty == 5 {
            let offset = if big { value } else { order.u32(b, value)? as usize };
            let num = order.u32(b, offset)?;
            let den = order.u32(b, offset + 4)?;
            if den != 0 {
                axes[usize::from(tag == 283)] = Some(num as f32 / den as f32);
            }
        }
    }
    let factor = match unit {
        2 => 1.,
        3 => 2.54,
        _ => return None,
    };
    let result = [axes[0]? * factor, axes[1]? * factor];
    result.iter().all(|v| v.is_finite() && *v > 0.).then_some(result)
}
pub fn file_ppi(b: &[u8]) -> Option<[f32; 2]> {
    if b.starts_with(b"II") || b.starts_with(b"MM") {
        return tiff_ppi(b);
    }
    if b.starts_with(&[255, 216]) {
        let mut at = 2usize;
        while b.get(at) == Some(&255) {
            let marker = *b.get(at + 1)?;
            if marker == 0xda || marker == 0xd9 {
                break;
            }
            let len = u16::from_be_bytes(b.get(at + 2..at + 4)?.try_into().ok()?) as usize;
            if len < 2 {
                return None;
            }
            let data = b.get(at + 4..at.checked_add(2)?.checked_add(len)?)?;
            if marker == 0xe1 && data.starts_with(b"Exif\0\0") {
                return tiff_ppi(data);
            }
            at = at.checked_add(len)?.checked_add(2)?;
        }
    }
    if b.starts_with(b"\x89PNG") {
        let mut at = 8usize;
        while let Some(header) = b.get(at..at.checked_add(8)?) {
            let len = u32::from_be_bytes(header[..4].try_into().ok()?) as usize;
            let end = at.checked_add(8)?.checked_add(len)?;
            if &header[4..] == b"eXIf" {
                return tiff_ppi(b.get(at + 8..end)?);
            }
            at = end.checked_add(4)?;
        }
    }
    if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        let mut at = 12usize;
        while let Some(header) = b.get(at..at.checked_add(8)?) {
            let len = u32::from_le_bytes(header[4..].try_into().ok()?) as usize;
            let end = at.checked_add(8)?.checked_add(len)?;
            if &header[..4] == b"EXIF" {
                return tiff_ppi(b.get(at + 8..end)?);
            }
            at = end.checked_add(len % 2)?;
        }
    }
    None
}
