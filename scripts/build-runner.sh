#!/bin/sh
set -eu

triple="$(rustc -vV | sed -n 's/^host: //p')"

if ! rustc +nightly --version >/dev/null 2>&1; then
    echo "build-runner: needs a nightly toolchain (rustup toolchain install nightly)" >&2
    exit 1
fi
if [ ! -d "$(rustc +nightly --print sysroot)/lib/rustlib/src" ]; then
    echo "build-runner: needs rust-src (rustup component add rust-src --toolchain nightly)" >&2
    exit 1
fi

RUSTFLAGS="-Zunstable-options -Cpanic=immediate-abort" \
    cargo +nightly build --release -p runner \
    -Z build-std=std,panic_abort \
    --target "$triple"

mkdir -p target/release
cp "target/$triple/release/gecko-runner" target/release/gecko-runner

size="$(wc -c < target/release/gecko-runner | tr -d ' ')"
echo "release runner: target/release/gecko-runner ($size bytes)"
