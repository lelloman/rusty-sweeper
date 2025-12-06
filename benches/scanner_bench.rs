//! Benchmark tests for the scanner module

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rusty_sweeper::scanner::{scan_directory, scan_directory_parallel, ScanOptions};
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

/// Create a benchmark directory with the given number of files and directories
fn create_benchmark_dir(file_count: usize, dir_count: usize) -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    let files_per_dir = if dir_count > 0 {
        file_count / dir_count
    } else {
        file_count
    };

    for d in 0..dir_count {
        let subdir = root.join(format!("dir{}", d));
        fs::create_dir(&subdir).unwrap();

        for f in 0..files_per_dir {
            let mut file = File::create(subdir.join(format!("file{}.txt", f))).unwrap();
            file.write_all(&vec![b'x'; 1024]).unwrap();
        }
    }

    // Create remaining files in root if needed
    let remaining = file_count - (files_per_dir * dir_count);
    for f in 0..remaining {
        let mut file = File::create(root.join(format!("root_file{}.txt", f))).unwrap();
        file.write_all(&vec![b'y'; 1024]).unwrap();
    }

    dir
}

fn benchmark_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("scan");

    // Benchmark different directory sizes
    for size in [100, 500, 1000].iter() {
        let dir = create_benchmark_dir(*size, 10);
        let options = ScanOptions::default();

        group.bench_with_input(BenchmarkId::new("sequential", size), size, |b, _| {
            b.iter(|| scan_directory(black_box(dir.path()), &options))
        });

        group.bench_with_input(BenchmarkId::new("parallel", size), size, |b, _| {
            b.iter(|| scan_directory_parallel(black_box(dir.path()), &options))
        });
    }

    group.finish();
}

fn benchmark_deep_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("deep_scan");

    // Create a deeply nested structure
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Create 5 levels deep with 10 files each
    let mut current = root.to_path_buf();
    for level in 0..5 {
        current = current.join(format!("level{}", level));
        fs::create_dir(&current).unwrap();

        for f in 0..10 {
            let mut file = File::create(current.join(format!("file{}.txt", f))).unwrap();
            file.write_all(&vec![b'z'; 512]).unwrap();
        }
    }

    let options = ScanOptions::default();

    group.bench_function("sequential", |b| {
        b.iter(|| scan_directory(black_box(dir.path()), &options))
    });

    group.bench_function("parallel", |b| {
        b.iter(|| scan_directory_parallel(black_box(dir.path()), &options))
    });

    group.finish();
}

fn benchmark_with_hidden(c: &mut Criterion) {
    let mut group = c.benchmark_group("hidden_files");

    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Create visible and hidden files
    for i in 0..50 {
        let mut file = File::create(root.join(format!("visible{}.txt", i))).unwrap();
        file.write_all(b"visible").unwrap();

        let mut hidden = File::create(root.join(format!(".hidden{}", i))).unwrap();
        hidden.write_all(b"hidden").unwrap();
    }

    let options_without = ScanOptions::new().with_hidden(false);
    let options_with = ScanOptions::new().with_hidden(true);

    group.bench_function("without_hidden", |b| {
        b.iter(|| scan_directory_parallel(black_box(dir.path()), &options_without))
    });

    group.bench_function("with_hidden", |b| {
        b.iter(|| scan_directory_parallel(black_box(dir.path()), &options_with))
    });

    group.finish();
}

criterion_group!(benches, benchmark_scan, benchmark_deep_scan, benchmark_with_hidden);
criterion_main!(benches);
