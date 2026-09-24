//! Short time text for storage status: "saved at 14:32" and "3 min ago" (work order §3.3 / Q1).
//! Local time comes from `chrono` (already in the build through `lopdf`; no new crate). The
//! `*_at` functions take the UTC offset explicitly so every bucket is tested without depending on
//! the machine's time zone.
use chrono::{DateTime, Datelike, FixedOffset, Local, Offset, TimeZone, Utc};

fn in_offset(unix: u64, offset: FixedOffset) -> DateTime<FixedOffset> {
    let secs = i64::try_from(unix).unwrap_or(i64::MAX);
    let utc = Utc.timestamp_opt(secs, 0).single().unwrap_or(DateTime::<Utc>::MIN_UTC);
    utc.with_timezone(&offset)
}

/// The machine's UTC offset at `unix` (DST-aware).
fn local_offset(unix: u64) -> FixedOffset {
    let secs = i64::try_from(unix).unwrap_or(i64::MAX);
    match Utc.timestamp_opt(secs, 0).single() {
        Some(t) => t.with_timezone(&Local).offset().fix(),
        None => Utc.fix(),
    }
}

/// 24-hour local clock text, e.g. `"14:32"`.
pub fn clock_hhmm(unix: u64) -> String {
    clock_hhmm_at(unix, local_offset(unix))
}

/// [`clock_hhmm`] in an explicit offset.
pub fn clock_hhmm_at(unix: u64, offset: FixedOffset) -> String {
    in_offset(unix, offset).format("%H:%M").to_string()
}

/// How long ago `then` was, seen from `now` (both unix seconds, local calendar):
/// "just now" (< 1 min, or `then` in the future) · "N min ago" (< 1 h) · "N h ago" (same day) ·
/// "Yesterday" · "12 Sep" (this year) · "12 Sep 2025".
pub fn relative(now: u64, then: u64) -> String {
    relative_at(now, then, local_offset(now))
}

/// [`relative`] in an explicit offset.
pub fn relative_at(now: u64, then: u64, offset: FixedOffset) -> String {
    if then >= now || now - then < 60 {
        return "just now".to_string();
    }
    let d = now - then;
    if d < 3600 {
        return format!("{} min ago", d / 60);
    }
    let (n, t) = (in_offset(now, offset), in_offset(then, offset));
    let (nd, td) = (n.date_naive(), t.date_naive());
    if nd == td {
        return format!("{} h ago", d / 3600);
    }
    if nd.pred_opt() == Some(td) {
        return "Yesterday".to_string();
    }
    if nd.year() == td.year() {
        t.format("%-d %b").to_string()
    } else {
        t.format("%-d %b %Y").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-09-24 14:32:00 UTC
    const NOW: u64 = 1_790_260_320;

    #[test]
    fn clock_is_24h_local() {
        let utc = FixedOffset::east_opt(0).unwrap();
        assert_eq!(clock_hhmm_at(NOW, utc), "14:32");
        let cairo = FixedOffset::east_opt(3 * 3600).unwrap();
        assert_eq!(clock_hhmm_at(NOW, cairo), "17:32");
        assert_eq!(clock_hhmm(NOW).len(), 5); // real local zone: shape only
    }

    #[test]
    fn relative_time_buckets() {
        let utc = FixedOffset::east_opt(0).unwrap();
        let r = |then: u64| relative_at(NOW, then, utc);
        assert_eq!(r(NOW), "just now");
        assert_eq!(r(NOW + 90), "just now", "clock skew never shows the future");
        assert_eq!(r(NOW - 59), "just now");
        assert_eq!(r(NOW - 60), "1 min ago");
        assert_eq!(r(NOW - 3 * 60 - 20), "3 min ago");
        assert_eq!(r(NOW - 59 * 60), "59 min ago");
        assert_eq!(r(NOW - 3600), "1 h ago");
        assert_eq!(r(NOW - (14 * 3600 + 32 * 60)), "14 h ago"); // 00:00 today
        assert_eq!(r(NOW - (14 * 3600 + 33 * 60)), "Yesterday"); // 23:59 yesterday
        assert_eq!(r(NOW - 30 * 3600), "Yesterday");
        assert_eq!(r(NOW - 2 * 86_400), "22 Sep");
        assert_eq!(r(NOW - 365 * 86_400), "24 Sep 2025");
        // The calendar day follows the offset: 01:00 in Cairo is still "today" there, not in UTC-5.
        let cairo = FixedOffset::east_opt(3 * 3600).unwrap();
        let ny = FixedOffset::west_opt(5 * 3600).unwrap();
        let late = NOW - 15 * 3600; // 23:32 UTC yesterday = 02:32 Cairo today = 18:32 NY yesterday
        assert_eq!(relative_at(NOW, late, cairo), "15 h ago");
        assert_eq!(relative_at(NOW, late, ny), "Yesterday");
        // The real-zone wrapper answers the zone-free buckets identically.
        assert_eq!(relative(NOW, NOW - 120), "2 min ago");
    }
}
