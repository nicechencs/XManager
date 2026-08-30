#!/usr/bin/env python3
"""Generate the tauri-plugin-updater v2 manifest (latest.json).

Reads per-platform artifact URLs plus their detached signature files
(`*.sig` produced by `tauri build` when bundle.createUpdaterArtifacts is
enabled) and emits the JSON manifest the in-app updater polls:

    https://github.com/<owner>/<repo>/releases/latest/download/latest.json

Usage (publish job):
    python3 scripts/generate_update_manifest.py \
        --version 0.1.0 --repo owner/repo --notes "XManager v0.1.0" \
        --out latest.json \
        --artifact "windows-x86_64|https://.../XManager_0.1.0_x64-setup.exe|path/to/setup.exe.sig" \
        --artifact "darwin-aarch64|https://.../XManager.app.tar.gz|path/to/XManager.app.tar.gz.sig" \
        --artifact "darwin-x86_64|https://.../XManager.app.tar.gz|path/to/XManager.app.tar.gz.sig"

The `--artifact` value is `platform|url|signature_file` (pipe-separated;
URLs and Windows-free POSIX paths never contain pipes).
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path


def build_manifest(
    version: str,
    repo: str,
    notes: str,
    artifacts: list[tuple[str, str, str]],
    now: datetime,
) -> dict:
    if not version:
        raise ValueError("version must not be empty")
    if "/" not in repo:
        raise ValueError(f"repo must look like owner/name, got {repo!r}")
    platforms: dict[str, dict[str, str]] = {}
    for platform, url, sig_path in artifacts:
        if platform in platforms:
            raise ValueError(f"duplicate platform entry: {platform}")
        signature = Path(sig_path).read_text(encoding="utf-8").strip()
        if not signature:
            raise ValueError(f"signature file is empty: {sig_path}")
        if not url.startswith("https://"):
            raise ValueError(f"url must be https: {url}")
        platforms[platform] = {"signature": signature, "url": url}
    if not platforms:
        raise ValueError("no --artifact entries given")
    return {
        "version": version,
        "notes": notes,
        "pub_date": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }


def parse_artifact(value: str) -> tuple[str, str, str]:
    parts = value.split("|")
    if len(parts) != 3 or not all(parts):
        raise ValueError(
            f"--artifact must be platform|url|signature_file, got {value!r}"
        )
    return parts[0], parts[1], parts[2]


def self_test() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        sig = Path(tmp) / "fake.sig"
        sig.write_text("ZmFrZS1zaWduYXR1cmU=\n", encoding="utf-8")
        manifest = build_manifest(
            version="0.1.0",
            repo="owner/repo",
            notes="XManager v0.1.0",
            artifacts=[
                ("windows-x86_64", "https://example.com/a-setup.exe", str(sig)),
                ("darwin-aarch64", "https://example.com/app.tar.gz", str(sig)),
            ],
            now=datetime(2026, 8, 30, 12, 0, 0, tzinfo=timezone.utc),
        )
        assert manifest["version"] == "0.1.0"
        assert manifest["pub_date"] == "2026-08-30T12:00:00Z"
        assert manifest["platforms"]["windows-x86_64"]["url"].startswith("https://")
        assert manifest["platforms"]["windows-x86_64"]["signature"] == (
            "ZmFrZS1zaWduYXR1cmU="
        )
        assert set(manifest["platforms"]) == {"windows-x86_64", "darwin-aarch64"}
        json.dumps(manifest)

        # Error paths.
        try:
            build_manifest("", "owner/repo", "n", [], datetime.now(timezone.utc))
        except ValueError:
            pass
        else:
            raise AssertionError("empty version should fail")
        try:
            build_manifest(
                "0.1.0",
                "justname",
                "n",
                [("p", "https://x/y", str(sig))],
                datetime.now(timezone.utc),
            )
        except ValueError:
            pass
        else:
            raise AssertionError("bad repo should fail")
        try:
            parse_artifact("p|https://x")
        except ValueError:
            pass
        else:
            raise AssertionError("malformed artifact should fail")
    print("generate_update_manifest self-test ok")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", default="")
    parser.add_argument("--repo", default="")
    parser.add_argument("--notes", default="")
    parser.add_argument("--out", default="latest.json")
    parser.add_argument("--artifact", action="append", default=[])
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return 0

    artifacts = [parse_artifact(value) for value in args.artifact]
    manifest = build_manifest(
        version=args.version,
        repo=args.repo,
        notes=args.notes,
        artifacts=artifacts,
        now=datetime.now(timezone.utc),
    )
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(f"wrote {out} with platforms: {', '.join(manifest['platforms'])}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
