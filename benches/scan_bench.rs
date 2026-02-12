use std::fs;
use std::hint::black_box;
use std::path::Path;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tempfile::TempDir;
use unused_buddy::analyzer::{Analyzer, AnalyzerOptions};

const FILES: usize = 2500;
const LOC_PER_FILE: usize = 40;

fn scan_benchmark(c: &mut Criterion) {
    let fixture = create_large_fixture(FILES, LOC_PER_FILE);
    let analyzer = Analyzer::new(AnalyzerOptions {
        include: vec!["src/**/*.{js,ts,jsx,tsx}".to_string()],
        exclude: vec![
            "node_modules/**".to_string(),
            "dist/**".to_string(),
            "build/**".to_string(),
            "coverage/**".to_string(),
            ".next/**".to_string(),
            "out/**".to_string(),
            "**/*.d.ts".to_string(),
            "**/*.test.*".to_string(),
            "**/*.spec.*".to_string(),
            "**/__tests__/**".to_string(),
        ],
        entry: vec!["src/index.ts".into()],
        extensions: vec!["js".into(), "ts".into(), "jsx".into(), "tsx".into()],
    });

    let total_loc = (FILES * LOC_PER_FILE) as u64;

    let mut group = c.benchmark_group("scan_js_ts");
    group.throughput(Throughput::Elements(total_loc));
    group.bench_with_input(BenchmarkId::new("synthetic", total_loc), &fixture, |b, root| {
        b.iter(|| {
            let result = analyzer.scan(black_box(root.path())).expect("scan should succeed");
            black_box(result.findings.len());
        });
    });
    group.finish();
}

fn create_large_fixture(file_count: usize, loc_per_file: usize) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("src");
    fs::create_dir_all(&src).expect("create src");

    // Anchor roots like a real package.
    fs::write(
        dir.path().join("package.json"),
        r#"{"name":"bench-fixture","main":"src/index.ts"}"#,
    )
    .expect("write package.json");

    // Include tsconfig path mapping to benchmark alias resolution path too.
    fs::write(
        dir.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
    )
    .expect("write tsconfig");

    let mut index = String::new();
    index.push_str("import { used_0 } from './mod_0';\n");
    index.push_str("console.log(used_0);\n");
    fs::write(src.join("index.ts"), index).expect("write index");

    for i in 0..file_count {
        write_module(&src, i, file_count, loc_per_file);
    }

    // Add unreachable files to keep cleanup logic represented.
    for i in 0..200 {
        let dead = format!("export const dead_{i} = {i};\n");
        fs::write(src.join(format!("dead_{i}.ts")), dead).expect("write dead file");
    }

    dir
}

fn write_module(src: &Path, i: usize, file_count: usize, loc_per_file: usize) {
    let mut body = String::new();
    if i + 1 < file_count {
        body.push_str(&format!("import {{ used_{} }} from './mod_{}';\n", i + 1, i + 1));
        body.push_str(&format!("export const used_{i} = used_{} + {i};\n", i + 1));
    } else {
        body.push_str(&format!("export const used_{i} = {i};\n"));
    }

    body.push_str(&format!("export const unused_{i} = {i} * 2;\n"));

    // Fill up deterministic LOC for throughput-like comparisons.
    for n in 0..loc_per_file.saturating_sub(3) {
        body.push_str(&format!("const local_{i}_{n} = {n};\n"));
    }

    fs::write(src.join(format!("mod_{i}.ts")), body).expect("write module");
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = scan_benchmark
}
criterion_main!(benches);
