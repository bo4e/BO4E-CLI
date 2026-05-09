#!/usr/bin/env bash
# Importability smoke test for the python-pydantic generator.
#
# Pulls BO4E schemas, runs `generate -t python-pydantic`, then walks the output
# and importlib.import_module()s every generated .py. Exits non-zero on any
# ImportError or SyntaxError. Honours BO4E_VERSION (default "latest").
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${BO4E_VERSION:-latest}"
SCHEMAS_DIR=".tmp/smoke-pydantic/schemas"
PY_DIR=".tmp/smoke-pydantic/bo4e"

cargo build -p bo4e-cli --release
./target/release/bo4e pull -t "$VERSION" -o "$SCHEMAS_DIR"
./target/release/bo4e generate -i "$SCHEMAS_DIR" -o "$PY_DIR" -t python-pydantic

python3 crates/bo4e-codegen/tests/scripts/import_smoke.py "$PY_DIR"
