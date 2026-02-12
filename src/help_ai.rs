use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HelpSchema {
    pub n: String,
    pub d: String,
    pub u: String,
    pub s: Vec<Sub>,
    pub f: Vec<Flag>,
    pub e: Vec<String>,
    pub x: Vec<ExitCode>,
}

#[derive(Debug, Serialize)]
pub struct Sub {
    pub name: String,
    pub desc: String,
}

#[derive(Debug, Serialize)]
pub struct Flag {
    pub name: String,
    pub short: Option<String>,
    pub r#type: String,
    pub default: Option<String>,
    pub required: bool,
    pub r#enum: Option<Vec<String>>,
    pub desc: String,
}

#[derive(Debug, Serialize)]
pub struct ExitCode {
    pub code: i32,
    pub meaning: String,
}

pub fn schema_for(subcommand: Option<&str>) -> HelpSchema {
    match subcommand {
        Some("scan") => scan_schema(),
        Some("list") => list_schema(),
        Some("remove") => remove_schema(),
        _ => root_schema(),
    }
}

fn root_schema() -> HelpSchema {
    HelpSchema {
        n: "unused-buddy".to_string(),
        d: "Find, list, and safely remove unused JS/TS code".to_string(),
        u: "unused-buddy [GLOBAL OPTIONS] <COMMAND> [ARGS]".to_string(),
        s: vec![
            Sub { name: "scan".into(), desc: "Scan project and print findings.".into() },
            Sub { name: "list".into(), desc: "List findings (human mode by default).".into() },
            Sub { name: "remove".into(), desc: "Remove safe unreachable files.".into() },
            Sub { name: "help".into(), desc: "Show command help.".into() },
        ],
        f: global_flags(),
        e: vec![
            "unused-buddy scan".into(),
            "unused-buddy scan . --format ai".into(),
            "unused-buddy remove . --fix --yes".into(),
        ],
        x: exit_codes(),
    }
}

fn scan_schema() -> HelpSchema {
    HelpSchema {
        n: "scan".to_string(),
        d: "Scan project and print findings.".to_string(),
        u: "unused-buddy scan [path] [GLOBAL OPTIONS]".to_string(),
        s: vec![],
        f: global_flags(),
        e: vec!["unused-buddy scan .".into(), "unused-buddy scan . --fail-on-findings".into()],
        x: exit_codes(),
    }
}

fn list_schema() -> HelpSchema {
    HelpSchema {
        n: "list".to_string(),
        d: "List findings (human mode by default).".to_string(),
        u: "unused-buddy list [path] [GLOBAL OPTIONS]".to_string(),
        s: vec![],
        f: global_flags(),
        e: vec!["unused-buddy list .".into()],
        x: exit_codes(),
    }
}

fn remove_schema() -> HelpSchema {
    let mut flags = global_flags();
    flags.push(Flag {
        name: "fix".into(),
        short: None,
        r#type: "bool".into(),
        default: Some("false".into()),
        required: false,
        r#enum: None,
        desc: "Apply removal changes to disk.".into(),
    });
    flags.push(Flag {
        name: "yes".into(),
        short: None,
        r#type: "bool".into(),
        default: Some("false".into()),
        required: false,
        r#enum: None,
        desc: "Skip interactive confirmation.".into(),
    });

    HelpSchema {
        n: "remove".to_string(),
        d: "Remove safe unreachable files.".to_string(),
        u: "unused-buddy remove [path] [GLOBAL OPTIONS] [--fix] [--yes]".to_string(),
        s: vec![],
        f: flags,
        e: vec![
            "unused-buddy remove .".into(),
            "unused-buddy remove . --fix --yes".into(),
        ],
        x: exit_codes(),
    }
}

fn global_flags() -> Vec<Flag> {
    vec![
        Flag {
            name: "config".into(),
            short: None,
            r#type: "path".into(),
            default: None,
            required: false,
            r#enum: None,
            desc: "Path to config file.".into(),
        },
        Flag {
            name: "format".into(),
            short: None,
            r#type: "string".into(),
            default: Some("human".into()),
            required: false,
            r#enum: Some(vec!["human".into(), "ai".into()]),
            desc: "Output format.".into(),
        },
        Flag {
            name: "color".into(),
            short: None,
            r#type: "string".into(),
            default: Some("auto".into()),
            required: false,
            r#enum: Some(vec!["auto".into(), "always".into(), "never".into()]),
            desc: "Color policy.".into(),
        },
        Flag {
            name: "entry".into(),
            short: None,
            r#type: "path[]".into(),
            default: None,
            required: false,
            r#enum: None,
            desc: "Explicit entry file(s).".into(),
        },
        Flag {
            name: "include".into(),
            short: None,
            r#type: "glob[]".into(),
            default: Some("src/**/*.{js,ts,jsx,tsx}".into()),
            required: false,
            r#enum: None,
            desc: "Include glob(s).".into(),
        },
        Flag {
            name: "exclude".into(),
            short: None,
            r#type: "glob[]".into(),
            default: None,
            required: false,
            r#enum: None,
            desc: "Exclude glob(s).".into(),
        },
        Flag {
            name: "max-workers".into(),
            short: None,
            r#type: "int".into(),
            default: None,
            required: false,
            r#enum: None,
            desc: "Maximum worker threads.".into(),
        },
        Flag {
            name: "fail-on-findings".into(),
            short: None,
            r#type: "bool".into(),
            default: Some("false".into()),
            required: false,
            r#enum: None,
            desc: "Exit non-zero when findings exist.".into(),
        },
    ]
}

fn exit_codes() -> Vec<ExitCode> {
    vec![
        ExitCode { code: 0, meaning: "Success".into() },
        ExitCode { code: 1, meaning: "Findings present with --fail-on-findings or runtime error".into() },
        ExitCode { code: 2, meaning: "Invalid CLI usage".into() },
    ]
}

#[cfg(test)]
mod tests {
    use super::schema_for;

    #[test]
    fn help_ai_deterministic_order() {
        let json = serde_json::to_string(&schema_for(None)).expect("serialize");
        let idx_n = json.find("\"n\"").expect("n");
        let idx_d = json.find("\"d\"").expect("d");
        let idx_u = json.find("\"u\"").expect("u");
        assert!(idx_n < idx_d && idx_d < idx_u);
    }

    #[test]
    fn help_scan_ai_schema() {
        let json = serde_json::to_string(&schema_for(Some("scan"))).expect("serialize");
        assert!(json.contains("\"n\":\"scan\""));
        assert!(!json.contains("\u{001b}"));
    }
}
