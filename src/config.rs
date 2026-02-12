use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{color::ColorPolicy, model::OutputFormat, Cli};

#[derive(Debug, Clone, Deserialize, Default)]
struct FileConfig {
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    entry: Option<Vec<PathBuf>>,
    extensions: Option<Vec<String>>,
    max_workers: Option<usize>,
    format: Option<String>,
    color: Option<String>,
    fix_mode: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub entry: Vec<PathBuf>,
    pub extensions: Vec<String>,
    pub max_workers: Option<usize>,
    pub format: OutputFormat,
    pub color: ColorPolicy,
    pub fix_mode: String,
}

impl EffectiveConfig {
    pub fn load(cli: &Cli) -> Result<Self> {
        let path = cli
            .config
            .clone()
            .or_else(|| {
                let p = PathBuf::from("unused-buddy.toml");
                if p.exists() { Some(p) } else { None }
            });

        let fcfg = if let Some(path) = path {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed reading config {}", path.display()))?;
            toml::from_str::<FileConfig>(&raw)
                .with_context(|| format!("failed parsing config {}", path.display()))?
        } else {
            FileConfig::default()
        };

        let mut include = fcfg
            .include
            .unwrap_or_else(|| vec!["src/**/*.{js,ts,jsx,tsx}".to_string()]);
        if !cli.include.is_empty() {
            include = cli.include.clone();
        }

        let mut exclude = fcfg.exclude.unwrap_or_else(default_excludes);
        if !cli.exclude.is_empty() {
            exclude = cli.exclude.clone();
        }

        let mut entry = fcfg.entry.unwrap_or_default();
        if !cli.entry.is_empty() {
            entry = cli.entry.clone();
        }

        let mut extensions = fcfg
            .extensions
            .unwrap_or_else(|| vec!["js".into(), "ts".into(), "jsx".into(), "tsx".into()]);
        extensions.sort();
        extensions.dedup();

        let format = cli
            .format
            .or_else(|| parse_format(fcfg.format.as_deref()))
            .unwrap_or(OutputFormat::Human);

        let color = cli
            .color
            .or_else(|| parse_color(fcfg.color.as_deref()))
            .unwrap_or(ColorPolicy::Auto);

        Ok(Self {
            include,
            exclude,
            entry,
            extensions,
            max_workers: cli.max_workers.or(fcfg.max_workers),
            format,
            color,
            fix_mode: fcfg.fix_mode.unwrap_or_else(|| "files_only".to_string()),
        })
    }
}

fn parse_format(v: Option<&str>) -> Option<OutputFormat> {
    match v {
        Some("ai") => Some(OutputFormat::Ai),
        Some("human") => Some(OutputFormat::Human),
        _ => None,
    }
}

fn parse_color(v: Option<&str>) -> Option<ColorPolicy> {
    match v {
        Some("always") => Some(ColorPolicy::Always),
        Some("never") => Some(ColorPolicy::Never),
        Some("auto") => Some(ColorPolicy::Auto),
        _ => None,
    }
}

fn default_excludes() -> Vec<String> {
    vec![
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
    ]
}
