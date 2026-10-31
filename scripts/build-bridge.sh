#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
host=$(rustc -vV | sed -n 's/^host: //p')
linker="$(rustc --print sysroot)/lib/rustlib/$host/bin/rust-lld"
cargo rustc --manifest-path bridge/Cargo.toml --release --target x86_64-pc-windows-msvc -- \
  -C linker="$linker" -C link-arg=/entry:mainCRTStartup -C link-arg=/subsystem:console -C link-arg=/nodefaultlib
mkdir -p assets
cp bridge/target/x86_64-pc-windows-msvc/release/bnet-session-bridge.exe assets/bnet-session-bridge.exe
if [ "${1:-}" = "--tests" ]; then
    cargo rustc --manifest-path bridge/Cargo.toml --example launch_fixture --release --target x86_64-pc-windows-msvc -- \
      -C linker="$linker" -C link-arg=/entry:mainCRTStartup -C link-arg=/subsystem:console -C link-arg=/nodefaultlib
fi
