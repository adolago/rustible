#!/usr/bin/env python3
"""Validate the release contract before any publishing job can start."""

import os
from pathlib import Path
import re
import sys
import tomllib


SEMVER = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-((?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*))?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
)


def validate_version(version: str, package_version: str) -> bool:
    """Return prerelease status, rejecting malformed or mismatched versions."""
    match = SEMVER.fullmatch(version)
    if match is None:
        raise ValueError("release version must be a valid semantic version")
    if version != package_version:
        raise ValueError("release version must exactly match package.version in Cargo.toml")
    return match.group(4) is not None


def main() -> None:
    if os.environ.get("GITHUB_EVENT_NAME") == "workflow_dispatch":
        version = os.environ.get("RELEASE_VERSION_INPUT", "")
    else:
        tag = os.environ.get("GITHUB_REF", "")
        if not tag.startswith("refs/tags/v"):
            raise ValueError("release must use a v-prefixed tag or workflow_dispatch")
        version = tag.removeprefix("refs/tags/v")
    with Path("Cargo.toml").open("rb") as manifest:
        package_version = tomllib.load(manifest)["package"]["version"]
    prerelease = validate_version(version, package_version)
    # Output is written only after validating the complete version, including newlines.
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"version={version}\nis_prerelease={str(prerelease).lower()}\n")
    print(f"Validated release {version}; prerelease={prerelease}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, tomllib.TOMLDecodeError) as error:
        print(f"::error::{error}", file=sys.stderr)
        sys.exit(1)
