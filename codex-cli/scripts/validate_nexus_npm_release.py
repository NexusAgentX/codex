#!/usr/bin/env python3
"""Validate the complete set of Nexus Codex npm release tarballs."""

import argparse
import json
import sys
import tarfile
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

import build_npm_package  # noqa: E402


EXPECTED_REPOSITORY_URL = "git+https://github.com/NexusAgentX/codex.git"
PACKAGE_PREFIX = "package/"


class ReleaseValidationError(RuntimeError):
    """Raised when an npm tarball is not safe to publish."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate Nexus Codex npm release tarballs."
    )
    parser.add_argument("--version", required=True, help="Nexus release version.")
    parser.add_argument(
        "--tarball-dir",
        required=True,
        type=Path,
        help="Directory containing npm tarballs produced by build_npm_package.py.",
    )
    parser.add_argument(
        "--platform",
        choices=tuple(build_npm_package.CODEX_PLATFORM_PACKAGES),
        help="Validate one platform tarball instead of the complete release set.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        validate_release_set(args.tarball_dir, args.version, args.platform)
    except ReleaseValidationError as exc:
        print(f"Nexus npm release validation failed: {exc}", file=sys.stderr)
        return 1

    scope = args.platform or "complete release set"
    print(f"Validated Nexus npm {scope} for {args.version}.")
    return 0


def validate_release_set(
    tarball_dir: Path, version: str, platform: str | None = None
) -> None:
    tarball_dir = tarball_dir.resolve()
    if not tarball_dir.is_dir():
        raise ReleaseValidationError(f"tarball directory not found: {tarball_dir}")

    if platform is not None:
        validate_platform_tarball(
            platform_tarball_path(tarball_dir, version, platform), version, platform
        )
        return

    for platform_name in build_npm_package.CODEX_PLATFORM_PACKAGES:
        validate_platform_tarball(
            platform_tarball_path(tarball_dir, version, platform_name),
            version,
            platform_name,
        )
    validate_root_tarball(tarball_dir / f"codex-npm-{version}.tgz", version)

    expected_tarballs = {
        platform_tarball_path(tarball_dir, version, platform_name).name
        for platform_name in build_npm_package.CODEX_PLATFORM_PACKAGES
    }
    expected_tarballs.add(f"codex-npm-{version}.tgz")
    actual_tarballs = {path.name for path in tarball_dir.glob("*.tgz")}
    if actual_tarballs != expected_tarballs:
        missing = sorted(expected_tarballs - actual_tarballs)
        unexpected = sorted(actual_tarballs - expected_tarballs)
        details = []
        if missing:
            details.append(f"missing: {', '.join(missing)}")
        if unexpected:
            details.append(f"unexpected: {', '.join(unexpected)}")
        raise ReleaseValidationError(
            "release tarball set does not contain exactly seven files ("
            + "; ".join(details)
            + ")"
        )


def platform_tarball_path(tarball_dir: Path, version: str, platform_name: str) -> Path:
    config = build_npm_package.CODEX_PLATFORM_PACKAGES[platform_name]
    return tarball_dir / f"codex-npm-{config['npm_tag']}-{version}.tgz"


def validate_root_tarball(tarball_path: Path, version: str) -> None:
    with open_tarball(tarball_path) as archive:
        package_json = read_json_member(archive, "package/package.json")
        require_equal(package_json, "name", build_npm_package.CODEX_NPM_NAME)
        require_equal(package_json, "version", version)
        require_equal(package_json, "bin", {"codex": "bin/codex.js"})
        require_equal(package_json, "files", ["bin/codex.js"])
        require_nexus_metadata(package_json)

        expected_dependencies = {
            build_npm_package.platform_package_alias(
                build_npm_package.CODEX_NPM_NAME, config["npm_tag"]
            ): (
                f"npm:{build_npm_package.CODEX_NPM_NAME}@"
                f"{build_npm_package.compute_platform_package_version(version, config['npm_tag'])}"
            )
            for config in build_npm_package.CODEX_PLATFORM_PACKAGES.values()
        }
        require_equal(package_json, "optionalDependencies", expected_dependencies)
        require_regular_files(
            archive,
            {
                "package/package.json",
                "package/bin/codex.js",
                "package/README.md",
            },
        )
        require_executable_files(archive, {"package/bin/codex.js"})


def validate_platform_tarball(
    tarball_path: Path, version: str, platform_name: str
) -> None:
    config = build_npm_package.CODEX_PLATFORM_PACKAGES[platform_name]
    npm_tag = config["npm_tag"]
    target = config["target_triple"]
    platform_version = build_npm_package.compute_platform_package_version(
        version, npm_tag
    )
    exe_suffix = ".exe" if config["os"] == "win32" else ""
    vendor_prefix = f"package/vendor/{target}/"

    with open_tarball(tarball_path) as archive:
        package_json = read_json_member(archive, "package/package.json")
        require_equal(package_json, "name", build_npm_package.CODEX_NPM_NAME)
        require_equal(package_json, "version", platform_version)
        require_equal(package_json, "os", [config["os"]])
        require_equal(package_json, "cpu", [config["cpu"]])
        require_equal(package_json, "files", ["vendor"])
        require_nexus_metadata(package_json)

        package_metadata = read_json_member(
            archive, f"{vendor_prefix}codex-package.json"
        )
        expected_metadata = {
            "layoutVersion": 1,
            "version": version,
            "target": target,
            "variant": "codex",
            "entrypoint": f"bin/codex{exe_suffix}",
            "resourcesDir": "codex-resources",
            "pathDir": "codex-path",
        }
        if package_metadata != expected_metadata:
            raise ReleaseValidationError(
                f"{tarball_path.name}: invalid codex-package.json: "
                f"expected {expected_metadata!r}, got {package_metadata!r}"
            )

        required_files = {
            "package/package.json",
            "package/README.md",
            f"{vendor_prefix}codex-package.json",
            f"{vendor_prefix}bin/codex{exe_suffix}",
            f"{vendor_prefix}bin/codex-code-mode-host{exe_suffix}",
            f"{vendor_prefix}codex-path/rg{exe_suffix}",
        }
        executable_files: set[str] = set()
        if config["os"] != "win32":
            zsh_path = f"{vendor_prefix}codex-resources/zsh/bin/zsh"
            required_files.add(zsh_path)
            executable_files.update(
                {
                    f"{vendor_prefix}bin/codex",
                    f"{vendor_prefix}bin/codex-code-mode-host",
                    f"{vendor_prefix}codex-path/rg",
                    zsh_path,
                }
            )
        if config["os"] == "linux":
            bwrap_path = f"{vendor_prefix}codex-resources/bwrap"
            required_files.add(bwrap_path)
            executable_files.add(bwrap_path)
        elif config["os"] == "win32":
            required_files.update(
                {
                    f"{vendor_prefix}codex-resources/codex-command-runner.exe",
                    f"{vendor_prefix}codex-resources/codex-windows-sandbox-setup.exe",
                }
            )
        require_regular_files(archive, required_files)
        require_executable_files(archive, executable_files)

        unexpected_targets = sorted(
            {
                name.removeprefix("package/vendor/").split("/", 1)[0]
                for name in archive.getnames()
                if name.startswith("package/vendor/") and name != "package/vendor/"
            }
            - {target}
        )
        if unexpected_targets:
            raise ReleaseValidationError(
                f"{tarball_path.name}: contains unexpected vendor targets: "
                + ", ".join(unexpected_targets)
            )


def open_tarball(tarball_path: Path) -> tarfile.TarFile:
    if not tarball_path.is_file():
        raise ReleaseValidationError(f"tarball not found: {tarball_path}")
    try:
        archive = tarfile.open(tarball_path, "r:gz")
    except (OSError, tarfile.TarError) as exc:
        raise ReleaseValidationError(
            f"could not read tarball {tarball_path.name}: {exc}"
        ) from exc

    names = archive.getnames()
    if len(names) != len(set(names)):
        archive.close()
        raise ReleaseValidationError(
            f"{tarball_path.name}: contains duplicate archive paths"
        )
    unsafe_names = sorted(
        name
        for name in names
        if (name != "package" and not name.startswith(PACKAGE_PREFIX))
        or name.startswith("/")
        or ".." in Path(name).parts
    )
    if unsafe_names:
        archive.close()
        raise ReleaseValidationError(
            f"{tarball_path.name}: contains unsafe archive paths: "
            + ", ".join(unsafe_names)
        )
    return archive


def read_json_member(archive: tarfile.TarFile, member_name: str) -> dict[str, Any]:
    try:
        member = archive.getmember(member_name)
    except KeyError as exc:
        raise ReleaseValidationError(
            f"{Path(archive.name).name}: missing {member_name}"
        ) from exc
    if not member.isfile():
        raise ReleaseValidationError(
            f"{Path(archive.name).name}: {member_name} is not a regular file"
        )

    member_file = archive.extractfile(member)
    if member_file is None:
        raise ReleaseValidationError(
            f"{Path(archive.name).name}: could not read {member_name}"
        )
    try:
        value = json.load(member_file)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReleaseValidationError(
            f"{Path(archive.name).name}: invalid JSON in {member_name}"
        ) from exc
    if not isinstance(value, dict):
        raise ReleaseValidationError(
            f"{Path(archive.name).name}: {member_name} must contain a JSON object"
        )
    return value


def require_nexus_metadata(package_json: dict[str, Any]) -> None:
    require_equal(package_json, "publishConfig", {"access": "public"})
    repository = package_json.get("repository")
    if not isinstance(repository, dict):
        raise ReleaseValidationError(
            "package.json field 'repository' must be an object"
        )
    if repository.get("url") != EXPECTED_REPOSITORY_URL:
        raise ReleaseValidationError(
            "package.json repository.url must point to the NexusAgentX fork"
        )


def require_equal(package_json: dict[str, Any], field: str, expected: Any) -> None:
    actual = package_json.get(field)
    if actual != expected:
        raise ReleaseValidationError(
            f"package.json field {field!r}: expected {expected!r}, got {actual!r}"
        )


def require_regular_files(archive: tarfile.TarFile, required_files: set[str]) -> None:
    for member_name in sorted(required_files):
        try:
            member = archive.getmember(member_name)
        except KeyError as exc:
            raise ReleaseValidationError(
                f"{Path(archive.name).name}: missing {member_name}"
            ) from exc
        if not member.isfile():
            raise ReleaseValidationError(
                f"{Path(archive.name).name}: {member_name} is not a regular file"
            )


def require_executable_files(
    archive: tarfile.TarFile, executable_files: set[str]
) -> None:
    for member_name in sorted(executable_files):
        member = archive.getmember(member_name)
        if member.mode & 0o111 == 0:
            raise ReleaseValidationError(
                f"{Path(archive.name).name}: {member_name} is not executable"
            )


if __name__ == "__main__":
    raise SystemExit(main())
