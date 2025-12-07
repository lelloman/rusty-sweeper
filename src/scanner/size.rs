use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;

/// Get apparent file size (content length)
pub fn apparent_size(metadata: &Metadata) -> u64 {
    metadata.len()
}

/// Get actual disk usage (blocks * block_size)
/// On most Linux systems, st_blocks is in 512-byte units
pub fn disk_usage(metadata: &Metadata) -> u64 {
    metadata.blocks() * 512
}

/// Format size in human-readable format
pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} B", bytes)
    } else if size >= 100.0 {
        format!("{:.0} {}", size, UNITS[unit_idx])
    } else if size >= 10.0 {
        format!("{:.1} {}", size, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// Parse a size string like "1GB" into bytes
#[allow(dead_code)]
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();

    let (num_str, unit) = if s.ends_with("TB") {
        (&s[..s.len() - 2], 1024u64.pow(4))
    } else if s.ends_with("GB") {
        (&s[..s.len() - 2], 1024u64.pow(3))
    } else if s.ends_with("MB") {
        (&s[..s.len() - 2], 1024u64.pow(2))
    } else if s.ends_with("KB") {
        (&s[..s.len() - 2], 1024u64)
    } else if s.ends_with('B') {
        (&s[..s.len() - 1], 1u64)
    } else {
        (s.as_str(), 1u64)
    };

    num_str
        .trim()
        .parse::<f64>()
        .ok()
        .map(|n| (n * unit as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_zero() {
        assert_eq!(format_size(0), "0 B");
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1), "1 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(1024 * 15), "15.0 KB");
        assert_eq!(format_size(1024 * 150), "150 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1048576), "1.00 MB");
        assert_eq!(format_size(1048576 * 5), "5.00 MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1073741824), "1.00 GB");
    }

    #[test]
    fn test_format_size_terabytes() {
        assert_eq!(format_size(1099511627776), "1.00 TB");
    }

    #[test]
    fn test_parse_size_plain_number() {
        assert_eq!(parse_size("1024"), Some(1024));
        assert_eq!(parse_size("0"), Some(0));
    }

    #[test]
    fn test_parse_size_with_units() {
        assert_eq!(parse_size("1KB"), Some(1024));
        assert_eq!(parse_size("1 KB"), Some(1024));
        assert_eq!(parse_size("1MB"), Some(1048576));
        assert_eq!(parse_size("1 MB"), Some(1048576));
        assert_eq!(parse_size("1GB"), Some(1073741824));
        assert_eq!(parse_size("1TB"), Some(1099511627776));
    }

    #[test]
    fn test_parse_size_decimal() {
        assert_eq!(parse_size("1.5GB"), Some(1610612736));
        assert_eq!(parse_size("2.5MB"), Some(2621440));
    }

    #[test]
    fn test_parse_size_case_insensitive() {
        assert_eq!(parse_size("1kb"), Some(1024));
        assert_eq!(parse_size("1Kb"), Some(1024));
        assert_eq!(parse_size("1KB"), Some(1024));
    }

    #[test]
    fn test_parse_size_invalid() {
        assert_eq!(parse_size("invalid"), None);
        assert_eq!(parse_size("abc KB"), None);
    }

    #[test]
    fn test_parse_size_bytes_suffix() {
        assert_eq!(parse_size("100B"), Some(100));
        assert_eq!(parse_size("100 B"), Some(100));
    }
}
