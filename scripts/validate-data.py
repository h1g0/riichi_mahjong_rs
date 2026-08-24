#!/usr/bin/env python3
"""Validate the published data files in `data/` against their JSON Schemas.

Each `data/<name>.schema.json` is applied to `data/<name>.json`. The Rust test
suite already pins the data to the code, but nothing in Rust executes the
schema, so a schema could quietly stop describing the file it documents. This
script is what makes the schema a contract rather than a comment.

Requires `jsonschema` (`pip install jsonschema`).
"""

import json
import pathlib
import sys

from jsonschema import Draft202012Validator

DATA_DIR = pathlib.Path(__file__).resolve().parent.parent / "data"


def main() -> int:
    schemas = sorted(DATA_DIR.glob("*.schema.json"))
    if not schemas:
        print(f"no schemas found in {DATA_DIR}", file=sys.stderr)
        return 1

    failed = False
    for schema_path in schemas:
        name = schema_path.name.removesuffix(".schema.json")
        data_path = DATA_DIR / f"{name}.json"
        if not data_path.exists():
            print(f"{schema_path.name}: no matching {data_path.name}", file=sys.stderr)
            failed = True
            continue

        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        data = json.loads(data_path.read_text(encoding="utf-8"))

        # A schema that is itself malformed would otherwise pass everything.
        Draft202012Validator.check_schema(schema)

        errors = sorted(
            Draft202012Validator(schema).iter_errors(data),
            key=lambda error: list(error.absolute_path),
        )
        for error in errors:
            location = "/".join(str(part) for part in error.absolute_path) or "<root>"
            print(f"{data_path.name}: {location}: {error.message}", file=sys.stderr)
        if errors:
            failed = True
        else:
            print(f"{data_path.name} validates against {schema_path.name}")

    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
