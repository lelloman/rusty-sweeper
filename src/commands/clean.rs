//! Clean command implementation.

use crate::cleaner::{
    all_valid_type_ids, CleanOptions, CleanOrchestrator, CleanProgress, CleanResult,
    DetectedSystemResource, DetectorRegistry, ProjectScanner, ScanOptions, SystemCleanResult,
    SystemCleanerRegistry,
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

    // Determine which types were requested
    let requested_types: Option<Vec<&str>> = args
        .types
        .as_ref()
        .map(|t| t.iter().map(|s| s.as_str()).collect());

    // Check if any requested types are system cleaner types
    let system_cleaner_ids: Vec<&str> = crate::cleaner::system_registry::all_system_cleaner_ids();
    let run_system_cleaners = match &requested_types {
        None => true, // No filter: run everything
        Some(types) => types.iter().any(|t| system_cleaner_ids.contains(t)),
    };

    // Filter project types (exclude system cleaner IDs from detector registry)
    let project_types: Option<Vec<&str>> = requested_types.as_ref().map(|types| {
        types
            .iter()
            .filter(|t| !system_cleaner_ids.contains(t))
            .copied()
            .collect()
    });

    // Set up project detector registry
    let registry = match &project_types {
        Some(types) if !types.is_empty() => DetectorRegistry::with_types(types),
        Some(_) => DetectorRegistry::with_types(&[]), // Only system types requested
        None => DetectorRegistry::new(),
    };

    // Set up system cleaner registry
    let system_registry = if run_system_cleaners {
        match &requested_types {
            Some(types) => SystemCleanerRegistry::with_types(types),
            None => SystemCleanerRegistry::new(),
        }
    } else {
        SystemCleanerRegistry::with_types(&[])
    };

    // Validate that at least some valid types were requested
    if registry.is_empty() && system_registry.is_empty() {
        if let Some(types) = &args.types {
            eprintln!(
                "Error: No valid types found. Requested: {}",
                types.join(", ")
            );
            eprintln!("Valid types: {}", all_valid_type_ids().join(", "));
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
            // Don't return early if we also have system resources to check
            if !run_system_cleaners {
                return Ok(());
            }
        }
    }

    // Detect system resources
    let system_resources = if run_system_cleaners {
        system_registry.detect_all()
    } else {
        Vec::new()
    };

    // Check if there's anything to do
    if projects.is_empty() && system_resources.is_empty() {
        println!("No cleanable artifacts found.");
        return Ok(());
    }

    // Display found projects
    if !projects.is_empty() {
        print_projects_table(&projects);
    }

    // Display system resources
    if !system_resources.is_empty() {
        print_system_resources_table(&system_resources);
    }

    let project_size: u64 = projects.iter().map(|p| p.artifact_size).sum();
    let system_size: u64 = system_resources.iter().map(|r| r.size).sum();
    let total_size = project_size + system_size;

    let total_items = projects.len() + system_resources.len();
    println!(
        "\nTotal: {} in {} item{}",
        format_size(total_size, BINARY),
        total_items,
        if total_items == 1 { "" } else { "s" }
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

    // Clean projects
    let results = orchestrator.clean_all(projects, Some(progress));
    let mut summary = CleanOrchestrator::summarize(&results);

    // Clean system resources
    let mut system_results = Vec::new();
    for resource in &system_resources {
        if let Some(cleaner) = system_registry.get_cleaner(&resource.category) {
            let result = cleaner.clean(resource, args.dry_run);
            summary.add_system_result(&result);
            system_results.push(result);
        }
    }

    // Print results
    println!("\nResults:");
    println!(
        "  Cleaned: {} item{}",
        summary.success_count,
        if summary.success_count == 1 { "" } else { "s" }
    );
    if summary.failed_count > 0 {
        println!(
            "  Failed:  {} item{}",
            summary.failed_count,
            if summary.failed_count == 1 { "" } else { "s" }
        );
    }
    if summary.skipped_count > 0 {
        println!(
            "  Skipped: {} item{}",
            summary.skipped_count,
            if summary.skipped_count == 1 { "" } else { "s" }
        );
    }
    println!("  Freed:   {}", format_size(summary.total_freed, BINARY));

    // Print project failures
    for result in &results {
        if let CleanResult::Failed { project, error } = result {
            eprintln!("  Error cleaning {}: {}", project.path.display(), error);
        }
    }

    // Print system failures
    for result in &system_results {
        if let SystemCleanResult::Failed { resource, error } = result {
            eprintln!("  Error cleaning {}: {}", resource.display_name, error);
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

fn print_system_resources_table(resources: &[DetectedSystemResource]) {
    println!("\n  System Resources:");
    println!("  {:<20} {:<40} {:>10}", "RESOURCE", "DESCRIPTION", "SIZE");
    println!("  {}", "─".repeat(72));

    for resource in resources {
        let count_str = resource
            .item_count
            .map(|c| format!(" ({} items)", c))
            .unwrap_or_default();

        println!(
            "  {:<20} {:<40} {:>10}",
            resource.display_name,
            format!("{}{}", resource.description, count_str),
            format_size(resource.size, BINARY),
        );
    }
}
