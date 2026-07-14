import json
import shutil
import tarfile
import tempfile
import unittest
from pathlib import Path

import build_npm_package
import validate_nexus_npm_release


NPM_VERSION = "0.144.3-nexus.1"


class ValidateNexusNpmReleaseTest(unittest.TestCase):
    def test_accepts_complete_release_set_built_by_packager(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tarball_dir = self._build_release_set(Path(temp_dir))

            validate_nexus_npm_release.validate_release_set(tarball_dir, NPM_VERSION)

    def test_accepts_single_platform_tarball(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tarball_dir = self._build_release_set(Path(temp_dir))

            validate_nexus_npm_release.validate_release_set(
                tarball_dir, NPM_VERSION, "codex-win32-arm64"
            )

    def test_rejects_missing_platform_tarball(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tarball_dir = self._build_release_set(Path(temp_dir))
            missing_tarball = validate_nexus_npm_release.platform_tarball_path(
                tarball_dir, NPM_VERSION, "codex-darwin-arm64"
            )
            missing_tarball.unlink()

            with self.assertRaisesRegex(
                validate_nexus_npm_release.ReleaseValidationError,
                "tarball not found",
            ):
                validate_nexus_npm_release.validate_release_set(
                    tarball_dir, NPM_VERSION
                )

    def test_rejects_tampered_platform_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tarball_dir = self._build_release_set(Path(temp_dir))
            tarball_path = validate_nexus_npm_release.platform_tarball_path(
                tarball_dir, NPM_VERSION, "codex-linux-x64"
            )
            self._rewrite_json_member(
                tarball_path,
                "package/vendor/x86_64-unknown-linux-musl/codex-package.json",
                {"target": "wrong-target"},
            )

            with self.assertRaisesRegex(
                validate_nexus_npm_release.ReleaseValidationError,
                "invalid codex-package.json",
            ):
                validate_nexus_npm_release.validate_release_set(
                    tarball_dir, NPM_VERSION, "codex-linux-x64"
                )

    def test_rejects_non_executable_unix_entrypoint(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tarball_dir = self._build_release_set(Path(temp_dir))
            tarball_path = validate_nexus_npm_release.platform_tarball_path(
                tarball_dir, NPM_VERSION, "codex-linux-x64"
            )
            self._set_tarball_member_modes(
                tarball_path,
                {"package/vendor/x86_64-unknown-linux-musl/bin/codex"},
                0o644,
            )

            with self.assertRaisesRegex(
                validate_nexus_npm_release.ReleaseValidationError,
                "is not executable",
            ):
                validate_nexus_npm_release.validate_release_set(
                    tarball_dir, NPM_VERSION, "codex-linux-x64"
                )

    def _build_release_set(self, temp_path: Path) -> Path:
        tarball_dir = temp_path / "tarballs"
        tarball_dir.mkdir()
        vendor_src = temp_path / "vendor"

        for platform_name, config in build_npm_package.CODEX_PLATFORM_PACKAGES.items():
            target = config["target_triple"]
            self._write_vendor_fixture(
                vendor_src / target, target, config["os"] == "win32"
            )
            staging_dir = temp_path / f"staging-{platform_name}"
            staging_dir.mkdir()
            build_npm_package.stage_sources(staging_dir, NPM_VERSION, platform_name)
            build_npm_package.copy_native_binaries(
                vendor_src,
                staging_dir,
                build_npm_package.PACKAGE_NATIVE_COMPONENTS[platform_name],
                target_filter={target},
            )
            tarball_path = validate_nexus_npm_release.platform_tarball_path(
                tarball_dir, NPM_VERSION, platform_name
            )
            build_npm_package.run_npm_pack(
                staging_dir,
                tarball_path,
            )
            if config["os"] != "win32":
                executable_files = {
                    f"package/vendor/{target}/bin/codex",
                    f"package/vendor/{target}/bin/codex-code-mode-host",
                    f"package/vendor/{target}/codex-path/rg",
                    f"package/vendor/{target}/codex-resources/zsh/bin/zsh",
                }
                if config["os"] == "linux":
                    executable_files.add(
                        f"package/vendor/{target}/codex-resources/bwrap"
                    )
                self._set_tarball_member_modes(tarball_path, executable_files, 0o755)

        root_staging_dir = temp_path / "staging-codex"
        root_staging_dir.mkdir()
        build_npm_package.stage_sources(root_staging_dir, NPM_VERSION, "codex")
        build_npm_package.run_npm_pack(
            root_staging_dir, tarball_dir / f"codex-npm-{NPM_VERSION}.tgz"
        )
        return tarball_dir

    @staticmethod
    def _write_vendor_fixture(target_dir: Path, target: str, is_windows: bool) -> None:
        exe_suffix = ".exe" if is_windows else ""
        required_files = [
            target_dir / "bin" / f"codex{exe_suffix}",
            target_dir / "bin" / f"codex-code-mode-host{exe_suffix}",
            target_dir / "codex-path" / f"rg{exe_suffix}",
        ]
        if "linux" in target:
            required_files.append(target_dir / "codex-resources" / "bwrap")
        if not is_windows:
            required_files.append(
                target_dir / "codex-resources" / "zsh" / "bin" / "zsh"
            )
        if is_windows:
            required_files.extend(
                [
                    target_dir / "codex-resources" / "codex-command-runner.exe",
                    target_dir / "codex-resources" / "codex-windows-sandbox-setup.exe",
                ]
            )
        for path in required_files:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"fixture")

        metadata = {
            "layoutVersion": 1,
            "version": NPM_VERSION,
            "target": target,
            "variant": "codex",
            "entrypoint": f"bin/codex{exe_suffix}",
            "resourcesDir": "codex-resources",
            "pathDir": "codex-path",
        }
        (target_dir / "codex-package.json").write_text(
            json.dumps(metadata), encoding="utf-8"
        )

    @staticmethod
    def _rewrite_json_member(
        tarball_path: Path, member_name: str, replacement: dict[str, str]
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            with tarfile.open(tarball_path, "r:gz") as source_archive:
                source_archive.extractall(temp_path, filter="data")
            (temp_path / member_name).write_text(
                json.dumps(replacement), encoding="utf-8"
            )
            rewritten = temp_path / "rewritten.tgz"
            with tarfile.open(rewritten, "w:gz") as output_archive:
                output_archive.add(temp_path / "package", arcname="package")
            shutil.copyfile(rewritten, tarball_path)

    @staticmethod
    def _set_tarball_member_modes(
        tarball_path: Path, member_names: set[str], mode: int
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            rewritten = Path(temp_dir) / "rewritten.tgz"
            found_members: set[str] = set()
            with (
                tarfile.open(tarball_path, "r:gz") as source_archive,
                tarfile.open(rewritten, "w:gz") as output_archive,
            ):
                for member in source_archive.getmembers():
                    member_file = source_archive.extractfile(member)
                    if member.name in member_names:
                        member.mode = mode
                        found_members.add(member.name)
                    output_archive.addfile(member, member_file)

            if found_members != member_names:
                missing = sorted(member_names - found_members)
                raise AssertionError(f"tarball members not found: {missing}")
            shutil.copyfile(rewritten, tarball_path)


if __name__ == "__main__":
    unittest.main()
