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

    #[test]
    fn parse_file_size_bytes() {
        assert_eq!(parse_file_size("100B").unwrap(), 100);
    }

    #[test]
    fn parse_file_size_kb() {
        assert_eq!(parse_file_size("1KB").unwrap(), KB);
    }

    #[test]
    fn parse_file_size_mb() {
        assert_eq!(parse_file_size("50MB").unwrap(), 50 * MB);
    }

    #[test]
    fn parse_file_size_gb() {
        assert_eq!(parse_file_size("2GB").unwrap(), 2 * GB);
    }

    #[test]
    fn parse_file_size_short_units() {
        assert_eq!(parse_file_size("1K").unwrap(), KB);
        assert_eq!(parse_file_size("1M").unwrap(), MB);
        assert_eq!(parse_file_size("1G").unwrap(), GB);
    }

    #[test]
    fn parse_file_size_case_insensitive() {
        assert_eq!(parse_file_size("100mb").unwrap(), 100 * MB);
        assert_eq!(parse_file_size("100Mb").unwrap(), 100 * MB);
    }

    #[test]
    fn parse_file_size_decimal() {
        assert_eq!(parse_file_size("1.5MB").unwrap(), (1.5 * MB as f64) as u64);
    }

    #[test]
    fn parse_file_size_no_unit() {
        assert_eq!(parse_file_size("1024").unwrap(), 1024);
    }

    #[test]
    fn parse_file_size_whitespace() {
        assert_eq!(parse_file_size(" 100 MB ").unwrap(), 100 * MB);
    }

    #[test]
    fn parse_file_size_zero() {
        assert_eq!(parse_file_size("0MB").unwrap(), 0);
    }

    #[test]
    fn parse_file_size_invalid_unit() {
        let err = parse_file_size("1TB").unwrap_err();
        assert!(err.to_string().contains("unknown file size unit"));
    }

    #[test]
    fn parse_file_size_invalid_number() {
        let err = parse_file_size("abcMB").unwrap_err();
        assert!(err.to_string().contains("invalid number"));
    }

    #[test]
    fn format_size_zero() {
        assert_eq!(format_size(0), "0B");
    }

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(512), "512B");
    }

    #[test]
    fn format_size_below_kb() {
        assert_eq!(format_size(1023), "1023B");
    }

    #[test]
    fn format_size_kb() {
        assert_eq!(format_size(KB), "1.0KB");
    }

    #[test]
    fn format_size_mb() {
        assert_eq!(format_size(MB), "1.0MB");
    }

    #[test]
    fn format_size_gb() {
        assert_eq!(format_size(GB), "1.0GB");
    }
}
