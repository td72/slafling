use anyhow::{bail, Context, Result};

pub(super) const KB: u64 = 1_024;
pub(super) const MB: u64 = 1_048_576;
pub(super) const GB: u64 = 1_073_741_824;

pub(super) const DEFAULT_MAX_FILE_SIZE: u64 = 100 * MB;

pub fn parse_file_size(s: &str) -> Result<u64> {
    let s = s.trim();
    let (num_part, unit) = match s.find(|c: char| c.is_ascii_alphabetic()) {
        Some(i) => (s[..i].trim(), s[i..].trim().to_ascii_uppercase()),
        None => (s, String::new()),
    };

    let num: f64 = num_part
        .parse()
        .with_context(|| format!("invalid number in file size: '{s}'"))?;

    let multiplier: u64 = match unit.as_str() {
        "" | "B" => 1,
        "KB" | "K" => KB,
        "MB" | "M" => MB,
        "GB" | "G" => GB,
        _ => bail!("unknown file size unit: '{unit}' (use KB, MB, or GB)"),
    };

    Ok((num * multiplier as f64) as u64)
}

pub fn format_size(bytes: u64) -> String {
    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes}B")
    }
}

pub(super) fn is_truthy(s: &str) -> bool {
    matches!(s.to_lowercase().as_str(), "1" | "true" | "yes")
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn is_truthy_values() {
        for val in &["1", "true", "yes", "TRUE", "Yes", "YES"] {
            assert!(is_truthy(val), "expected '{val}' to be truthy");
        }
        for val in &["0", "false", "no", "", "maybe"] {
            assert!(!is_truthy(val), "expected '{val}' to be falsy");
        }
    }

    #[rstest]
    #[case("100B", 100)]
    #[case("1KB", KB)]
    #[case("50MB", 50 * MB)]
    #[case("2GB", 2 * GB)]
    #[case("1K", KB)]
    #[case("1M", MB)]
    #[case("1G", GB)]
    #[case("100mb", 100 * MB)]
    #[case("100Mb", 100 * MB)]
    #[case("1.5MB", (1.5 * MB as f64) as u64)]
    #[case("1024", 1024)]
    #[case(" 100 MB ", 100 * MB)]
    #[case("0MB", 0)]
    fn parse_file_size_valid(#[case] input: &str, #[case] expected: u64) {
        assert_eq!(parse_file_size(input).unwrap(), expected);
    }

    #[rstest]
    #[case("1TB", "unknown file size unit")]
    #[case("abcMB", "invalid number")]
    fn parse_file_size_invalid(#[case] input: &str, #[case] msg: &str) {
        assert!(parse_file_size(input)
            .unwrap_err()
            .to_string()
            .contains(msg));
    }

    #[rstest]
    #[case(0, "0B")]
    #[case(512, "512B")]
    #[case(1023, "1023B")]
    #[case(KB, "1.0KB")]
    #[case(MB, "1.0MB")]
    #[case(GB, "1.0GB")]
    fn format_size_cases(#[case] input: u64, #[case] expected: &str) {
        assert_eq!(format_size(input), expected);
    }
}
