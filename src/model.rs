use clap::ValueEnum;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Human,
    Ai,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum FindingKind {
    UnusedExport,
    UnreachableFile,
    Uncertain,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub id: String,
    pub kind: FindingKind,
    pub file: PathBuf,
    pub symbol: Option<String>,
    pub reason: String,
    pub line: Option<usize>,
    pub col: Option<usize>,
    pub confidence: f32,
    pub fixable: bool,
}

#[derive(Debug, Clone)]
pub struct RemoveSummary {
    pub planned: usize,
    pub removed: usize,
    pub skipped_risky: usize,
    pub dry_run: bool,
}
