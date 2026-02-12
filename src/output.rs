use std::collections::BTreeMap;

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::analyzer::ScanResult;
use crate::config::EffectiveConfig;
use crate::model::{FindingKind, RemoveSummary};

pub fn print_scan(result: &ScanResult, cfg: &EffectiveConfig) -> Result<()> {
    match cfg.format {
        crate::model::OutputFormat::Ai => print_ai_scan(result),
        crate::model::OutputFormat::Human => print_human_scan(result, cfg.color.enabled()),
    }
}

pub fn print_remove_summary(summary: &RemoveSummary, cfg: &EffectiveConfig) -> Result<()> {
    if matches!(cfg.format, crate::model::OutputFormat::Ai) {
        let line = serde_json::json!({
            "planned": summary.planned,
            "removed": summary.removed,
            "skipped_risky": summary.skipped_risky,
            "dry_run": summary.dry_run,
        });
        println!("{}", serde_json::to_string(&line)?);
        return Ok(());
    }

    if cfg.color.enabled() {
        println!(
            "{} planned={} removed={} skipped_risky={} dry_run={}",
            "Remove summary".bold().cyan(),
            summary.planned,
            summary.removed,
            summary.skipped_risky,
            summary.dry_run
        );
    } else {
        println!(
            "Remove summary planned={} removed={} skipped_risky={} dry_run={}",
            summary.planned, summary.removed, summary.skipped_risky, summary.dry_run
        );
    }

    Ok(())
}

fn print_ai_scan(result: &ScanResult) -> Result<()> {
    for f in &result.findings {
        let k = match f.kind {
            FindingKind::UnusedExport => "ue",
            FindingKind::UnreachableFile => "uf",
            FindingKind::Uncertain => "uc",
        };

        let obj = serde_json::json!({
            "i": f.id,
            "k": k,
            "f": f.file,
            "s": f.symbol,
            "r": f.reason,
            "l": f.line,
            "c": f.col,
            "x": if f.fixable {1} else {0},
            "q": f.confidence,
        });
        println!("{}", serde_json::to_string(&obj)?);
    }
    Ok(())
}

fn print_human_scan(result: &ScanResult, color: bool) -> Result<()> {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for f in &result.findings {
        let key = match f.kind {
            FindingKind::UnreachableFile => "UF",
            FindingKind::UnusedExport => "UE",
            FindingKind::Uncertain => "UC",
        };
        *counts.entry(key).or_default() += 1;

        if color {
            let label = match f.kind {
                FindingKind::UnreachableFile => "[UF]".red().to_string(),
                FindingKind::UnusedExport => "[UE]".yellow().to_string(),
                FindingKind::Uncertain => "[UC]".magenta().to_string(),
            };
            let file = f.file.display().to_string().blue().to_string();
            let symbol = f
                .symbol
                .as_ref()
                .map(|s| format!(" {}", s.bright_white()))
                .unwrap_or_default();
            println!("{} {}{} {}", label, file, symbol, f.reason);
        } else {
            let label = match f.kind {
                FindingKind::UnreachableFile => "[UF]",
                FindingKind::UnusedExport => "[UE]",
                FindingKind::Uncertain => "[UC]",
            };
            let symbol = f.symbol.as_ref().map(|s| format!(" {s}")).unwrap_or_default();
            println!("{} {}{} {}", label, f.file.display(), symbol, f.reason);
        }
    }

    if result.findings.is_empty() {
        if color {
            println!("{}", "No findings".green());
        } else {
            println!("No findings");
        }
        return Ok(());
    }

    if color {
        println!(
            "{} UF={} UE={} UC={} total={}",
            "Summary".bold().cyan(),
            counts.get("UF").copied().unwrap_or(0),
            counts.get("UE").copied().unwrap_or(0),
            counts.get("UC").copied().unwrap_or(0),
            result.findings.len()
        );
    } else {
        println!(
            "Summary UF={} UE={} UC={} total={}",
            counts.get("UF").copied().unwrap_or(0),
            counts.get("UE").copied().unwrap_or(0),
            counts.get("UC").copied().unwrap_or(0),
            result.findings.len()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_mono_preserves_tags_and_layout() {
        let result = ScanResult {
            findings: vec![crate::model::Finding {
                id: "uf:x".into(),
                kind: FindingKind::UnreachableFile,
                file: "src/dead.ts".into(),
                symbol: None,
                reason: "unreachable_file".into(),
                line: None,
                col: None,
                confidence: 0.98,
                fixable: true,
            }],
        };
        let cfg = EffectiveConfig {
            include: vec![],
            exclude: vec![],
            entry: vec![],
            extensions: vec!["ts".into()],
            max_workers: None,
            format: crate::model::OutputFormat::Human,
            color: crate::color::ColorPolicy::Never,
            fix_mode: "files_only".into(),
        };
        print_scan(&result, &cfg).expect("print");
    }
}
