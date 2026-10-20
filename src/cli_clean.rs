use anyhow::{bail, Context, Result};
use rusty_sweeper::cleaner::{
    CleanOptions, CleanOrchestrator, CleanResult, DetectorRegistry, ProjectScanner, ScanOptions,
};
use rusty_sweeper::config::CleanerConfig;
use rusty_sweeper::format_size;
use std::io::{self, Write};
use std::path::Path;

pub fn run(root: &Path, config: &CleanerConfig, yes: bool) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("Cannot access {}", root.display()))?;
    if !root.is_dir() {
        bail!("Not a directory: {}", root.display());
    }
    // Fail before discovery if the root itself cannot be read.
    std::fs::read_dir(&root)?;
    let types: Vec<_> = config.project_types.iter().map(String::as_str).collect();
    let scanner = ProjectScanner::new(
        DetectorRegistry::with_types(&types),
        ScanOptions {
            max_depth: config.max_depth,
            exclude_patterns: config.exclude_patterns.clone(),
            follow_symlinks: false,
        },
    );
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "Scanning projects in {}...", root.display())?;
    stdout.flush()?;
    let projects = scanner.scan(&root);
    let mut projects = if config.min_age_days > 0 {
        ProjectScanner::filter_by_age(projects, u64::from(config.min_age_days))
    } else {
        projects
    };
    projects.retain(|project| !project.artifact_paths.is_empty());
    projects.sort_by(|a, b| a.path.cmp(&b.path));
    if projects.is_empty() {
        writeln!(stdout, "No eligible project artifacts to clean.")?;
        return Ok(());
    }

    let total: u64 = projects.iter().map(|project| project.artifact_size).sum();
    for project in &projects {
        writeln!(
            stdout,
            "{}  [{}]  {}",
            project.path.display(),
            project.display_name,
            format_size(project.artifact_size)
        )?;
        for artifact in &project.artifact_paths {
            writeln!(stdout, "  {}", artifact.display())?;
        }
    }
    writeln!(
        stdout,
        "{} project(s), estimated reclaimable size: {}",
        projects.len(),
        format_size(total)
    )?;
    // Ensure the report was delivered before performing any deletion.
    stdout.flush()?;
    if !yes {
        write!(stdout, "Delete these project artifacts? [y/N] ")?;
        stdout.flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            writeln!(stdout, "Cancelled; no artifacts deleted.")?;
            return Ok(());
        }
    }

    let orchestrator = CleanOrchestrator::new(
        DetectorRegistry::with_types(&types),
        CleanOptions {
            dry_run: false,
            // Remove exactly the discovered artifacts, without invoking project scripts.
            use_native_commands: false,
        },
        config.parallel_jobs.max(1),
    );
    let results = orchestrator.clean_all(projects, None);
    let summary = CleanOrchestrator::summarize(&results);
    for result in results {
        match result {
            CleanResult::Success {
                project,
                freed_bytes,
            } => {
                writeln!(
                    stdout,
                    "Cleaned {}: freed {}",
                    project.path.display(),
                    format_size(freed_bytes)
                )?;
            }
            CleanResult::Failed {
                project,
                error,
                freed_bytes,
            } => {
                writeln!(
                    stdout,
                    "Failed {}: {error} (freed {})",
                    project.path.display(),
                    format_size(freed_bytes)
                )?;
            }
            CleanResult::Skipped { project, reason } => {
                writeln!(stdout, "Skipped {}: {reason}", project.path.display())?;
            }
        }
    }
    writeln!(
        stdout,
        "Cleaned: {}; failed: {}; skipped: {}; freed: {}",
        summary.success_count,
        summary.failed_count,
        summary.skipped_count,
        format_size(summary.total_freed)
    )?;
    if summary.failed_count > 0 {
        bail!("{} project cleanup(s) failed", summary.failed_count);
    }
    Ok(())
}
