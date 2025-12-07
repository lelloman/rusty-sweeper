//! Clean command implementation.

use crate::cleaner::{
    CleanOptions, CleanOrchestrator, CleanProgress, CleanResult, DetectorRegistry, ProjectScanner,
    ScanOptions,
};
use crate::cli::CleanArgs;
use anyhow::Result;
use humansize::{format_size, BINARY};
use std::io::{self, Write};
use std::sync::Arc;

/// Run the clean command.
pub fn run(args: CleanArgs) -> Result<()> {
    // Resolve to absolute path
    let path = args
        .path
        .canonicalize()
        .unwrap_or_else(|_| args.path.clone());

    // Set up registry
    let registry = if let Some(types) = &args.types {
        let types: Vec<&str> = types.iter().map(|s| s.as_str()).collect();
        DetectorRegistry::with_types(&types)
    } else {
        DetectorRegistry::new()
    };

    if registry.is_empty() {
        if let Some(types) = &args.types {
            eprintln!(
                "Error: No valid project types found. Requested: {}",
                types.join(", ")
            );
            eprintln!("Valid types: {}", DetectorRegistry::new().ids().join(", "));
            std::process::exit(2);
        }
    }

    // Set up scanner options
    let mut exclude_patterns = vec![".git".to_string()];
    if let Some(excludes) = &args.exclude {
        exclude_patterns.extend(excludes.iter().cloned());
    }

    let scan_options = ScanOptions {
        max_depth: args.max_depth,
        exclude_patterns,
        follow_symlinks: false,
    };

    let scanner = ProjectScanner::new(registry.clone(), scan_options);

    // Scan for projects
    println!("Scanning for projects in {}...", path.display());
    let mut projects = scanner.scan(&path);

    // Apply age filter if specified
    if let Some(age_days) = args.age {
        let before_count = projects.len();
        projects = ProjectScanner::filter_by_age(projects, age_days as u64);
        if before_count > 0 && projects.is_empty() {
            println!(
                "Found {} project(s), but none older than {} days.",
                before_count, age_days
            );
            return Ok(());
        }
    }

    if projects.is_empty() {
        println!("No projects with cleanable artifacts found.");
        return Ok(());
    }

    // Display found projects
    print_projects_table(&projects);

    let total_size: u64 = projects.iter().map(|p| p.artifact_size).sum();
    println!(
        "\nTotal: {} in {} project{}",
        format_size(total_size, BINARY),
        projects.len(),
        if projects.len() == 1 { "" } else { "s" }
    );

    if args.size_only {
        return Ok(());
    }

    // Confirmation
    if !args.force && !args.dry_run {
        print!("\nProceed with cleanup? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Execute cleanup
    let clean_options = CleanOptions {
        dry_run: args.dry_run,
        use_native_commands: true,
    };
    let orchestrator = CleanOrchestrator::new(registry, clean_options, args.jobs);

    let progress = Arc::new(CleanProgress::new(projects.len()));

    if args.dry_run {
        println!("\n[DRY RUN] Would clean:");
    } else {
        println!("\nCleaning...");
    }

    let results = orchestrator.clean_all(projects, Some(progress));
    let summary = CleanOrchestrator::summarize(&results);

    // Print results
    println!("\nResults:");
    println!(
        "  Cleaned: {} project{}",
        summary.success_count,
        if summary.success_count == 1 { "" } else { "s" }
    );
    if summary.failed_count > 0 {
        println!(
            "  Failed:  {} project{}",
            summary.failed_count,
            if summary.failed_count == 1 { "" } else { "s" }
        );
    }
    if summary.skipped_count > 0 {
        println!(
            "  Skipped: {} project{}",
            summary.skipped_count,
            if summary.skipped_count == 1 { "" } else { "s" }
        );
    }
    println!("  Freed:   {}", format_size(summary.total_freed, BINARY));

    // Print failures
    for result in &results {
        if let CleanResult::Failed { project, error } = result {
            eprintln!("  Error cleaning {}: {}", project.path.display(), error);
        }
    }

    if summary.failed_count > 0 {
        std::process::exit(5); // Partial failure
    }

    Ok(())
}

fn print_projects_table(projects: &[crate::cleaner::DetectedProject]) {
    println!("\n  {:<10} {:<50} {:>10}", "TYPE", "PATH", "SIZE");
    println!("  {}", "─".repeat(72));

    for project in projects {
        let path_str = project.path.display().to_string();
        let path_display = if path_str.len() > 48 {
            format!("...{}", &path_str[path_str.len() - 45..])
        } else {
            path_str
        };

        println!(
            "  {:<10} {:<50} {:>10}",
            project.project_type,
            path_display,
            format_size(project.artifact_size, BINARY),
        );
    }
}
