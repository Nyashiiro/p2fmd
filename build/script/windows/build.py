#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///

import argparse
import hashlib
import shutil
import subprocess
from pathlib import Path


def run(cmd: list[str]) -> None:
    print(f"+ {' '.join(cmd)}")
    subprocess.run(cmd, check=True)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()

    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)

    return h.hexdigest()


def write_sha256(path: Path) -> Path:
    checksum_path = path.with_name(path.name + ".sha256")
    digest = sha256_file(path)

    checksum_path.write_text(
        f"{digest}  {path.name}\n",
        encoding="utf-8",
    )

    print(f"Generated checksum: {checksum_path}")
    return checksum_path


def build_binary(args: argparse.Namespace) -> None:
    run([
        "cargo",
        "build",
        "--release",
        "--target",
        args.target,
    ])

    source = Path("target") / args.target / "release" / args.bin_name
    output = Path(args.asset_name)

    if not source.exists():
        raise FileNotFoundError(f"Build output not found: {source}")

    print(f"Copying {source} -> {output}")
    shutil.copy2(source, output)

    write_sha256(output)

    print(f"Generated asset: {output}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Windows build helper",
    )

    subparsers = parser.add_subparsers(
        dest="command",
        required=True,
    )

    binary_parser = subparsers.add_parser("binary")
    binary_parser.add_argument("--target", required=True)
    binary_parser.add_argument("--bin-name", required=True)
    binary_parser.add_argument("--asset-name", required=True)
    binary_parser.set_defaults(func=build_binary)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
