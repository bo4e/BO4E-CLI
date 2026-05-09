#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION="${BO4E_VERSION:-latest}"
cargo build -p bo4e-cli --release
./target/release/bo4e pull -t "$VERSION" -o .tmp/bo4e_latest
echo "Hydrated .tmp/bo4e_latest at $(cat .tmp/bo4e_latest/.version)"
