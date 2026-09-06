#!/usr/bin/env python3
"""Copy the JavaScript bundle shipped with the Cargo-resolved macroquad."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys


REPOSITORY_ROOT = Path(__file__).resolve().parent.parent


def load_cargo_metadata() -> dict:
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked"],
        cwd=REPOSITORY_ROOT,
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "cargo metadata failed")
    return json.loads(result.stdout)


def find_macroquad_package(metadata: dict) -> dict:
    packages = {package["id"]: package for package in metadata["packages"]}
    workspace_members = set(metadata["workspace_members"])
    clients = [
        package
        for package in packages.values()
        if package["id"] in workspace_members and package["name"] == "mahjong-client"
    ]
    if len(clients) != 1:
        raise RuntimeError("expected exactly one mahjong-client workspace package")

    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    client_node = nodes[clients[0]["id"]]
    dependencies = [
        packages[dependency["pkg"]]
        for dependency in client_node["deps"]
        if packages[dependency["pkg"]]["name"] == "macroquad"
    ]
    if len(dependencies) != 1:
        raise RuntimeError("expected mahjong-client to have exactly one macroquad dependency")
    return dependencies[0]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "output",
        nargs="?",
        default="public/mq_js_bundle.js",
        help="destination path relative to the repository root",
    )
    args = parser.parse_args()

    try:
        metadata = load_cargo_metadata()
        macroquad = find_macroquad_package(metadata)
        source = Path(macroquad["manifest_path"]).parent / "js" / "mq_js_bundle.js"
        if not source.is_file():
            raise RuntimeError(f"macroquad bundle was not found: {source}")

        output = Path(args.output)
        if not output.is_absolute():
            output = REPOSITORY_ROOT / output
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, output)
    except (KeyError, OSError, TypeError, ValueError, RuntimeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    try:
        displayed_output = output.relative_to(REPOSITORY_ROOT)
    except ValueError:
        displayed_output = output
    print(
        f"Copied macroquad {macroquad['version']} mq_js_bundle.js "
        f"to {displayed_output.as_posix()}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
