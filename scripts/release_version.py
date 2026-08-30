#!/usr/bin/env python3
"""Read [workspace.package].version and compare it with a git tag.

Mirrors AgentHub's release-metadata check, without Node or a CHANGELOG gate.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# SemVer 2.0.0 without a leading `v`. Prerelease and build metadata allowed.
STRICT_SEMVER = re.compile(
    r"^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)"
    r"(?:-(?:(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)


def read_cargo_workspace_version(cargo_toml: str) -> str:
    in_workspace_package = False
    found_workspace_package = False
    versions: list[str] = []

    for line in cargo_toml.splitlines():
        section = re.match(r"^\s*\[([^\]]+)\]\s*(?:#.*)?$", line)
        if section:
            in_workspace_package = section.group(1).strip() == "workspace.package"
            found_workspace_package = found_workspace_package or in_workspace_package
            continue
        if not in_workspace_package:
            continue
        version = re.match(r"""^\s*version\s*=\s*(["'])(.*?)\1\s*(?:#.*)?$""", line)
        if version:
            versions.append(version.group(2))

    if not found_workspace_package:
        raise ValueError("Cargo.toml is missing [workspace.package]")
    if len(versions) != 1:
        raise ValueError(
            f"expected exactly one [workspace.package] version, found {versions!r}"
        )
    return versions[0]


def parse_release_tag(tag: str) -> tuple[str, bool]:
    if not tag.startswith("v"):
        raise ValueError(f"release tag must start with 'v': {tag}")
    version = tag[1:]
    if not STRICT_SEMVER.fullmatch(version):
        raise ValueError(f"tag version is not strict semver: {version}")
    core = version.split("+", 1)[0]
    prerelease = "-" in core
    return version, prerelease


def write_github_output(path: Path, values: dict[str, str]) -> None:
    with path.open("a", encoding="utf-8") as handle:
        for key, value in values.items():
            handle.write(f"{key}={value}\n")


def self_test() -> None:
    cargo = (
        "[workspace]\n"
        "members = [\"crates/xmanager-ui\"]\n\n"
        "[workspace.package]\n"
        'version = "0.1.0"\n'
        'edition = "2021"\n'
        "[workspace.dependencies]\n"
        'gpui = "0.2.2"\n'
    )
    assert read_cargo_workspace_version(cargo) == "0.1.0"
    assert parse_release_tag("v0.1.0") == ("0.1.0", False)
    assert parse_release_tag("v1.2.3-rc.1") == ("1.2.3-rc.1", True)
    try:
        parse_release_tag("0.1.0")
    except ValueError:
        pass
    else:
        raise AssertionError("tag without v must fail")
    try:
        parse_release_tag("v01.0.0")
    except ValueError:
        pass
    else:
        raise AssertionError("leading-zero tag must fail")
    print("release_version self-test ok")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--cargo", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--expect-tag", help="Git tag such as v0.1.0")
    parser.add_argument(
        "--github-output",
        type=Path,
        help="Append version/tag/prerelease to this GitHub Actions output file",
    )
    args = parser.parse_args(argv)

    if args.self_test:
        self_test()
        return 0

    if not args.expect_tag:
        parser.error("--expect-tag is required unless --self-test")

    cargo_version = read_cargo_workspace_version(args.cargo.read_text(encoding="utf-8"))
    tag_version, prerelease = parse_release_tag(args.expect_tag)
    if cargo_version != tag_version:
        raise SystemExit(
            f"Cargo.toml workspace version {cargo_version!r} does not match "
            f"tag {args.expect_tag!r}"
        )
    if not STRICT_SEMVER.fullmatch(cargo_version):
        raise SystemExit(f"Cargo.toml version is not strict semver: {cargo_version}")

    values = {
        "version": cargo_version,
        "tag": args.expect_tag,
        "prerelease": "true" if prerelease else "false",
    }
    if args.github_output:
        write_github_output(args.github_output, values)
    for key, value in values.items():
        print(f"{key}={value}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
