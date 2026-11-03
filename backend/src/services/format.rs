//! Wording shared by every service that writes a number into a sentence.

/// A percentile or rank as an ordinal: `22` -> `"22nd"`, `11` -> `"11th"`.
///
/// The teens are the trap — 11, 12 and 13 take `th` despite ending in 1, 2 and
/// 3 — and percentiles land there often enough to matter.
pub fn ordinal(value: f32) -> String {
    let n = value.round().max(0.0) as i64;
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::ordinal;

    #[test]
    fn suffixes_follow_english_including_the_teens() {
        let cases = [
            (0.0, "0th"),
            (1.0, "1st"),
            (2.0, "2nd"),
            (3.0, "3rd"),
            (4.0, "4th"),
            (11.0, "11th"),
            (12.0, "12th"),
            (13.0, "13th"),
            (21.0, "21st"),
            (22.0, "22nd"),
            (23.0, "23rd"),
            (72.0, "72nd"),
            (100.0, "100th"),
            (101.0, "101st"),
            (111.0, "111th"),
        ];
        for (value, expected) in cases {
            assert_eq!(ordinal(value), expected, "{value}");
        }
    }

    #[test]
    fn rounds_before_choosing_a_suffix() {
        assert_eq!(ordinal(71.6), "72nd");
        assert_eq!(ordinal(10.6), "11th");
        assert_eq!(ordinal(-3.0), "0th");
    }
}
