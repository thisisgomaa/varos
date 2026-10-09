//! Lane A test-only stamp normalization for classic pdf-writer native containers.
//! The v10 stamp adds a byte. Adjust only model length, stamps, xref offsets and startxref;
//! every other byte still compares to independent older-era frozen artifacts.
pub fn restamp(bytes: &[u8], from: u32, to: u32) -> Vec<u8> {
    let model = format!("\"varos\":{from}");
    let stamp = format!("/VAROS_SchemaVersion {from}");
    let at = bytes.windows(model.len()).position(|w| w == model.as_bytes()).expect("model stamp");
    let start = bytes[..at].windows(7).rposition(|w| w == b"stream\n").unwrap() + 7;
    let end = start + bytes[start..].windows(10).position(|w| w == b"\nendstream").unwrap();
    let length = b"/Length ";
    let length_at = bytes[..start].windows(length.len()).rposition(|w| w == length).unwrap() + length.len();
    let length_end = length_at + bytes[length_at..].iter().take_while(|b| b.is_ascii_digit()).count();
    assert_eq!(std::str::from_utf8(&bytes[length_at..length_end]).unwrap().parse::<usize>().unwrap(), end - start);
    let delta = format!("\"varos\":{to}").len() as i64 - model.len() as i64;
    let stamp_at = bytes.windows(stamp.len()).position(|w| w == stamp.as_bytes()).unwrap();
    let mut changes = [
        (at, at + model.len(), format!("\"varos\":{to}").into_bytes()),
        (stamp_at, stamp_at + stamp.len(), format!("/VAROS_SchemaVersion {to}").into_bytes()),
        (length_at, length_end, ((end - start) as i64 + delta).to_string().into_bytes()),
    ];
    changes.sort_by_key(|c| c.0);
    let shifted = |offset: usize| -> usize {
        (offset as i64
            + changes.iter().filter(|c| c.0 < offset).map(|(a, b, v)| v.len() as i64 - (*b - *a) as i64).sum::<i64>())
            as usize
    };
    let xref = bytes.windows(6).rposition(|w| w == b"\nxref\n").unwrap() + 1;
    let tail = std::str::from_utf8(&bytes[xref..]).unwrap();
    let lines = tail.lines().collect::<Vec<_>>();
    assert_eq!(lines[0], "xref");
    let count = lines[1].strip_prefix("0 ").unwrap().parse::<usize>().unwrap();
    let mut rebuilt = format!("xref\n0 {count}\n");
    for (i, line) in lines[2..2 + count].iter().enumerate() {
        let old = line[..10].parse::<usize>().unwrap();
        let offset = if i == 0 { old } else { shifted(old) };
        rebuilt.push_str(&format!("{offset:010}{}\r\n", &line[10..]));
    }
    let suffix = &lines[2 + count..];
    for (i, line) in suffix.iter().enumerate() {
        if i > 0 && suffix[i - 1] == "startxref" {
            rebuilt.push_str(&shifted(xref).to_string());
        } else {
            rebuilt.push_str(line);
        }
        if i + 1 < suffix.len() {
            rebuilt.push('\n');
        }
    }
    let mut out = bytes[..xref].to_vec();
    for (a, b, value) in changes.iter().rev() {
        out.splice(*a..*b, value.iter().copied());
    }
    out.extend(rebuilt.as_bytes());
    out
}
