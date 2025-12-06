mod entry;
mod formatter;
mod options;
mod size;
mod walker;

pub use entry::DirEntry;
pub use options::ScanOptions;
pub use walker::scan_directory;

// These will be re-exported as they are implemented:
// pub use walker::scan_directory_parallel;
// pub use formatter::{format_tree, format_table, format_json};
