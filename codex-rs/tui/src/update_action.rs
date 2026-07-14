#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallContext;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallMethod;
use codex_install_context::ManagedPackage;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::StandalonePlatform;
#[cfg(not(debug_assertions))]
use codex_install_context::is_nexus_version;

const OFFICIAL_RELEASE_NOTES_URL: &str = "https://github.com/openai/codex/releases/latest";
const NEXUS_RELEASE_NOTES_URL: &str = "https://github.com/NexusAgentX/codex/releases/latest";

/// Update action the CLI should perform after the TUI exits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateAction {
    /// Update the package that launched Codex via npm.
    NpmGlobalLatest(ManagedPackage),
    /// Update the package that launched Codex via Bun.
    BunGlobalLatest(ManagedPackage),
    /// Update the package that launched Codex via pnpm.
    PnpmGlobalLatest(ManagedPackage),
    /// Update via `brew upgrade codex`.
    BrewUpgrade,
    /// Update via `curl -fsSL https://chatgpt.com/codex/install.sh | CODEX_NON_INTERACTIVE=1 sh`.
    StandaloneUnix,
    /// Update via `$env:CODEX_NON_INTERACTIVE=1; irm https://chatgpt.com/codex/install.ps1 | iex`.
    StandaloneWindows,
}

impl UpdateAction {
    #[cfg(not(debug_assertions))]
    pub(crate) fn from_install_context(context: &InstallContext) -> Option<Self> {
        Self::from_install_context_for_version(context, crate::version::CODEX_CLI_VERSION)
    }

    #[cfg(not(debug_assertions))]
    fn from_install_context_for_version(
        context: &InstallContext,
        cli_version: &str,
    ) -> Option<Self> {
        let package = ManagedPackage::for_codex_version(cli_version);
        Self::from_install_context_with_package(context, package, is_nexus_version(cli_version))
    }

    #[cfg(any(not(debug_assertions), test))]
    fn from_install_context_with_package(
        context: &InstallContext,
        package: ManagedPackage,
        nexus_build: bool,
    ) -> Option<Self> {
        match &context.method {
            InstallMethod::Npm => Some(UpdateAction::NpmGlobalLatest(package)),
            InstallMethod::Bun => Some(UpdateAction::BunGlobalLatest(package)),
            InstallMethod::Pnpm => Some(UpdateAction::PnpmGlobalLatest(package)),
            InstallMethod::Brew if !nexus_build => Some(UpdateAction::BrewUpgrade),
            InstallMethod::Standalone { platform, .. } if !nexus_build => Some(match platform {
                StandalonePlatform::Unix => UpdateAction::StandaloneUnix,
                StandalonePlatform::Windows => UpdateAction::StandaloneWindows,
            }),
            InstallMethod::Brew | InstallMethod::Standalone { .. } => None,
            InstallMethod::Other => None,
        }
    }

    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(&self) -> (&'static str, Vec<&str>) {
        match self {
            UpdateAction::NpmGlobalLatest(package) => {
                ("npm", vec!["install", "-g", package.name()])
            }
            UpdateAction::BunGlobalLatest(package) => {
                ("bun", vec!["install", "-g", package.name()])
            }
            UpdateAction::PnpmGlobalLatest(package) => ("pnpm", vec!["add", "-g", package.name()]),
            UpdateAction::BrewUpgrade => ("brew", vec!["upgrade", "--cask", "codex"]),
            UpdateAction::StandaloneUnix => (
                "sh",
                vec![
                    "-c",
                    "curl -fsSL https://chatgpt.com/codex/install.sh | CODEX_NON_INTERACTIVE=1 sh",
                ],
            ),
            UpdateAction::StandaloneWindows => (
                "powershell",
                vec![
                    "-ExecutionPolicy",
                    "Bypass",
                    "-c",
                    "$env:CODEX_NON_INTERACTIVE=1; irm https://chatgpt.com/codex/install.ps1 | iex",
                ],
            ),
        }
    }

    /// Returns string representation of the command-line arguments for invoking the update.
    pub fn command_str(&self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }

    pub(crate) fn managed_package(&self) -> Option<&ManagedPackage> {
        match self {
            UpdateAction::NpmGlobalLatest(package)
            | UpdateAction::BunGlobalLatest(package)
            | UpdateAction::PnpmGlobalLatest(package) => Some(package),
            UpdateAction::BrewUpgrade
            | UpdateAction::StandaloneUnix
            | UpdateAction::StandaloneWindows => None,
        }
    }

    pub(crate) fn release_notes_url(&self) -> &'static str {
        if self
            .managed_package()
            .is_some_and(|package| package.name() == codex_install_context::NEXUS_CODEX_NPM_PACKAGE)
        {
            NEXUS_RELEASE_NOTES_URL
        } else {
            OFFICIAL_RELEASE_NOTES_URL
        }
    }
}

#[cfg(not(debug_assertions))]
pub fn get_update_action() -> Option<UpdateAction> {
    UpdateAction::from_install_context(InstallContext::current())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_install_context::NEXUS_CODEX_NPM_PACKAGE;
    use codex_install_context::OFFICIAL_CODEX_NPM_PACKAGE;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    fn package(name: &str, version: &str) -> ManagedPackage {
        ManagedPackage::from_parts(name, version).expect("valid managed package")
    }

    fn action_for(method: InstallMethod) -> Option<UpdateAction> {
        UpdateAction::from_install_context_with_package(
            &InstallContext {
                method,
                package_layout: None,
            },
            package(OFFICIAL_CODEX_NPM_PACKAGE, "1.2.3"),
            false,
        )
    }

    #[test]
    fn maps_install_context_to_update_action() {
        let native_release_dir =
            AbsolutePathBuf::from_absolute_path(std::env::temp_dir().join("native-release"))
                .expect("temp dir path should be absolute");

        assert_eq!(action_for(InstallMethod::Other), None);
        assert_eq!(
            action_for(InstallMethod::Npm),
            Some(UpdateAction::NpmGlobalLatest(package(
                OFFICIAL_CODEX_NPM_PACKAGE,
                "1.2.3"
            )))
        );
        assert_eq!(
            action_for(InstallMethod::Bun),
            Some(UpdateAction::BunGlobalLatest(package(
                OFFICIAL_CODEX_NPM_PACKAGE,
                "1.2.3"
            )))
        );
        assert_eq!(
            action_for(InstallMethod::Pnpm),
            Some(UpdateAction::PnpmGlobalLatest(package(
                OFFICIAL_CODEX_NPM_PACKAGE,
                "1.2.3"
            )))
        );
        assert_eq!(
            action_for(InstallMethod::Brew),
            Some(UpdateAction::BrewUpgrade)
        );
        assert_eq!(
            action_for(InstallMethod::Standalone {
                platform: StandalonePlatform::Unix,
                release_dir: native_release_dir.clone(),
                resources_dir: Some(native_release_dir.join("codex-resources")),
            }),
            Some(UpdateAction::StandaloneUnix)
        );
        assert_eq!(
            action_for(InstallMethod::Standalone {
                platform: StandalonePlatform::Windows,
                release_dir: native_release_dir.clone(),
                resources_dir: Some(native_release_dir.join("codex-resources")),
            }),
            Some(UpdateAction::StandaloneWindows)
        );
    }

    #[test]
    fn nexus_builds_only_offer_package_manager_updates() {
        let nexus_package = package(NEXUS_CODEX_NPM_PACKAGE, "0.144.3-nexus.1");
        let standalone_context = InstallContext {
            method: InstallMethod::Standalone {
                platform: StandalonePlatform::Unix,
                release_dir: AbsolutePathBuf::from_absolute_path("/tmp/nexus-release")
                    .expect("absolute release path"),
                resources_dir: None,
            },
            package_layout: None,
        };

        for (method, expected_command) in [
            (InstallMethod::Npm, "npm install -g @nexus-agent-x/codex"),
            (InstallMethod::Bun, "bun install -g @nexus-agent-x/codex"),
            (InstallMethod::Pnpm, "pnpm add -g @nexus-agent-x/codex"),
        ] {
            let action = UpdateAction::from_install_context_with_package(
                &InstallContext {
                    method,
                    package_layout: None,
                },
                nexus_package.clone(),
                true,
            )
            .expect("package-manager update action");
            assert_eq!(action.command_str(), expected_command);
            assert_eq!(action.release_notes_url(), NEXUS_RELEASE_NOTES_URL);
        }
        assert_eq!(
            UpdateAction::from_install_context_with_package(
                &standalone_context,
                nexus_package,
                true,
            ),
            None
        );
    }

    #[test]
    fn standalone_update_commands_rerun_latest_installer() {
        assert_eq!(
            UpdateAction::StandaloneUnix.command_args(),
            (
                "sh",
                vec![
                    "-c",
                    "curl -fsSL https://chatgpt.com/codex/install.sh | CODEX_NON_INTERACTIVE=1 sh"
                ],
            )
        );
        assert_eq!(
            UpdateAction::StandaloneWindows.command_args(),
            (
                "powershell",
                vec![
                    "-ExecutionPolicy",
                    "Bypass",
                    "-c",
                    "$env:CODEX_NON_INTERACTIVE=1; irm https://chatgpt.com/codex/install.ps1 | iex"
                ],
            )
        );
    }
}
