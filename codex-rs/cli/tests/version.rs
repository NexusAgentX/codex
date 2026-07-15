use anyhow::Result;
use codex_tui::CODEX_CLI_VERSION;
use pretty_assertions::assert_eq;
use std::process::Command;

#[test]
fn version_reports_embedded_cli_version() -> Result<()> {
    let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .arg("--version")
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    let (_, version) = stdout
        .trim()
        .rsplit_once(' ')
        .expect("version output should contain the program name and version");
    assert_eq!(version, CODEX_CLI_VERSION);
    Ok(())
}
