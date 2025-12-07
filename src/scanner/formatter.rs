use super::entry::DirEntry;
use super::size::format_size;

/// Format options for tree output
#[derive(Debug, Clone)]
pub struct FormatOptions {
    /// Maximum depth to display
    pub max_depth: Option<usize>,
    /// Show only top N entries per directory
    pub top_n: Option<usize>,
    /// Use colors in output
    pub colors: bool,
    /// Show file counts
    pub show_counts: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            max_depth: Some(3),
            top_n: Some(20),
            colors: true,
            show_counts: false,
        }
    }
}

impl FormatOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn with_top_n(mut self, n: usize) -> Self {
        self.top_n = Some(n);
        self
    }

    pub fn with_colors(mut self, enabled: bool) -> Self {
        self.colors = enabled;
        self
    }

    pub fn with_counts(mut self, show: bool) -> Self {
        self.show_counts = show;
        self
    }

    pub fn unlimited() -> Self {
        Self {
            max_depth: None,
            top_n: None,
            colors: false,
            show_counts: false,
        }
    }
}

/// Format entry as a tree string
pub fn format_tree(entry: &DirEntry, options: &FormatOptions) -> String {
    let mut output = String::new();
    format_tree_recursive(entry, &mut output, "", true, 0, options);
    output
}

fn format_tree_recursive(
    entry: &DirEntry,
    output: &mut String,
    prefix: &str,
    is_last: bool,
    depth: usize,
    options: &FormatOptions,
) {
    // Check depth limit
    if let Some(max_depth) = options.max_depth {
        if depth > max_depth {
            return;
        }
    }

    // Build the current line
    let connector = if depth == 0 {
        ""
    } else if is_last {
        "└── "
    } else {
        "├── "
    };

    let size_str = format_size(entry.size);
    let name = if entry.is_dir {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    };

    // Error indicator
    let error_indicator = if entry.error.is_some() { " [!]" } else { "" };

    // Count indicator
    let count_str = if options.show_counts && entry.is_dir {
        format!(" ({} files)", entry.file_count)
    } else {
        String::new()
    };

    output.push_str(&format!(
        "{}{}{:>10}  {}{}{}\n",
        prefix, connector, size_str, name, count_str, error_indicator
    ));

    // Process children
    if entry.is_dir && !entry.children.is_empty() {
        let new_prefix = if depth == 0 {
            String::new()
        } else if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        let children_to_show: Vec<_> = if let Some(top_n) = options.top_n {
            entry.children.iter().take(top_n).collect()
        } else {
            entry.children.iter().collect()
        };

        let total = children_to_show.len();
        let has_more = options.top_n.is_some_and(|n| entry.children.len() > n);

        for (i, child) in children_to_show.iter().enumerate() {
            let is_last_child = i == total - 1 && !has_more;
            format_tree_recursive(child, output, &new_prefix, is_last_child, depth + 1, options);
        }

        // Show truncation indicator if needed
        if has_more {
            let remaining = entry.children.len() - options.top_n.unwrap();
            output.push_str(&format!(
                "{}└── ... and {} more entries\n",
                new_prefix, remaining
            ));
        }
    }
}

/// Format entry as a simple table
pub fn format_table(entry: &DirEntry, options: &FormatOptions) -> String {
    let mut output = String::new();

    output.push_str(&format!("{:>12}  {}\n", "SIZE", "PATH"));
    output.push_str(&format!("{:->12}  {:-<50}\n", "", ""));

    format_table_recursive(entry, &mut output, 0, options);

    output
}

fn format_table_recursive(
    entry: &DirEntry,
    output: &mut String,
    depth: usize,
    options: &FormatOptions,
) {
    if let Some(max_depth) = options.max_depth {
        if depth > max_depth {
            return;
        }
    }

    let size_str = format_size(entry.size);
    let indent = "  ".repeat(depth);

    output.push_str(&format!("{:>12}  {}{}\n", size_str, indent, entry.name));

    let children_to_show: Vec<_> = if let Some(top_n) = options.top_n {
        entry.children.iter().take(top_n).collect()
    } else {
        entry.children.iter().collect()
    };

    for child in children_to_show {
        format_table_recursive(child, output, depth + 1, options);
    }

    // Show truncation indicator if needed
    if let Some(top_n) = options.top_n {
        if entry.children.len() > top_n {
            let remaining = entry.children.len() - top_n;
            let indent = "  ".repeat(depth + 1);
            output.push_str(&format!("{:>12}  {}... {} more\n", "", indent, remaining));
        }
    }
}

/// Format entry as JSON
pub fn format_json(entry: &DirEntry, pretty: bool) -> Result<String, serde_json::Error> {
    if pretty {
        serde_json::to_string_pretty(entry)
    } else {
        serde_json::to_string(entry)
    }
}

/// Simplified JSON structure for large outputs
#[derive(serde::Serialize)]
pub struct SummarizedEntry {
    pub path: String,
    pub size: u64,
    pub size_human: String,
    pub file_count: u64,
    pub dir_count: u64,
    pub children: Vec<SummarizedEntry>,
}

impl From<&DirEntry> for SummarizedEntry {
    fn from(entry: &DirEntry) -> Self {
        Self {
            path: entry.path.to_string_lossy().to_string(),
            size: entry.size,
            size_human: format_size(entry.size),
            file_count: entry.file_count,
            dir_count: entry.dir_count,
            children: entry.children.iter().map(SummarizedEntry::from).collect(),
        }
    }
}

/// Format as summarized JSON (smaller output)
pub fn format_json_summary(entry: &DirEntry, pretty: bool) -> Result<String, serde_json::Error> {
    let summary = SummarizedEntry::from(entry);
    if pretty {
        serde_json::to_string_pretty(&summary)
    } else {
        serde_json::to_string(&summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_entry() -> DirEntry {
        let mut root = DirEntry::new_dir(PathBuf::from("/test"), None);

        let mut subdir = DirEntry::new_dir(PathBuf::from("/test/subdir"), None);
        subdir.children.push(DirEntry::new_file(
            PathBuf::from("/test/subdir/file.txt"),
            1024,
            4096,
            None,
        ));
        subdir.recalculate_totals();

        root.children.push(DirEntry::new_file(
            PathBuf::from("/test/large.bin"),
            1048576,
            1048576,
            None,
        ));
        root.children.push(subdir);
        root.recalculate_totals();
        root.sort_by_size();

        root
    }

    #[test]
    fn test_format_tree_basic() {
        let entry = create_test_entry();
        let options = FormatOptions::default();
        let output = format_tree(&entry, &options);

        assert!(output.contains("test/"));
        assert!(output.contains("large.bin"));
        assert!(output.contains("subdir/"));
        assert!(output.contains("1.00 MB")); // large.bin
    }

    #[test]
    fn test_format_tree_contains_structure() {
        let entry = create_test_entry();
        let options = FormatOptions::unlimited();
        let output = format_tree(&entry, &options);

        // Should have tree characters
        assert!(output.contains("├──") || output.contains("└──"));
    }

    #[test]
    fn test_format_tree_depth_limit() {
        let entry = create_test_entry();
        let options = FormatOptions {
            max_depth: Some(1),
            ..Default::default()
        };
        let output = format_tree(&entry, &options);

        // Should show root and immediate children, but not nested file
        assert!(output.contains("subdir/"));
        assert!(!output.contains("file.txt"));
    }

    #[test]
    fn test_format_tree_with_counts() {
        let entry = create_test_entry();
        let options = FormatOptions::new().with_counts(true);
        let output = format_tree(&entry, &options);

        assert!(output.contains("files)"));
    }

    #[test]
    fn test_format_tree_top_n() {
        let mut root = DirEntry::new_dir(PathBuf::from("/test"), None);
        for i in 0..10 {
            root.children.push(DirEntry::new_file(
                PathBuf::from(format!("/test/file{}.txt", i)),
                1000 * (10 - i as u64),
                4096,
                None,
            ));
        }
        root.recalculate_totals();
        root.sort_by_size();

        let options = FormatOptions::new().with_top_n(3);
        let output = format_tree(&root, &options);

        assert!(output.contains("file0.txt")); // largest
        assert!(output.contains("file1.txt"));
        assert!(output.contains("file2.txt"));
        assert!(!output.contains("file9.txt")); // smallest, truncated
        assert!(output.contains("7 more entries"));
    }

    #[test]
    fn test_format_tree_error_entry() {
        let mut root = DirEntry::new_dir(PathBuf::from("/test"), None);
        root.children
            .push(DirEntry::new_error(PathBuf::from("/test/forbidden"), "Permission denied".to_string()));
        root.recalculate_totals();

        let options = FormatOptions::default();
        let output = format_tree(&root, &options);

        assert!(output.contains("[!]"));
    }

    #[test]
    fn test_format_table_basic() {
        let entry = create_test_entry();
        let options = FormatOptions::default();
        let output = format_table(&entry, &options);

        assert!(output.contains("SIZE"));
        assert!(output.contains("PATH"));
        assert!(output.contains("1.00 MB"));
        assert!(output.contains("test"));
    }

    #[test]
    fn test_format_table_indentation() {
        let entry = create_test_entry();
        let options = FormatOptions::unlimited();
        let output = format_table(&entry, &options);

        // Check that nested entries are indented
        let lines: Vec<&str> = output.lines().collect();
        // subdir should have more indentation than root
        let subdir_line = lines.iter().find(|l| l.contains("subdir")).unwrap();
        let root_line = lines.iter().find(|l| l.contains("test") && !l.contains("subdir")).unwrap();

        // subdir should appear after more spaces than root name
        assert!(subdir_line.find("subdir").unwrap() > root_line.find("test").unwrap());
    }

    #[test]
    fn test_format_json() {
        let entry = create_test_entry();
        let json = format_json(&entry, false).unwrap();

        assert!(json.contains("\"path\""));
        assert!(json.contains("\"size\""));
        assert!(json.contains("\"children\""));
    }

    #[test]
    fn test_format_json_pretty() {
        let entry = create_test_entry();
        let json = format_json(&entry, true).unwrap();

        // Pretty JSON should have newlines
        assert!(json.contains('\n'));
    }

    #[test]
    fn test_format_json_summary() {
        let entry = create_test_entry();
        let json = format_json_summary(&entry, false).unwrap();

        assert!(json.contains("size_human"));
        assert!(json.contains("1.00 MB") || json.contains("1 MB"));
    }

    #[test]
    fn test_format_json_summary_structure() {
        let entry = create_test_entry();
        let json = format_json_summary(&entry, true).unwrap();

        // Should have the expected fields
        assert!(json.contains("\"path\""));
        assert!(json.contains("\"size\""));
        assert!(json.contains("\"size_human\""));
        assert!(json.contains("\"file_count\""));
        assert!(json.contains("\"dir_count\""));
        assert!(json.contains("\"children\""));
    }

    #[test]
    fn test_format_json_parseable() {
        let entry = create_test_entry();
        let json = format_json(&entry, false).unwrap();

        // Verify it's valid JSON by parsing it
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_object());
        assert!(parsed["path"].is_string());
        assert!(parsed["size"].is_number());
    }
}
