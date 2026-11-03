use jiff::Timestamp;
use jiff::tz::TimeZone;
use unicode_width::UnicodeWidthChar;

/// Formats Unix seconds as `YYYY-MM-DD HH:MM` in `tz` (the user's local time
/// in the app, with its daylight saving rules; UTC in tests).
#[must_use]
pub fn format_date(secs: i64, tz: &TimeZone) -> String {
    match Timestamp::from_second(secs) {
        Ok(at) => {
            let local = tz.to_datetime(at);
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                local.year(),
                local.month(),
                local.day(),
                local.hour(),
                local.minute()
            )
        }
        // Beyond what jiff represents (years past 9999): a broken commit
        // date, shown as UTC rather than not at all.
        Err(_) => format_utc(secs),
    }
}

fn format_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let minutes = secs.rem_euclid(86_400) / 60;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        minutes / 60,
        minutes % 60
    )
}

/// Days since 1970-01-01 to a (year, month, day) date.
/// Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Pads or truncates `text` to exactly `width` terminal columns, ending
/// truncated text with `…`. Counts display width, so wide characters (CJK,
/// emoji) never break column alignment.
#[must_use]
pub fn fit(text: &str, width: usize) -> String {
    let total: usize = text.chars().map(char_width).sum();
    if total <= width {
        return format!("{text}{}", " ".repeat(width - total));
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = char_width(ch);
        if used + w > width - 1 {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    used += 1;
    out.push_str(&" ".repeat(width - used));
    out
}

fn char_width(ch: char) -> usize {
    ch.width().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn formats_unix_time_as_date_and_minutes() {
        let utc = TimeZone::UTC;
        assert_eq!(format_date(0, &utc), "1970-01-01 00:00");
        assert_eq!(format_date(1_767_225_660, &utc), "2026-01-01 00:01");
        assert_eq!(
            format_date(951_782_400, &utc),
            "2000-02-29 00:00",
            "leap day"
        );
    }

    #[test]
    fn dates_are_shown_in_the_given_time_zone() {
        let minus_five = TimeZone::fixed(jiff::tz::offset(-5));
        assert_eq!(format_date(0, &minus_five), "1969-12-31 19:00");
        // 2026-11-03 03:47 UTC is still November 2 in Bogota.
        assert_eq!(format_date(1_793_677_620, &minus_five), "2026-11-02 22:47");
    }

    #[test]
    fn daylight_saving_time_is_applied() {
        // US Eastern: UTC-4 in summer, UTC-5 in winter.
        let eastern = TimeZone::posix("EST5EDT,M3.2.0,M11.1.0").unwrap();
        assert_eq!(format_date(1_783_080_000, &eastern), "2026-07-03 08:00");
        assert_eq!(format_date(1_767_225_600, &eastern), "2025-12-31 19:00");
    }

    #[test]
    fn out_of_range_dates_fall_back_to_utc() {
        assert_eq!(
            format_date(i64::MAX / 2, &TimeZone::UTC),
            format_utc(i64::MAX / 2)
        );
    }

    #[test]
    fn fit_pads_short_text_and_truncates_long_text() {
        assert_eq!(fit("abc", 5), "abc  ");
        assert_eq!(fit("abcdef", 4), "abc…");
        assert_eq!(fit("abc", 0), "");
    }

    #[test]
    fn fit_counts_display_width_not_bytes() {
        assert_eq!(fit("año", 3), "año");
        assert_eq!(fit("日本語", 5), "日本…");
        assert_eq!(
            fit("日本語", 4),
            "日… ",
            "a wide char that doesn't fit leaves a space"
        );
    }
}
