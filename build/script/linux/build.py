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
    target = args.target
    bin_name = args.bin_name
    asset_name = args.asset_name

    run([
        "cargo",
        "build",
        "--release",
        "--target",
        target,
    ])

    source = Path("target") / target / "release" / bin_name
    output = Path(asset_name)

    if not source.exists():
        raise FileNotFoundError(f"Build output not found: {source}")

    print(f"Copying {source} -> {output}")
    shutil.copy2(source, output)

    write_sha256(output)

    print(f"Generated asset: {output}")


def prepare_source_archive(args: argparse.Namespace) -> None:
    output_dir = Path(args.output_dir)
    archive_name = args.archive_name
    ref = args.ref

    output_dir.mkdir(parents=True, exist_ok=True)

    archive_path = output_dir / archive_name

    run([
        "git",
        "archive",
        "--format=zip",
        f"--output={archive_path}",
        ref,
    ])

    write_sha256(archive_path)

    print(f"Generated source archive: {archive_path}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Linux build/release helper for p2fmd",
    )

    subparsers = parser.add_subparsers(dest="command", required=True)

    binary_parser = subparsers.add_parser("binary")
    binary_parser.add_argument("--target", required=True)
    binary_parser.add_argument("--bin-name", required=True)
    binary_parser.add_argument("--asset-name", required=True)
    binary_parser.set_defaults(func=build_binary)

    source_parser = subparsers.add_parser("source-archive")
    source_parser.add_argument("--output-dir", default="dist")
    source_parser.add_argument("--archive-name", default="p2fmd-source.zip")
    source_parser.add_argument("--ref", default="HEAD")
    source_parser.set_defaults(func=prepare_source_archive)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
