/// The current Codex CLI version as embedded at compile time.
///
/// Downstream releases can override the workspace version without editing
/// Cargo manifests by setting `CODEX_CLI_VERSION_OVERRIDE` during compilation.
pub const CODEX_CLI_VERSION: &str = match option_env!("CODEX_CLI_VERSION_OVERRIDE") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};
