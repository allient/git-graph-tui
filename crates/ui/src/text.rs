use unicode_width::UnicodeWidthChar;

/// Formats Unix seconds as `YYYY-MM-DD HH:MM` (UTC).
#[must_use]
pub fn format_date(secs: i64) -> String {
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
    fn formats_unix_time_as_utc_date_and_minutes() {
        assert_eq!(format_date(0), "1970-01-01 00:00");
        assert_eq!(format_date(1_767_225_660), "2026-01-01 00:01");
        assert_eq!(format_date(951_782_400), "2000-02-29 00:00", "leap day");
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
