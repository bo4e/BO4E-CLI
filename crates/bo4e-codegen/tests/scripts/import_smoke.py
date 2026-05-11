#!/usr/bin/env python3
"""
Importability smoke test for a generated Python package.

Walks every `.py` file under the given root directory and attempts to import it
as a module of the package rooted at that directory. Reports the first import
failure per file and exits non-zero if any failed.

Usage:
    python import_smoke.py <output_dir>

Where <output_dir> is the directory `bo4e generate -t python-pydantic -o ...`
wrote into (i.e. the directory containing `__init__.py`, `bo/`, `com/`, etc.).
"""

from __future__ import annotations

import importlib
import sys
from pathlib import Path


def discover_modules(root: Path) -> list[str]:
    """Return dotted module names for every .py under root, package-rooted at root.name."""
    pkg = root.name
    modules: list[str] = []
    for path in sorted(root.rglob("*.py")):
        rel = path.relative_to(root)
        parts = list(rel.with_suffix("").parts)
        if parts[-1] == "__init__":
            parts = parts[:-1]
        dotted = ".".join([pkg, *parts]) if parts else pkg
        modules.append(dotted)
    return modules


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(f"usage: {argv[0]} <output_dir>", file=sys.stderr)
        return 2

    root = Path(argv[1]).resolve()
    if not root.is_dir():
        print(f"error: {root} is not a directory", file=sys.stderr)
        return 2

    # Make the *parent* of root importable so that `import <root.name>...` works.
    sys.path.insert(0, str(root.parent))

    modules = discover_modules(root)
    failures: list[tuple[str, str]] = []

    for name in modules:
        try:
            importlib.import_module(name)
        except Exception as exc:
            failures.append((name, f"{type(exc).__name__}: {exc}"))

    print(f"Imported {len(modules) - len(failures)}/{len(modules)} modules cleanly.")
    if failures:
        print("\nFailures:", file=sys.stderr)
        for name, msg in failures:
            print(f"  {name}: {msg}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
