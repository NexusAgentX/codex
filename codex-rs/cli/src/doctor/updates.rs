//! Diagnoses whether Codex update paths target the running installation.
//!
//! Update diagnostics combine cached version metadata, install-channel hints,
//! and bounded latest-version probes. For npm-managed launches, this module also
//! verifies that npm install -g would update the package root that launched the
//! current process, which catches PATH and prefix mismatches before the user runs
//! an update command.

use std::collections::HashMap;
use std::path::Path;

use codex_core::config::Config;
use codex_install_context::InstallContext;
use codex_install_context::InstallMethod;
use codex_install_context::ManagedPackage;
use codex_install_context::is_nexus_version;
use codex_tui::CODEX_CLI_VERSION;
use semver::Version;
use serde::Deserialize;
use url::Url;

use super::CheckStatus;
use super::DoctorCheck;
use super::NpmRootCheck;
use super::doctor_install_context;
use super::doctor_managed_by_npm;
use super::npm_global_root_check;
use super::run_command;

const VERSION_FILE_NAME: &str = "version.json";
const GITHUB_LATEST_RELEASE_URL: &str = "https://api.github.com/repos/openai/codex/releases/latest";
const HOMEBREW_CASK_API_URL: &str = "https://formulae.brew.sh/api/cask/codex.json";
const NPM_REGISTRY_URL: &str = "https://registry.npmjs.org/";

/// Builds the update-health row for the current installation.
///
/// Network failures while fetching latest-version metadata degrade the row to a
/// warning instead of failing doctor outright; update freshness is useful
/// support context but should not mask more direct install/config failures.
pub(super) fn updates_check(config: &Config) -> DoctorCheck {
    let current_exe = std::env::current_exe().ok();
    let install_context = doctor_install_context(current_exe.as_deref());
    let managed_package = ManagedPackage::for_codex_version(CODEX_CLI_VERSION);
    let mut details = vec![
        format!(
            "check for update on startup: {}",
            config.check_for_update_on_startup
        ),
        format!(
            "update action: {}",
            update_action_label(&install_context, &managed_package)
        ),
    ];
    let version_file = config.codex_home.join(VERSION_FILE_NAME);
    push_cached_version_details(&mut details, &version_file);

    let mut status = CheckStatus::Ok;
    let mut summary = "update configuration is locally consistent".to_string();
    let mut remediation = None;

    if doctor_managed_by_npm(current_exe.as_deref()) {
        match npm_global_root_check(managed_package.name()) {
            NpmRootCheck::Match { package_root } => {
                details.push(format!("npm update target: {}", package_root.display()));
            }
            NpmRootCheck::Mismatch {
                running_package_root,
                npm_package_root,
            } => {
                status = CheckStatus::Fail;
                summary = "update would target a different npm install".to_string();
                details.push(format!(
                    "running package root: {}",
                    running_package_root.display()
                ));
                details.push(format!("npm package root: {}", npm_package_root.display()));
                remediation = Some(format!(
                    "Fix PATH or npm prefix so the running package root ({}) matches the npm global package root ({}).",
                    running_package_root.display(),
                    npm_package_root.display()
                ));
            }
            NpmRootCheck::MissingPackageRoot => {
                status = status.max(CheckStatus::Warning);
                summary = "npm update target could not be proven".to_string();
                remediation = Some(format!(
                    "Reinstall or update {} so the JS shim provides CODEX_MANAGED_PACKAGE_ROOT.",
                    managed_package.name()
                ));
            }
            NpmRootCheck::NpmUnavailable(error) => {
                status = status.max(CheckStatus::Warning);
                summary = "npm update target could not be inspected".to_string();
                details.push(format!("npm root -g failed: {error}"));
            }
        }
    }

    if nexus_updates_require_package_manager(&install_context) {
        details.push(
            "latest version probe: disabled for Nexus installs not managed by npm, bun, or pnpm"
                .to_string(),
        );
    } else {
        match fetch_latest_version(&install_context, &managed_package) {
            Ok(latest_version) => {
                details.push(format!("latest version: {latest_version}"));
                let current_version = installed_version(&install_context, &managed_package);
                if is_newer(&latest_version, current_version) == Some(true) {
                    details.push("latest version status: newer version is available".to_string());
                } else {
                    details.push("latest version status: current version is not older".to_string());
                }
            }
            Err(err) => {
                status = status.max(CheckStatus::Warning);
                details.push(format!("latest version probe: {err}"));
            }
        }
    }

    let mut check = DoctorCheck::new("updates.status", "updates", status, summary).details(details);
    if let Some(remediation) = remediation {
        check = check.remediation(remediation);
    }
    check
}

fn push_cached_version_details(details: &mut Vec<String>, version_file: &Path) {
    details.push(format!("version cache: {}", version_file.display()));
    match std::fs::read_to_string(version_file) {
        Ok(contents) => match serde_json::from_str::<VersionInfo>(&contents) {
            Ok(info) => {
                details.push(format!("cached latest version: {}", info.latest_version));
                if let Some(source) = info.source {
                    details.push(format!("cached version source: {source}"));
                }
                if let Some(last_checked_at) = info.last_checked_at {
                    details.push(format!("last checked at: {last_checked_at}"));
                }
                if let Some(dismissed_version) = info.dismissed_version {
                    details.push(format!("dismissed version: {dismissed_version}"));
                }
            }
            Err(err) => details.push(format!("version cache parse: {err}")),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            details.push("version cache: missing".to_string());
        }
        Err(err) => details.push(format!("version cache read: {err}")),
    }
}

fn update_action_label(context: &InstallContext, package: &ManagedPackage) -> String {
    if nexus_updates_require_package_manager(context) {
        return "unavailable (Nexus package-manager install required)".to_string();
    }

    match &context.method {
        InstallMethod::Npm => format!("npm install -g {}", package.name()),
        InstallMethod::Bun => format!("bun install -g {}", package.name()),
        InstallMethod::Pnpm => format!("pnpm add -g {}", package.name()),
        InstallMethod::Brew => "brew upgrade --cask codex".to_string(),
        InstallMethod::Standalone { .. } => "standalone installer".to_string(),
        InstallMethod::Other => "manual or unknown".to_string(),
    }
}

fn fetch_latest_version(
    context: &InstallContext,
    package: &ManagedPackage,
) -> Result<String, String> {
    if nexus_updates_require_package_manager(context) {
        return Err(
            "Nexus updates require an installation managed by npm, bun, or pnpm".to_string(),
        );
    }

    match &context.method {
        InstallMethod::Brew => fetch_homebrew_cask_version(),
        InstallMethod::Npm | InstallMethod::Bun | InstallMethod::Pnpm => {
            fetch_latest_npm_version(package.name())
        }
        InstallMethod::Standalone { .. } | InstallMethod::Other => {
            fetch_latest_github_release_version()
        }
    }
}

fn nexus_updates_require_package_manager(context: &InstallContext) -> bool {
    is_nexus_version(CODEX_CLI_VERSION)
        && !matches!(
            &context.method,
            InstallMethod::Npm | InstallMethod::Bun | InstallMethod::Pnpm
        )
}

fn installed_version<'a>(context: &InstallContext, package: &'a ManagedPackage) -> &'a str {
    match &context.method {
        InstallMethod::Npm | InstallMethod::Bun | InstallMethod::Pnpm => package.version(),
        InstallMethod::Brew | InstallMethod::Standalone { .. } | InstallMethod::Other => {
            CODEX_CLI_VERSION
        }
    }
}

fn fetch_latest_npm_version(package_name: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct NpmPackageInfo {
        #[serde(rename = "dist-tags")]
        dist_tags: HashMap<String, String>,
    }

    let url = npm_registry_package_url(package_name)?;
    let info = http_get_json::<NpmPackageInfo>(url.as_str())?;
    info.dist_tags
        .get("latest")
        .map(String::as_str)
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("npm package {package_name} is missing latest dist-tag"))
}

fn npm_registry_package_url(package_name: &str) -> Result<Url, String> {
    let mut url = Url::parse(NPM_REGISTRY_URL).map_err(|err| err.to_string())?;
    url.path_segments_mut()
        .map_err(|_| "npm registry URL cannot contain package paths".to_string())?
        .push(package_name);
    Ok(url)
}

fn fetch_latest_github_release_version() -> Result<String, String> {
    #[derive(Deserialize)]
    struct ReleaseInfo {
        tag_name: String,
    }

    let info = http_get_json::<ReleaseInfo>(GITHUB_LATEST_RELEASE_URL)?;
    info.tag_name
        .strip_prefix("rust-v")
        .map(str::to_string)
        .ok_or_else(|| format!("failed to parse latest tag {}", info.tag_name))
}

fn fetch_homebrew_cask_version() -> Result<String, String> {
    #[derive(Deserialize)]
    struct HomebrewCaskInfo {
        version: String,
    }

    http_get_json::<HomebrewCaskInfo>(HOMEBREW_CASK_API_URL).map(|info| info.version)
}

fn http_get_json<T>(url: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let body = run_command("curl", ["-fsSL", "--max-time", "5", url])?;
    serde_json::from_str::<T>(&body).map_err(|err| err.to_string())
}

fn is_newer(latest: &str, current: &str) -> Option<bool> {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => Some(latest > current),
        (Some(_), None) | (None, Some(_)) | (None, None) => None,
    }
}

fn parse_version(value: &str) -> Option<Version> {
    Version::parse(value.trim()).ok()
}

#[derive(Deserialize)]
struct VersionInfo {
    latest_version: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    last_checked_at: Option<String>,
    #[serde(default)]
    dismissed_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_newer_compares_plain_semver() {
        assert_eq!(is_newer("1.2.4", "1.2.3"), Some(true));
        assert_eq!(is_newer("1.2.3", "1.2.4"), Some(false));
        assert_eq!(is_newer("1.2.3-beta.1", "1.2.2"), Some(true));
        assert_eq!(is_newer("0.144.3-nexus.2", "0.144.3-nexus.1"), Some(true));
    }

    #[test]
    fn update_action_labels_install_contexts() {
        let package = ManagedPackage::from_parts("@nexus-agent-x/codex", "0.144.3-nexus.1")
            .expect("valid managed package");
        assert_eq!(
            update_action_label(
                &InstallContext {
                    method: InstallMethod::Npm,
                    package_layout: None,
                },
                &package,
            ),
            "npm install -g @nexus-agent-x/codex"
        );
        assert_eq!(
            update_action_label(
                &InstallContext {
                    method: InstallMethod::Pnpm,
                    package_layout: None,
                },
                &package,
            ),
            "pnpm add -g @nexus-agent-x/codex"
        );
        assert_eq!(
            update_action_label(
                &InstallContext {
                    method: InstallMethod::Other,
                    package_layout: None,
                },
                &package,
            ),
            if is_nexus_version(CODEX_CLI_VERSION) {
                "unavailable (Nexus package-manager install required)"
            } else {
                "manual or unknown"
            }
        );
    }

    #[test]
    fn npm_registry_url_targets_managed_scope() {
        assert_eq!(
            npm_registry_package_url("@nexus-agent-x/codex")
                .expect("valid registry URL")
                .as_str(),
            "https://registry.npmjs.org/@nexus-agent-x%2Fcodex"
        );
    }
}
