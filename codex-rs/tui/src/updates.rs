#![cfg(not(debug_assertions))]

use crate::legacy_core::config::Config;
use crate::npm_registry;
use crate::npm_registry::NpmPackageInfo;
use crate::update_action;
use crate::update_action::UpdateAction;
use crate::update_versions::extract_version_from_latest_tag;
use crate::update_versions::is_newer;
use crate::update_versions::is_source_build_version;
use crate::updates_cache::VersionInfo;
use crate::updates_cache::read_version_info;
use crate::updates_cache::version_filepath;
use chrono::Duration;
use chrono::Utc;
use codex_install_context::is_nexus_version;
use codex_login::default_client::create_client;
use serde::Deserialize;
use std::path::Path;

use crate::version::CODEX_CLI_VERSION;

pub(crate) use crate::updates_cache::dismiss_version;

const GITHUB_RELEASE_SOURCE: &str = "github:openai/codex";
const HOMEBREW_CASK_SOURCE: &str = "homebrew:codex";

#[derive(Clone)]
struct UpdateCheck {
    action: Option<UpdateAction>,
    source: String,
    current_version: String,
}

pub fn get_upgrade_version(config: &Config) -> Option<String> {
    if !config.check_for_update_on_startup || is_source_build_version(CODEX_CLI_VERSION) {
        return None;
    }

    let update_check = current_update_check()?;
    let version_file = version_filepath(config);
    let info = read_version_info(&version_file)
        .ok()
        .filter(|info| cache_matches_source(info, &update_check.source));

    if match &info {
        None => true,
        Some(info) => info.last_checked_at < Utc::now() - Duration::hours(20),
    } {
        // Refresh the cached latest version in the background so TUI startup
        // isn’t blocked by a network call. The UI reads the previously cached
        // value (if any) for this run; the next run shows the banner if needed.
        let background_check = update_check.clone();
        tokio::spawn(async move {
            check_for_update(
                &version_file,
                background_check.action,
                background_check.source,
            )
            .await
            .inspect_err(|e| tracing::error!("Failed to update version: {e}"))
        });
    }

    info.and_then(|info| {
        if is_newer(&info.latest_version, &update_check.current_version).unwrap_or(false) {
            Some(info.latest_version)
        } else {
            None
        }
    })
}

// We use the latest version from the cask if installation is via homebrew - homebrew does not immediately pick up the latest release and can lag behind.
const HOMEBREW_CASK_API_URL: &str = "https://formulae.brew.sh/api/cask/codex.json";
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/openai/codex/releases/latest";

#[derive(Deserialize, Debug, Clone)]
struct ReleaseInfo {
    tag_name: String,
}

#[derive(Deserialize, Debug, Clone)]
struct HomebrewCaskInfo {
    version: String,
}

fn current_update_check() -> Option<UpdateCheck> {
    let action = update_action::get_update_action();
    if action.is_none() && is_nexus_version(CODEX_CLI_VERSION) {
        return None;
    }

    let (source, current_version) = match action.as_ref() {
        Some(UpdateAction::NpmGlobalLatest(package))
        | Some(UpdateAction::BunGlobalLatest(package))
        | Some(UpdateAction::PnpmGlobalLatest(package)) => {
            (package.cache_key(), package.version().to_string())
        }
        Some(UpdateAction::BrewUpgrade) => (
            HOMEBREW_CASK_SOURCE.to_string(),
            CODEX_CLI_VERSION.to_string(),
        ),
        Some(UpdateAction::StandaloneUnix) | Some(UpdateAction::StandaloneWindows) | None => (
            GITHUB_RELEASE_SOURCE.to_string(),
            CODEX_CLI_VERSION.to_string(),
        ),
    };

    Some(UpdateCheck {
        action,
        source,
        current_version,
    })
}

fn cache_matches_source(info: &VersionInfo, source: &str) -> bool {
    match info.source.as_deref() {
        Some(cached_source) => cached_source == source,
        None => source == GITHUB_RELEASE_SOURCE,
    }
}

async fn check_for_update(
    version_file: &Path,
    action: Option<UpdateAction>,
    source: String,
) -> anyhow::Result<()> {
    let latest_version = match action {
        Some(UpdateAction::BrewUpgrade) => {
            let HomebrewCaskInfo { version } = create_client()
                .get(HOMEBREW_CASK_API_URL)
                .send()
                .await?
                .error_for_status()?
                .json::<HomebrewCaskInfo>()
                .await?;
            version
        }
        Some(UpdateAction::NpmGlobalLatest(package))
        | Some(UpdateAction::BunGlobalLatest(package))
        | Some(UpdateAction::PnpmGlobalLatest(package)) => {
            let package_info = create_client()
                .get(npm_registry::package_url(package.name())?)
                .send()
                .await?
                .error_for_status()?
                .json::<NpmPackageInfo>()
                .await?;
            npm_registry::latest_ready_version(&package_info)?
        }
        Some(UpdateAction::StandaloneUnix) | Some(UpdateAction::StandaloneWindows) | None => {
            fetch_latest_github_release_version().await?
        }
    };

    // Preserve any previously dismissed version if present.
    let dismissed_version = read_version_info(version_file)
        .ok()
        .filter(|info| cache_matches_source(info, &source))
        .and_then(|info| info.dismissed_version);
    let info = VersionInfo {
        latest_version,
        source: Some(source),
        last_checked_at: Utc::now(),
        dismissed_version,
    };

    let json_line = format!("{}\n", serde_json::to_string(&info)?);
    if let Some(parent) = version_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(version_file, json_line).await?;
    Ok(())
}

async fn fetch_latest_github_release_version() -> anyhow::Result<String> {
    let ReleaseInfo {
        tag_name: latest_tag_name,
    } = create_client()
        .get(LATEST_RELEASE_URL)
        .send()
        .await?
        .error_for_status()?
        .json::<ReleaseInfo>()
        .await?;
    extract_version_from_latest_tag(&latest_tag_name)
}

/// Returns the latest version to show in a popup, if it should be shown.
/// This respects the user's dismissal choice for the current latest version.
pub fn get_upgrade_version_for_popup(config: &Config) -> Option<String> {
    if !config.check_for_update_on_startup || is_source_build_version(CODEX_CLI_VERSION) {
        return None;
    }

    let version_file = version_filepath(config);
    let latest = get_upgrade_version(config)?;
    // If the user dismissed this exact version previously, do not show the popup.
    if let Ok(info) = read_version_info(&version_file)
        && info.dismissed_version.as_deref() == Some(latest.as_str())
    {
        return None;
    }
    Some(latest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached_info(source: Option<&str>) -> VersionInfo {
        VersionInfo {
            latest_version: "0.145.0-nexus.1".to_string(),
            source: source.map(str::to_string),
            last_checked_at: Utc::now(),
            dismissed_version: None,
        }
    }

    #[test]
    fn nexus_cache_never_reuses_legacy_official_version() {
        let legacy_info = cached_info(None);

        assert!(cache_matches_source(&legacy_info, GITHUB_RELEASE_SOURCE));
        assert!(!cache_matches_source(
            &legacy_info,
            "npm:@nexus-agent-x/codex"
        ));
    }

    #[test]
    fn version_cache_is_scoped_to_exact_package_source() {
        let nexus_info = cached_info(Some("npm:@nexus-agent-x/codex"));

        assert!(cache_matches_source(
            &nexus_info,
            "npm:@nexus-agent-x/codex"
        ));
        assert!(!cache_matches_source(&nexus_info, "npm:@openai/codex"));
    }
}
