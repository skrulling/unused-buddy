use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::{SourceType, Span};
use oxc_syntax::module_record::{ExportExportName, ExportImportName, ImportImportName};
use regex::Regex;
use walkdir::WalkDir;

use crate::config::EffectiveConfig;
use crate::model::{Finding, FindingKind, RemoveSummary};

#[derive(Debug, Clone)]
pub struct AnalyzerOptions {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub entry: Vec<PathBuf>,
    pub extensions: Vec<String>,
}

impl AnalyzerOptions {
    pub fn from_config(cfg: EffectiveConfig) -> Self {
        Self {
            include: cfg.include,
            exclude: cfg.exclude,
            entry: cfg.entry,
            extensions: cfg.extensions,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone)]
pub struct Analyzer {
    opts: AnalyzerOptions,
}

impl Analyzer {
    pub fn new(opts: AnalyzerOptions) -> Self {
        Self { opts }
    }

    pub fn scan(&self, root: &Path) -> Result<ScanResult> {
        let files = collect_source_files(root, &self.opts)?;
        let mut module_map: BTreeMap<PathBuf, ModuleInfo> = BTreeMap::new();

        for file in &files {
            let content = fs::read_to_string(file)
                .with_context(|| format!("failed reading {}", file.display()))?;
            module_map.insert(file.clone(), parse_module(file, &content));
        }

        let ts_paths = load_ts_paths(root)?;
        let roots = resolve_roots(root, &self.opts, &module_map)?;

        let mut graph: HashMap<PathBuf, Vec<Edge>> = HashMap::new();
        let mut imported_symbols: HashMap<PathBuf, HashSet<String>> = HashMap::new();
        let mut findings: Vec<Finding> = Vec::new();

        for (file, m) in &module_map {
            let mut edges = Vec::new();
            for imp in &m.imports {
                if imp.is_dynamic_non_literal {
                    findings.push(Finding {
                        id: format!("uc:{}:{}", file.display(), imp.raw),
                        kind: FindingKind::Uncertain,
                        file: file.clone(),
                        symbol: None,
                        reason: "dynamic_import_non_literal".to_string(),
                        line: None,
                        col: None,
                        confidence: 0.3,
                        fixable: false,
                    });
                    continue;
                }

                if let Some(target) = resolve_import(root, file, &imp.raw, &files, &ts_paths, &self.opts.extensions) {
                    edges.push(Edge {
                        target: target.clone(),
                    });

                    for s in &imp.symbols {
                        imported_symbols.entry(target.clone()).or_default().insert(s.clone());
                    }
                    if imp.wildcard_use {
                        imported_symbols.entry(target.clone()).or_default().insert("*".to_string());
                    }
                }
            }
            graph.insert(file.clone(), edges);
        }

        let reachable = reachable_files(&roots, &graph);

        for file in &files {
            if !reachable.contains(file) {
                let m = module_map.get(file).expect("present");
                let risky = has_possible_side_effects(&m.raw_source);
                findings.push(Finding {
                    id: format!("uf:{}", file.display()),
                    kind: FindingKind::UnreachableFile,
                    file: file.clone(),
                    symbol: None,
                    reason: if risky {
                        "unreachable_but_has_possible_side_effects".to_string()
                    } else {
                        "unreachable_file".to_string()
                    },
                    line: None,
                    col: None,
                    confidence: if risky { 0.6 } else { 0.98 },
                    fixable: !risky,
                });
            }
        }

        for file in &reachable {
            if let Some(m) = module_map.get(file) {
                let used = imported_symbols.get(file).cloned().unwrap_or_default();
                let has_any = used.contains("*");
                for export in &m.exports {
                    if !has_any && !used.contains(export) {
                        findings.push(Finding {
                            id: format!("ue:{}:{}", file.display(), export),
                            kind: FindingKind::UnusedExport,
                            file: file.clone(),
                            symbol: Some(export.clone()),
                            reason: "export_not_referenced".to_string(),
                            line: None,
                            col: None,
                            confidence: 0.85,
                            fixable: false,
                        });
                    }
                }
            }
        }

        findings.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(ScanResult { findings })
    }

    pub fn remove_safe_unreachable(&self, result: &ScanResult, fix: bool, yes: bool) -> Result<RemoveSummary> {
        let mut candidates = Vec::new();
        let mut skipped_risky = 0usize;

        for f in &result.findings {
            if f.kind == FindingKind::UnreachableFile {
                if f.fixable {
                    candidates.push(f.file.clone());
                } else {
                    skipped_risky += 1;
                }
            }
        }

        let planned = candidates.len();
        if !fix {
            return Ok(RemoveSummary {
                planned,
                removed: 0,
                skipped_risky,
                dry_run: true,
            });
        }

        if !yes {
            eprintln!("Refusing to mutate without --yes in non-interactive mode");
            return Ok(RemoveSummary {
                planned,
                removed: 0,
                skipped_risky,
                dry_run: true,
            });
        }

        let mut removed = 0usize;
        for f in candidates {
            if fs::remove_file(&f).is_ok() {
                removed += 1;
            }
        }

        Ok(RemoveSummary {
            planned,
            removed,
            skipped_risky,
            dry_run: false,
        })
    }
}

#[derive(Debug, Clone)]
struct Edge {
    target: PathBuf,
}

#[derive(Debug, Clone)]
struct ImportRef {
    raw: String,
    symbols: Vec<String>,
    wildcard_use: bool,
    is_dynamic_non_literal: bool,
}

#[derive(Debug, Clone)]
struct ModuleInfo {
    exports: Vec<String>,
    imports: Vec<ImportRef>,
    raw_source: String,
}

fn parse_module(_file: &Path, content: &str) -> ModuleInfo {
    let mut exports = BTreeSet::new();
    let mut import_map: BTreeMap<String, ImportRef> = BTreeMap::new();

    let source_type = SourceType::from_path(_file).unwrap_or_else(|_| SourceType::default());
    let allocator = Allocator::new();
    let parser_return = Parser::new(&allocator, content, source_type).parse();
    let mr = parser_return.module_record;

    for imp in &mr.import_entries {
        let raw = imp.module_request.name.to_string();
        let entry = import_map.entry(raw.clone()).or_insert_with(|| ImportRef {
            raw,
            symbols: Vec::new(),
            wildcard_use: false,
            is_dynamic_non_literal: false,
        });
        match &imp.import_name {
            ImportImportName::Name(name) => entry.symbols.push(name.name.to_string()),
            ImportImportName::Default(_) => {
                entry.symbols.push("default".to_string());
            }
            ImportImportName::NamespaceObject => {
                entry.wildcard_use = true;
            }
        }
    }

    for requested in mr.requested_modules.keys() {
        let raw = requested.to_string();
        import_map.entry(raw.clone()).or_insert_with(|| ImportRef {
            raw,
            symbols: Vec::new(),
            wildcard_use: false,
            is_dynamic_non_literal: false,
        });
    }

    for exp in &mr.local_export_entries {
        if let Some(name) = export_name_to_string(&exp.export_name) {
            exports.insert(name);
        }
    }

    for exp in &mr.indirect_export_entries {
        if let Some(name) = export_name_to_string(&exp.export_name) {
            exports.insert(name);
        }

        if let Some(module_request) = &exp.module_request {
            let raw = module_request.name.to_string();
            let entry = import_map.entry(raw.clone()).or_insert_with(|| ImportRef {
                raw,
                symbols: Vec::new(),
                wildcard_use: false,
                is_dynamic_non_literal: false,
            });

            match &exp.import_name {
                ExportImportName::Name(name) => entry.symbols.push(name.name.to_string()),
                ExportImportName::All | ExportImportName::AllButDefault => {
                    entry.wildcard_use = true;
                }
                ExportImportName::Null => {}
            }
        }
    }

    for exp in &mr.star_export_entries {
        if let Some(module_request) = &exp.module_request {
            let raw = module_request.name.to_string();
            let entry = import_map.entry(raw.clone()).or_insert_with(|| ImportRef {
                raw,
                symbols: Vec::new(),
                wildcard_use: true,
                is_dynamic_non_literal: false,
            });
            entry.wildcard_use = true;
        }
    }

    for dyn_imp in &mr.dynamic_imports {
        let expr_text = span_text(content, dyn_imp.module_request).trim();
        if let Some(specifier) = parse_string_literal(expr_text) {
            let entry = import_map
                .entry(specifier.clone())
                .or_insert_with(|| ImportRef {
                    raw: specifier.clone(),
                    symbols: vec![],
                    wildcard_use: true,
                    is_dynamic_non_literal: false,
                });
            entry.wildcard_use = true;
        } else {
            import_map.insert(
                format!("dynamic:{expr_text}"),
                ImportRef {
                    raw: expr_text.to_string(),
                    symbols: vec![],
                    wildcard_use: false,
                    is_dynamic_non_literal: true,
                },
            );
        }
    }

    // Preserve pragmatic CJS detection in v1.
    let re_module_exports = Regex::new(r"module\.exports\s*=\s*\{([^}]*)\}").expect("regex");
    for cap in re_module_exports.captures_iter(content) {
        for name in cap[1].split(',') {
            let n = name.split(':').next().unwrap_or(name).trim();
            if !n.is_empty() {
                exports.insert(n.to_string());
            }
        }
    }

    let re_require = Regex::new(r#"require\(\s*['"]([^'"]+)['"]\s*\)"#).expect("regex");
    for cap in re_require.captures_iter(content) {
        let raw = cap[1].to_string();
        let entry = import_map.entry(raw.clone()).or_insert_with(|| ImportRef {
            raw,
            symbols: vec![],
            wildcard_use: true,
            is_dynamic_non_literal: false,
        });
        entry.wildcard_use = true;
    }

    ModuleInfo {
        exports: exports.into_iter().collect(),
        imports: import_map.into_values().collect(),
        raw_source: content.to_string(),
    }
}

fn export_name_to_string(name: &ExportExportName<'_>) -> Option<String> {
    match name {
        ExportExportName::Name(name_span) => Some(name_span.name.to_string()),
        ExportExportName::Default(_) => Some("default".to_string()),
        ExportExportName::Null => None,
    }
}

fn span_text<'a>(source: &'a str, span: Span) -> &'a str {
    let start = span.start as usize;
    let end = span.end as usize;
    if start >= end || end > source.len() {
        return "";
    }
    &source[start..end]
}

fn parse_string_literal(expr: &str) -> Option<String> {
    let trimmed = expr.trim();
    if trimmed.len() < 2 {
        return None;
    }

    let first = trimmed.as_bytes()[0];
    let last = *trimmed.as_bytes().last()?;
    if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
        return Some(trimmed[1..trimmed.len() - 1].to_string());
    }

    None
}

fn collect_source_files(root: &Path, opts: &AnalyzerOptions) -> Result<Vec<PathBuf>> {
    let include_set = build_globset(&opts.include)?;
    let exclude_set = build_globset(&opts.exclude)?;

    let mut out = Vec::new();
    for ent in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !ent.file_type().is_file() {
            continue;
        }
        let path = ent.path().to_path_buf();
        let rel = path.strip_prefix(root).unwrap_or(&path);

        let rel_s = rel.to_string_lossy().replace('\\', "/");

        if exclude_set.is_match(rel_s.as_str()) {
            continue;
        }

        if !has_allowed_ext(&path, &opts.extensions) {
            continue;
        }

        let included = if opts.include.is_empty() {
            true
        } else {
            include_set.is_match(rel_s.as_str()) || rel_s.starts_with("src/")
        };

        if included {
            out.push(path);
        }
    }

    out.sort();
    Ok(out)
}

fn build_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        b.add(Glob::new(p).with_context(|| format!("invalid glob: {p}"))?);
    }
    b.build().context("failed to build glob set")
}

fn has_allowed_ext(path: &Path, allowed: &[String]) -> bool {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or_default();
    allowed.iter().any(|e| e == ext)
}

fn resolve_roots(root: &Path, opts: &AnalyzerOptions, map: &BTreeMap<PathBuf, ModuleInfo>) -> Result<Vec<PathBuf>> {
    if !opts.entry.is_empty() {
        let mut entries = Vec::new();
        for e in &opts.entry {
            let p = if e.is_absolute() { e.clone() } else { root.join(e) };
            if p.exists() {
                entries.push(p);
            }
        }
        if !entries.is_empty() {
            return Ok(entries);
        }
    }

    let mut roots = Vec::new();
    let pkg = root.join("package.json");
    if pkg.exists() {
        let raw = fs::read_to_string(&pkg).context("failed reading package.json")?;
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            for k in ["main", "module", "bin"] {
                if let Some(s) = v.get(k).and_then(|x| x.as_str()) {
                    let p = root.join(s);
                    if p.exists() {
                        roots.push(p);
                    }
                }
            }
            if let Some(exports) = v.get("exports") {
                match exports {
                    serde_json::Value::String(s) => {
                        let p = root.join(s);
                        if p.exists() {
                            roots.push(p);
                        }
                    }
                    serde_json::Value::Object(o) => {
                        for val in o.values() {
                            if let Some(s) = val.as_str() {
                                let p = root.join(s);
                                if p.exists() {
                                    roots.push(p);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if roots.is_empty() {
        for c in ["src/index.ts", "src/index.tsx", "src/index.js", "src/index.jsx"] {
            let p = root.join(c);
            if p.exists() {
                roots.push(p);
                break;
            }
        }
    }

    if roots.is_empty() {
        if let Some(first) = map.keys().next() {
            roots.push(first.clone());
        }
    }

    roots.sort();
    roots.dedup();
    Ok(roots)
}

fn reachable_files(roots: &[PathBuf], graph: &HashMap<PathBuf, Vec<Edge>>) -> HashSet<PathBuf> {
    let mut seen = HashSet::new();
    let mut stack = roots.to_vec();

    while let Some(cur) = stack.pop() {
        if !seen.insert(cur.clone()) {
            continue;
        }
        if let Some(next) = graph.get(&cur) {
            for edge in next {
                stack.push(edge.target.clone());
            }
        }
    }

    seen
}

#[derive(Debug, Clone)]
struct TsPaths {
    base_url: Option<PathBuf>,
    mappings: Vec<(String, Vec<String>)>,
}

fn load_ts_paths(root: &Path) -> Result<TsPaths> {
    let file = root.join("tsconfig.json");
    if !file.exists() {
        return Ok(TsPaths {
            base_url: None,
            mappings: Vec::new(),
        });
    }

    let raw = fs::read_to_string(file)?;
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    let opts = v.get("compilerOptions");
    let base_url = opts
        .and_then(|o| o.get("baseUrl"))
        .and_then(|b| b.as_str())
        .map(|b| root.join(b));

    let mut mappings = Vec::new();
    if let Some(paths) = opts.and_then(|o| o.get("paths")).and_then(|p| p.as_object()) {
        for (k, vals) in paths {
            let vec_vals = vals
                .as_array()
                .into_iter()
                .flat_map(|a| a.iter())
                .filter_map(|x| x.as_str())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            mappings.push((k.clone(), vec_vals));
        }
    }

    Ok(TsPaths { base_url, mappings })
}

fn resolve_import(
    root: &Path,
    current: &Path,
    raw: &str,
    files: &[PathBuf],
    ts: &TsPaths,
    exts: &[String],
) -> Option<PathBuf> {
    if raw.starts_with('.') {
        let base = current.parent().unwrap_or(root).join(raw);
        return resolve_candidate(base, files, exts);
    }

    if let Some(p) = resolve_ts_path(root, raw, ts, files, exts) {
        return Some(p);
    }

    None
}

fn resolve_ts_path(
    root: &Path,
    raw: &str,
    ts: &TsPaths,
    files: &[PathBuf],
    exts: &[String],
) -> Option<PathBuf> {
    for (alias, targets) in &ts.mappings {
        if let Some(star) = alias.find('*') {
            let prefix = &alias[..star];
            let suffix = &alias[star + 1..];
            if raw.starts_with(prefix) && raw.ends_with(suffix) {
                let middle = &raw[prefix.len()..raw.len() - suffix.len()];
                for t in targets {
                    let expanded = t.replace('*', middle);
                    let base = ts
                        .base_url
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(|| root.to_path_buf())
                        .join(expanded);
                    if let Some(p) = resolve_candidate(base, files, exts) {
                        return Some(p);
                    }
                }
            }
        } else if alias == raw {
            for t in targets {
                let base = ts
                    .base_url
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| root.to_path_buf())
                    .join(t);
                if let Some(p) = resolve_candidate(base, files, exts) {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn resolve_candidate(base: PathBuf, files: &[PathBuf], exts: &[String]) -> Option<PathBuf> {
    let mut cands = Vec::new();
    cands.push(base.clone());
    for ext in exts {
        cands.push(base.with_extension(ext));
        cands.push(base.join(format!("index.{ext}")));
    }

    for c in cands {
        if files.iter().any(|f| f == &c) {
            return Some(c);
        }
    }

    None
}

fn has_possible_side_effects(content: &str) -> bool {
    for line in content.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with("//") || l.starts_with("/*") || l.starts_with('*') {
            continue;
        }
        if l.starts_with("import ") || l.starts_with("export ") {
            continue;
        }
        if l.starts_with("type ") || l.starts_with("interface ") || l.starts_with("enum ") {
            continue;
        }
        if l.starts_with("const ") || l.starts_with("let ") || l.starts_with("var ") || l.starts_with("function ") || l.starts_with("class ") {
            continue;
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn unreachable_file_plain() {
        let dir = tempdir().expect("tmp");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(
            dir.path().join("src/index.ts"),
            "import { used } from './used'; console.log(used);",
        )
        .expect("write");
        fs::write(dir.path().join("src/used.ts"), "export const used = 1;").expect("write");
        fs::write(dir.path().join("src/dead.ts"), "export const dead = 2;").expect("write");

        let analyzer = Analyzer::new(AnalyzerOptions {
            include: vec!["src/**/*.{js,ts,jsx,tsx}".into()],
            exclude: vec![],
            entry: vec![],
            extensions: vec!["js".into(), "ts".into(), "jsx".into(), "tsx".into()],
        });

        let out = analyzer.scan(dir.path()).expect("scan");
        assert!(out.findings.iter().any(|f| f.id.contains("uf:") && f.file.ends_with("dead.ts")));
    }

    #[test]
    fn tsconfig_paths_alias() {
        let dir = tempdir().expect("tmp");
        fs::create_dir_all(dir.path().join("src/lib")).expect("mkdir");
        fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
        )
        .expect("write");
        fs::write(
            dir.path().join("src/index.ts"),
            "import { used } from '@/lib/used'; console.log(used);",
        )
        .expect("write");
        fs::write(dir.path().join("src/lib/used.ts"), "export const used = 1;").expect("write");
        fs::write(dir.path().join("src/lib/dead.ts"), "export const dead = 2;").expect("write");

        let analyzer = Analyzer::new(AnalyzerOptions {
            include: vec!["src/**/*.{js,ts,jsx,tsx}".into()],
            exclude: vec![],
            entry: vec![],
            extensions: vec!["js".into(), "ts".into(), "jsx".into(), "tsx".into()],
        });
        let out = analyzer.scan(dir.path()).expect("scan");
        assert!(out.findings.iter().any(|f| f.kind == FindingKind::UnreachableFile && f.file.ends_with("dead.ts")));
    }

    #[test]
    fn remove_with_fix_only_safe_unreachable() {
        let dir = tempdir().expect("tmp");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(dir.path().join("src/index.ts"), "export const ok = 1;").expect("write");
        fs::write(dir.path().join("src/dead.ts"), "export const dead = 1;").expect("write");
        fs::write(dir.path().join("src/risky.ts"), "console.log('side effect')").expect("write");

        let analyzer = Analyzer::new(AnalyzerOptions {
            include: vec!["src/**/*.{js,ts,jsx,tsx}".into()],
            exclude: vec![],
            entry: vec![PathBuf::from("src/index.ts")],
            extensions: vec!["js".into(), "ts".into(), "jsx".into(), "tsx".into()],
        });

        let out = analyzer.scan(dir.path()).expect("scan");
        let sum = analyzer.remove_safe_unreachable(&out, true, true).expect("remove");
        assert!(sum.removed >= 1);
        assert!(!dir.path().join("src/dead.ts").exists());
        assert!(dir.path().join("src/risky.ts").exists());
    }

    #[test]
    fn entrypoint_from_package_json() {
        let dir = tempdir().expect("tmp");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(dir.path().join("package.json"), r#"{"main":"src/main.ts"}"#).expect("write");
        fs::write(dir.path().join("src/main.ts"), "export const x = 1;").expect("write");
        fs::write(dir.path().join("src/dead.ts"), "export const y = 2;").expect("write");

        let analyzer = Analyzer::new(AnalyzerOptions {
            include: vec!["src/**/*.{js,ts,jsx,tsx}".into()],
            exclude: vec![],
            entry: vec![],
            extensions: vec!["js".into(), "ts".into(), "jsx".into(), "tsx".into()],
        });
        let out = analyzer.scan(dir.path()).expect("scan");
        assert!(out.findings.iter().any(|f| f.file.ends_with("dead.ts")));
    }

    #[test]
    fn tests_excluded_by_default_pattern() {
        let dir = tempdir().expect("tmp");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(dir.path().join("src/index.ts"), "export const x = 1;").expect("write");
        fs::write(dir.path().join("src/a.test.ts"), "export const t = 1;").expect("write");

        let opts = AnalyzerOptions {
            include: vec!["src/**/*.{js,ts,jsx,tsx}".into()],
            exclude: vec!["**/*.test.*".into()],
            entry: vec![],
            extensions: vec!["js".into(), "ts".into(), "jsx".into(), "tsx".into()],
        };
        let files = collect_source_files(dir.path(), &opts).expect("collect");
        assert!(files.iter().all(|f| !f.ends_with("a.test.ts")));
    }
}
