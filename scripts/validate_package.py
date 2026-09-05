#!/usr/bin/env python3
"""Check the compressed crate against crates.io's default 10 MB upload limit."""

import os
from pathlib import Path
import sys
import tomllib

MAX_PACKAGE_BYTES = 10_000_000


def validate_package(path: Path, limit: int = MAX_PACKAGE_BYTES) -> int:
    size = path.stat().st_size
    if size == 0 or size > limit:
        raise ValueError(f"crate archive must be 1..{limit} bytes, got {size}")
    return size


def main() -> None:
    with Path("Cargo.toml").open("rb") as manifest:
        package = tomllib.load(manifest)["package"]
    archive = Path(os.environ.get("CARGO_TARGET_DIR", "target")) / "package" / (
        f"{package['name']}-{package['version']}.crate"
    )
    size = validate_package(archive)
    print(f"Package size: {size} bytes; maximum {MAX_PACKAGE_BYTES} bytes")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError) as error:
        print(f"::error::{error}", file=sys.stderr)
        sys.exit(1)
