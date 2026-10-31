#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --manifest-path login-helper/Cargo.toml --locked
cargo build --locked
cp login-helper/target/debug/bnet-login target/debug/.bnet-login.new
mv -f target/debug/.bnet-login.new target/debug/bnet-login
exec target/debug/battlenet-linux-launcher "$@"
