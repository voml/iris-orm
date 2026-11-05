#!/usr/bin/env python3
"""Generate a client from one VOS schema through the installed `iris` wheel.

The CLI only reads the schema file and prints the result: parsing, identity
allocation and emission stay in the Rust facade's `generator` module. It must
run against an installed wheel, not a source checkout, so a broken package
build fails here before it reaches generated clients.

Usage (from the repository root):

    python projects/iris.py/scripts/generate.py \
        projects/iris.py/examples/blog/schemas/blog.iris \
        projects/iris.py/examples/blog/src/generated/iris
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import iris


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Generate an Iris client from VOS schema source.")
    parser.add_argument("schema", type=Path, help="path to the `.iris` schema file")
    parser.add_argument("out_dir", type=Path, help="directory that receives the generated client")
    parser.add_argument("--target", default="python", help="`python` (default), `typescript` or `rust`")
    parser.add_argument("--datasource", default=None, help="adapter name; required for the `rust` target")
    args = parser.parse_args(argv)

    source = args.schema.read_text(encoding="utf-8")
    result = iris.generate(source, args.target, str(args.out_dir), args.datasource)
    if not result["ok"]:
        print(f"error: {result['error']}", file=sys.stderr)
        return 1
    print(
        f"generated {len(result['files'])} files "
        f"(fingerprint={result['schema_fingerprint']}) -> {result['output_path']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
