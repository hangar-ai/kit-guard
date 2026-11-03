#!/bin/sh
# Builds the Guardrails tab to WebAssembly and puts it where the manifest's
# `entry` says: tabs/guard.wasm. Run from anywhere. Needs the target once:
#
#   rustup target add wasm32-unknown-unknown
#
#   scripts/build-native.sh          build it
#   scripts/build-native.sh --check  build it, and fail if the committed one is not that
#
# Hangar runs the .wasm a kit ships and never builds one, so the module is
# committed, in the commit that changes its source (CI builds it again and
# fails when the committed one is another). For that the build must not depend
# on where it runs: --locked keeps Cargo.lock as it is, rust-toolchain.toml
# pins the compiler, and the paths of this machine are remapped out of the file.
set -eu
cd "$(dirname "$0")/.."
root="$(pwd)"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
rustup_home="${RUSTUP_HOME:-$HOME/.rustup}"
sep="$(printf '\037')"
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$root=.${sep}--remap-path-prefix=$cargo_home=/cargo${sep}--remap-path-prefix=$rustup_home=/rustup"

cargo build --locked --release --target wasm32-unknown-unknown \
  --manifest-path native/Cargo.toml --target-dir native/target
built="native/target/wasm32-unknown-unknown/release/kit_guard_tab.wasm"

if [ "${1:-}" = "--check" ]; then
  if cmp -s "$built" tabs/guard.wasm; then
    echo "tabs/guard.wasm is what native/ builds"
    exit 0
  fi
  echo "tabs/guard.wasm is stale: run scripts/build-native.sh and commit the result" >&2
  exit 1
fi

cp "$built" tabs/guard.wasm
echo "tabs/guard.wasm"
