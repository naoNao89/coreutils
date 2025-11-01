// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use divan::Bencher;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;
use uu_dd::uumain;
use uucore::benchmark::run_util_function;

fn create_test_file(path: &Path, size: usize) {
    let buffer = vec![0u8; size];
    let mut file = File::create(path).expect("Failed to create test file");
    file.write_all(&buffer).expect("Failed to write test file");
    file.sync_all().expect("Failed to sync test file");
}

/// Benchmark dd with various block sizes (full blocks only)
#[divan::bench(args = [512, 1024, 2048, 4096, 8192])]
fn dd_copy_full_blocks(bencher: Bencher, block_size: usize) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let input_file = temp_dir.path().join("input.bin");
    let output_file = temp_dir.path().join("output.bin");

    // Create input file: 256 full blocks
    let file_size = block_size * 256;
    create_test_file(&input_file, file_size);

    let input_str = input_file.to_str().unwrap();
    let output_str = output_file.to_str().unwrap();
    let bs_arg = format!("bs={}", block_size);

    bencher.bench(|| {
        // Remove output file before each iteration
        let _ = std::fs::remove_file(&output_file);

        run_util_function(uumain, &[
            input_str,
            output_str,
            &bs_arg,
            "status=none",
        ]);
    });
}

/// Benchmark dd with partial final block (O_DIRECT relevant)
#[divan::bench(args = [4096])]
fn dd_copy_with_partial_block(bencher: Bencher, block_size: usize) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let input_file = temp_dir.path().join("input.bin");
    let output_file = temp_dir.path().join("output.bin");

    // Create input file: 256 full blocks + 1 partial block
    let file_size = (block_size * 256) + (block_size / 2);
    create_test_file(&input_file, file_size);

    let input_str = input_file.to_str().unwrap();
    let output_str = output_file.to_str().unwrap();
    let bs_arg = format!("bs={}", block_size);

    bencher.bench(|| {
        let _ = std::fs::remove_file(&output_file);

        run_util_function(uumain, &[
            input_str,
            output_str,
            &bs_arg,
            "status=none",
        ]);
    });
}

/// Benchmark dd with small block size (maximizes dd overhead)
#[divan::bench]
fn dd_copy_small_blocks() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let input_file = temp_dir.path().join("input.bin");
    let output_file = temp_dir.path().join("output.bin");

    // Create input file: 1MB with 512-byte blocks = 2048 blocks
    let file_size = 1024 * 1024;
    create_test_file(&input_file, file_size);

    let input_str = input_file.to_str().unwrap();
    let output_str = output_file.to_str().unwrap();

    divan::black_box(|| {
        let _ = std::fs::remove_file(&output_file);

        run_util_function(uumain, &[
            input_str,
            output_str,
            "bs=512",
            "status=none",
        ]);
    });
}

/// Benchmark dd with large block size (minimizes dd overhead)
#[divan::bench]
fn dd_copy_large_blocks() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let input_file = temp_dir.path().join("input.bin");
    let output_file = temp_dir.path().join("output.bin");

    // Create input file: 1MB with 1MB blocks = 1 block
    let file_size = 1024 * 1024;
    create_test_file(&input_file, file_size);

    let input_str = input_file.to_str().unwrap();
    let output_str = output_file.to_str().unwrap();

    divan::black_box(|| {
        let _ = std::fs::remove_file(&output_file);

        run_util_function(uumain, &[
            input_str,
            output_str,
            "bs=1M",
            "status=none",
        ]);
    });
}

fn main() {
    divan::main();
}

