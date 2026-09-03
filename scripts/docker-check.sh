#!/usr/bin/env sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

docker run --rm \
  --env CARGO_HOME=/tmp/cargo \
  --volume "${repo_dir}:/workspace" \
  --workdir /workspace \
  rust:latest \
  sh -c 'rustup component add rustfmt clippy && cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --test discovery_cli -- --nocapture && cargo test --all-targets && cargo run --quiet -- --validate'
