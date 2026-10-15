#!/usr/bin/env bash
# The whole local gate, in one command — exactly what CI blocks on.
set -euo pipefail
cd "$(dirname "$0")/.."

step() {
  printf '\n\033[1m==> %s\033[0m\n' "$*"
  "$@"
}

step cargo fmt --check
step cargo clippy --locked --all-targets --all-features -- -D warnings
step cargo test --locked
step cargo build --locked --features python
step uv run --python 3.13 --with maturin --with pytest --with matplotlib sh -c \
  'maturin develop --release && pytest tests_python -q'
step uv run --with maturin sh -c 'maturin build --release --locked -o dist'
step uv run --python 3.13 python scripts/check_consumer.py --wheel-dir dist

printf '\n\033[1;32mAll gates passed.\033[0m\n'
