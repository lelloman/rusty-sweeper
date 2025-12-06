//! Scan command implementation

use crate::cli::ScanArgs;
use crate::error::Result;
use crate::scanner::{
    format_json, format_table, format_tree, scan_directory_parallel, FormatOptions, ScanOptions,
};

/// Sort order for entries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Size,
    Name,
    // Mtime support can be added later
}

impl SortOrder {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "name" => SortOrder::Name,
            "mtime" => SortOrder::Size, // Fallback to size for now
            _ => SortOrder::Size,
        }
    }
}

/// Run the scan command
pub fn run(args: ScanArgs) -> Result<()> {
    // Build scan options
    // Scan deeper than display depth to get accurate totals
    let scan_depth = args.max_depth.saturating_add(10);
    let scan_options = ScanOptions::new()
        .with_max_depth(scan_depth)
        .with_hidden(args.all)
        .with_one_file_system(args.one_file_system)
        .with_threads(args.jobs.unwrap_or(0));

    tracing::info!(path = %args.path.display(), "Scanning directory");

    // Perform scan
    let mut entry = scan_directory_parallel(&args.path, &scan_options)?;

    // Apply sorting
    let sort_order = SortOrder::from_str(&args.sort);
    match sort_order {
        SortOrder::Size => entry.sort_by_size(),
        SortOrder::Name => entry.sort_by_name(),
    }

    // Format and output
    let output = if args.json {
        format_json(&entry, true)?
    } else {
        let format_options = FormatOptions::new()
            .with_max_depth(args.max_depth)
            .with_top_n(args.top);

        format_tree(&entry, &format_options)
    };

    println!("{}", output);

    // Print summary
    if !args.json {
        println!();
        println!(
            "Total: {} in {} files, {} directories",
            crate::scanner::format_size(entry.size),
            entry.file_count,
            entry.dir_count
        );
    }

    Ok(())
}

/// Run scan with table output
pub fn run_table(args: ScanArgs) -> Result<()> {
    let scan_depth = args.max_depth.saturating_add(10);
    let scan_options = ScanOptions::new()
        .with_max_depth(scan_depth)
        .with_hidden(args.all)
        .with_one_file_system(args.one_file_system)
        .with_threads(args.jobs.unwrap_or(0));

    let mut entry = scan_directory_parallel(&args.path, &scan_options)?;

    let sort_order = SortOrder::from_str(&args.sort);
    match sort_order {
        SortOrder::Size => entry.sort_by_size(),
        SortOrder::Name => entry.sort_by_name(),
    }

    let format_options = FormatOptions::new()
        .with_max_depth(args.max_depth)
        .with_top_n(args.top);

    let output = format_table(&entry, &format_options);
    println!("{}", output);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_order_from_str() {
        assert_eq!(SortOrder::from_str("size"), SortOrder::Size);
        assert_eq!(SortOrder::from_str("SIZE"), SortOrder::Size);
        assert_eq!(SortOrder::from_str("name"), SortOrder::Name);
        assert_eq!(SortOrder::from_str("NAME"), SortOrder::Name);
        assert_eq!(SortOrder::from_str("invalid"), SortOrder::Size);
    }
}
