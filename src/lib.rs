pub mod analyzer;
pub mod color;
pub mod config;
pub mod help_ai;
pub mod model;
pub mod output;

use std::env;
use std::path::PathBuf;

use analyzer::{Analyzer, AnalyzerOptions, ScanResult};
use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use color::ColorPolicy;
use config::EffectiveConfig;
use model::OutputFormat;

#[derive(Debug, clap::Parser)]
#[command(
    name = "unused-buddy",
    version,
    about = "Find, list, and safely remove unused JS/TS code",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[arg(long, value_enum, global = true)]
    pub format: Option<OutputFormat>,

    #[arg(long, value_enum, global = true)]
    pub color: Option<ColorPolicy>,

    #[arg(long, global = true)]
    pub entry: Vec<PathBuf>,

    #[arg(long, global = true)]
    pub include: Vec<String>,

    #[arg(long, global = true)]
    pub exclude: Vec<String>,

    #[arg(long, global = true)]
    pub max_workers: Option<usize>,

    #[arg(long, global = true, default_value_t = false)]
    pub fail_on_findings: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Scan project and print findings.
    Scan {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// List findings (human mode by default).
    List {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Remove safe unreachable files.
    Remove {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = false)]
        fix: bool,
        #[arg(long, default_value_t = false)]
        yes: bool,
    },
    /// Show command help.
    Help {
        command: Option<String>,
    },
}

pub fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if let Some(json) = maybe_emit_ai_help(&args)? {
        println!("{json}");
        return Ok(());
    }

    let cli = Cli::parse();

    if let Some(Command::Help { command }) = &cli.command {
        if let Some(name) = command {
            let mut cmd = Cli::command();
            if let Some(sc) = cmd.find_subcommand_mut(name) {
                sc.print_help().context("failed to print help")?;
                println!();
                return Ok(());
            }
        }
        Cli::command().print_help().context("failed to print help")?;
        println!();
        return Ok(());
    }

    let cfg = EffectiveConfig::load(&cli)?;
    let command = cli.command.unwrap_or(Command::Scan {
        path: PathBuf::from("."),
    });
    let analyzer = Analyzer::new(AnalyzerOptions::from_config(cfg.clone()));

    match command {
        Command::Scan { path } => run_scan(&analyzer, path, &cfg, cli.fail_on_findings),
        Command::List { path } => run_scan(&analyzer, path, &cfg, cli.fail_on_findings),
        Command::Remove { path, fix, yes } => run_remove(&analyzer, path, &cfg, fix, yes, cli.fail_on_findings),
        Command::Help { .. } => unreachable!(),
    }
}

fn run_scan(analyzer: &Analyzer, path: PathBuf, cfg: &EffectiveConfig, fail_on_findings: bool) -> Result<()> {
    let result = analyzer.scan(&path)?;
    output::print_scan(&result, cfg)?;
    maybe_fail(&result, fail_on_findings)
}

fn run_remove(
    analyzer: &Analyzer,
    path: PathBuf,
    cfg: &EffectiveConfig,
    fix: bool,
    yes: bool,
    fail_on_findings: bool,
) -> Result<()> {
    let result = analyzer.scan(&path)?;
    output::print_scan(&result, cfg)?;
    let summary = analyzer.remove_safe_unreachable(&result, fix, yes)?;
    output::print_remove_summary(&summary, cfg)?;
    maybe_fail(&result, fail_on_findings)
}

fn maybe_fail(result: &ScanResult, fail_on_findings: bool) -> Result<()> {
    if fail_on_findings && !result.findings.is_empty() {
        anyhow::bail!("findings present and --fail-on-findings set");
    }
    Ok(())
}

fn maybe_emit_ai_help(args: &[String]) -> Result<Option<String>> {
    let has_help = args.iter().any(|a| a == "--help" || a == "help");
    let format_ai = args
        .windows(2)
        .any(|w| w[0] == "--format" && w[1] == "ai")
        || args.iter().any(|a| a == "--format=ai");

    if !has_help || !format_ai {
        return Ok(None);
    }

    let sub = args
        .iter()
        .skip(1)
        .find(|a| matches!(a.as_str(), "scan" | "list" | "remove"))
        .map(String::as_str);
    let schema = help_ai::schema_for(sub);
    Ok(Some(serde_json::to_string(&schema)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_ai_detects_root() {
        let args = vec![
            "unused-buddy".to_string(),
            "--help".to_string(),
            "--format".to_string(),
            "ai".to_string(),
        ];
        let out = maybe_emit_ai_help(&args).expect("ok").expect("some");
        assert!(out.contains("\"n\":\"unused-buddy\""));
    }
}
