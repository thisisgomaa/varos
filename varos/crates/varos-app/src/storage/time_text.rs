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

/// A board's date on the Start card / list (the mockup's form, local time): "Today 14:32",
/// "Yesterday 22:41", then "2 Oct" this year, "17 Sep 2025" in another year. A date in the future
/// (a clock moved back) reads as today. Each instant becomes a local calendar date with the offset in
/// force AT that instant (across a DST change the two differ), then the calendar days are compared.
pub fn board_date(now: u64, then: u64) -> String {
    board_date_with(now, then, local_offset)
}

/// [`board_date`] in one fixed offset.
pub fn board_date_at(now: u64, then: u64, offset: FixedOffset) -> String {
    board_date_with(now, then, |_| offset)
}

/// [`board_date`] with an explicit "offset in force at this instant" rule (tests model DST with it).
pub fn board_date_with(now: u64, then: u64, offset_at: impl Fn(u64) -> FixedOffset) -> String {
    let (n, t) = (in_offset(now, offset_at(now)), in_offset(then, offset_at(then)));
    let (nd, td) = (n.date_naive(), t.date_naive());
    if td >= nd {
        return format!("Today {}", t.format("%H:%M"));
    }
    if nd.pred_opt() == Some(td) {
        return format!("Yesterday {}", t.format("%H:%M"));
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

    #[test]
    fn board_dates_read_like_the_mockup() {
        let utc = FixedOffset::east_opt(0).unwrap();
        assert_eq!(board_date_at(NOW, NOW - 600, utc), "Today 14:22");
        assert_eq!(board_date_at(NOW, NOW + 600, utc), "Today 14:42", "a future time is today");
        assert_eq!(board_date_at(NOW, NOW - 16 * 3600 - 51 * 60, utc), "Yesterday 21:41");
        assert_eq!(board_date_at(NOW, NOW - 22 * 86_400, utc), "2 Sep");
        assert_eq!(board_date_at(NOW, NOW - 372 * 86_400, utc), "17 Sep 2025");
        // local time: 23:30 UTC on the 23rd is the 24th in Cairo
        let cairo = FixedOffset::east_opt(3 * 3600).unwrap();
        assert_eq!(board_date_at(NOW, NOW - 15 * 3600 - 2 * 60, cairo), "Today 02:30");
    }

    /// A DST change between the file's time and now: each instant uses its own offset. Model: UTC+1
    /// until 2026-03-29 01:00 UTC, UTC+2 after (central European summer time).
    #[test]
    fn board_dates_use_each_instants_own_offset_across_dst_midnight_and_new_year() {
        const SWITCH: u64 = 1_774_746_000; // 2026-03-29 01:00:00 UTC
        let cet = |at: u64| FixedOffset::east_opt(if at < SWITCH { 3600 } else { 7200 }).unwrap();
        // now = 2026-03-30 00:00:30 local (+2) = 2026-03-29 22:00:30 UTC
        let now = SWITCH + 21 * 3600 + 30;
        assert_eq!(board_date_with(now, now - 10, cet), "Today 00:00", "00:00:20 local, ten seconds ago");
        assert_eq!(board_date_with(now, now - 90, cet), "Yesterday 23:59");
        // the file is from BEFORE the switch: 2026-03-28 23:30 UTC = 2026-03-29 00:30 local (+1) —
        // yesterday seen from the 30th, although the summer offset would make it the 29th 01:30 too
        let before = SWITCH - 90 * 60;
        assert_eq!(board_date_with(now, before, cet), "Yesterday 00:30");
        // with the FILE's offset applied to now as well (the old rule), now would read 23:00 on the
        // 29th and the file "Today" — the case this guards
        assert_eq!(board_date_at(now, before, cet(before)), "Today 00:30");
        // the autumn switch back (UTC+2 → UTC+1 at 2026-10-25 01:00 UTC)
        const BACK: u64 = 1_792_890_000;
        let cest = |at: u64| FixedOffset::east_opt(if at < BACK { 7200 } else { 3600 }).unwrap();
        let now = BACK + 22 * 3600 + 30; // 2026-10-25 23:00:30 UTC = 2026-10-26 00:00:30 local (+1)
        assert_eq!(board_date_with(now, BACK - 30 * 60, cest), "Yesterday 02:30", "00:30 UTC (+2)");
        // New Year at 00:00:30 local (UTC+1): the 31st is yesterday, the 30th is last year
        let utc1 = |_| FixedOffset::east_opt(3600).unwrap();
        let new_year = 1_798_758_030; // 2026-12-31 23:00:30 UTC = 2027-01-01 00:00:30 (+1)
        assert_eq!(board_date_with(new_year, new_year - 60, utc1), "Yesterday 23:59");
        assert_eq!(board_date_with(new_year, new_year - 86_400 - 60, utc1), "30 Dec 2026");
        assert_eq!(board_date_with(new_year, new_year - 20, utc1), "Today 00:00");
    }

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
