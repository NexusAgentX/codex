import json
import os
import platform
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import build_npm_package


NPM_NAME = "@nexus-agent-x/codex"
NPM_VERSION = "0.144.3-nexus.1"


class BuildNpmPackageTest(unittest.TestCase):
    def test_resolves_npm_executable_before_packing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            staging_dir = temp_path / "staging"
            staging_dir.mkdir()
            output_path = temp_path / "codex.tgz"
            resolved_npm = r"C:\hostedtoolcache\node\npm.cmd"

            def fake_npm_pack(command: list[str], **_: object) -> str:
                self.assertEqual(command[0], resolved_npm)
                pack_dir = Path(command[command.index("--pack-destination") + 1])
                (pack_dir / "packed.tgz").touch()
                return json.dumps([{"filename": "packed.tgz"}])

            with (
                mock.patch.object(
                    build_npm_package.shutil,
                    "which",
                    return_value=resolved_npm,
                ),
                mock.patch.object(
                    build_npm_package.subprocess,
                    "check_output",
                    side_effect=fake_npm_pack,
                ),
            ):
                build_npm_package.run_npm_pack(staging_dir, output_path)

            self.assertTrue(output_path.is_file())

    def test_root_package_uses_nexus_platform_aliases(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            staging_dir = Path(temp_dir) / "staging"
            staging_dir.mkdir()

            build_npm_package.stage_sources(staging_dir, NPM_VERSION, "codex")

            package_json = json.loads(
                (staging_dir / "package.json").read_text(encoding="utf-8")
            )
            self.assertEqual(package_json["name"], NPM_NAME)
            expected_dependencies = {
                f"{NPM_NAME}-{config['npm_tag']}": (
                    f"npm:{NPM_NAME}@{NPM_VERSION}-{config['npm_tag']}"
                )
                for config in build_npm_package.CODEX_PLATFORM_PACKAGES.values()
            }
            self.assertEqual(
                package_json["optionalDependencies"], expected_dependencies
            )
            self.assertEqual(package_json["publishConfig"], {"access": "public"})
            self.assertIn(
                "npm install -g @nexus-agent-x/codex",
                (staging_dir / "README.md").read_text(encoding="utf-8"),
            )

    def test_platform_package_uses_nexus_name_and_version(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            staging_dir = Path(temp_dir) / "staging"
            staging_dir.mkdir()

            build_npm_package.stage_sources(staging_dir, NPM_VERSION, "codex-linux-x64")

            package_json = json.loads(
                (staging_dir / "package.json").read_text(encoding="utf-8")
            )
            self.assertEqual(package_json["name"], NPM_NAME)
            self.assertEqual(package_json["version"], f"{NPM_VERSION}-linux-x64")
            self.assertEqual(package_json["os"], ["linux"])
            self.assertEqual(package_json["cpu"], ["x64"])
            self.assertEqual(package_json["publishConfig"], {"access": "public"})

    def test_npm_pack_contains_nexus_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            staging_dir = temp_path / "staging"
            staging_dir.mkdir()
            tarball_path = temp_path / "codex.tgz"

            build_npm_package.stage_sources(staging_dir, NPM_VERSION, "codex")
            build_npm_package.run_npm_pack(staging_dir, tarball_path)

            with tarfile.open(tarball_path, "r:gz") as archive:
                package_json_file = archive.extractfile("package/package.json")
                self.assertIsNotNone(package_json_file)
                package_json = json.load(package_json_file)
                self.assertEqual(package_json["name"], NPM_NAME)
                self.assertEqual(package_json["version"], NPM_VERSION)
                self.assertIsNotNone(archive.getmember("package/README.md"))

    @unittest.skipUnless(os.name == "posix", "fixture executable uses a shebang")
    @unittest.skipUnless(shutil.which("node"), "node is required")
    def test_js_wrapper_resolves_nexus_platform_alias(self) -> None:
        self._run_js_wrapper_fixture("npm", "CODEX_MANAGED_BY_NPM")

    @unittest.skipUnless(os.name == "posix", "fixture executable uses symlinks")
    @unittest.skipUnless(shutil.which("node"), "node is required")
    def test_js_wrapper_detects_pnpm_owned_nexus_install(self) -> None:
        self._run_js_wrapper_fixture("pnpm", "CODEX_MANAGED_BY_PNPM")

    def _run_js_wrapper_fixture(
        self, package_manager: str, managed_by_variable: str
    ) -> None:
        target_and_tag = self._host_target_and_tag()
        if target_and_tag is None:
            self.skipTest("unsupported test host")
        target, platform_tag = target_and_tag

        with tempfile.TemporaryDirectory() as temp_dir:
            staging_dir = Path(temp_dir) / "staging"
            staging_dir.mkdir()
            build_npm_package.stage_sources(staging_dir, NPM_VERSION, "codex")

            alias_dir = (
                staging_dir
                / "node_modules"
                / "@nexus-agent-x"
                / (f"codex-{platform_tag}")
            )
            alias_dir.mkdir(parents=True)
            (alias_dir / "package.json").write_text(
                json.dumps({"name": NPM_NAME, "version": NPM_VERSION}),
                encoding="utf-8",
            )
            executable = alias_dir / "vendor" / target / "bin" / "codex"
            executable.parent.mkdir(parents=True)
            executable.write_text(
                "#!/bin/sh\n"
                "printf '%s\\n' \"$CODEX_MANAGED_PACKAGE_NAME\"\n"
                "printf '%s\\n' \"$CODEX_MANAGED_PACKAGE_VERSION\"\n"
                f"printf '%s\\n' \"${managed_by_variable}\"\n"
                "printf '%s\\n' \"$*\"\n",
                encoding="utf-8",
            )
            executable.chmod(0o755)

            env = os.environ.copy()
            env["npm_config_user_agent"] = (
                f"{package_manager}/11.0.0 node/v24.0.0 linux x64"
            )
            if package_manager == "pnpm":
                owning_node_modules = Path(temp_dir) / "node_modules"
                package_link = owning_node_modules / "@nexus-agent-x" / "codex"
                package_link.parent.mkdir(parents=True)
                (owning_node_modules / ".modules.yaml").touch()
                package_link.symlink_to(staging_dir, target_is_directory=True)

            result = subprocess.run(
                ["node", str(staging_dir / "bin" / "codex.js"), "--probe"],
                check=True,
                capture_output=True,
                env=env,
                text=True,
            )
            self.assertEqual(
                result.stdout.splitlines(), [NPM_NAME, NPM_VERSION, "1", "--probe"]
            )

    @staticmethod
    def _host_target_and_tag() -> tuple[str, str] | None:
        machine = platform.machine().lower()
        if machine in {"amd64", "x86_64"}:
            arch = "x86_64"
            npm_arch = "x64"
        elif machine in {"aarch64", "arm64"}:
            arch = "aarch64"
            npm_arch = "arm64"
        else:
            return None

        if platform.system() == "Linux":
            return f"{arch}-unknown-linux-musl", f"linux-{npm_arch}"
        if platform.system() == "Darwin":
            return f"{arch}-apple-darwin", f"darwin-{npm_arch}"
        return None


if __name__ == "__main__":
    unittest.main()
