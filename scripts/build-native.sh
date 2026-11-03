#!/bin/sh
# Builds the Guardrails tab to WebAssembly and puts it where the manifest's
# `native` says: tabs/guard.wasm. Run from anywhere. Needs the target once:
#
#   rustup target add wasm32-unknown-unknown
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
cp native/target/wasm32-unknown-unknown/release/kit_guard_tab.wasm tabs/guard.wasm
echo "tabs/guard.wasm"
