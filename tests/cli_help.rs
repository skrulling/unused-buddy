#[test]
fn help_root_human_snapshot() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("unused-buddy");
    cmd.arg("--help");
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("Find, list, and safely remove unused JS/TS code"));
    assert!(text.contains("scan"));
    assert!(text.contains("list"));
    assert!(text.contains("remove"));
}

#[test]
fn help_scan_human_snapshot() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("unused-buddy");
    cmd.args(["scan", "--help"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("Scan project and print findings"));
}

#[test]
fn help_list_human_snapshot() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("unused-buddy");
    cmd.args(["list", "--help"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("List findings"));
}

#[test]
fn help_remove_human_snapshot() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("unused-buddy");
    cmd.args(["remove", "--help"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("Remove safe unreachable files"));
    assert!(text.contains("--fix"));
}

#[test]
fn help_root_ai_schema() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("unused-buddy");
    cmd.args(["--help", "--format", "ai"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).expect("utf8");
    let value: serde_json::Value = serde_json::from_str(text.trim()).expect("json");
    assert_eq!(value["n"], "unused-buddy");
    assert!(value["f"].is_array());
}

#[test]
fn help_scan_ai_schema() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("unused-buddy");
    cmd.args(["scan", "--help", "--format", "ai"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).expect("utf8");
    let value: serde_json::Value = serde_json::from_str(text.trim()).expect("json");
    assert_eq!(value["n"], "scan");
}

#[test]
fn help_remove_ai_schema() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("unused-buddy");
    cmd.args(["remove", "--help", "--format", "ai"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).expect("utf8");
    let value: serde_json::Value = serde_json::from_str(text.trim()).expect("json");
    assert_eq!(value["n"], "remove");
}

#[test]
fn help_ai_no_ansi_sequences() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("unused-buddy");
    cmd.args(["--help", "--format", "ai"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).expect("utf8");
    assert!(!text.contains("\u{1b}"));
}
