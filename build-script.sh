#!/bin/bash

cargo build --release

version="$(toml get Cargo.toml package.version -r)"

mkdir -p dist/adamantite-v${version}-unknown-linux-gnu

cp target/release/adamantite dist/adamantite-v${version}-unknown-linux-gnu/
cp README.md dist/adamantite-v${version}-unknown-linux-gnu/
cp LICENSE dist/adamantite-v${version}-unknown-linux-gnu/

tar czf dist/adamantite-v${version}-unknown-linux-gnu.tar.gz dist/adamantite-v${version}-unknown-linux-gnu/

sha256sum dist/adamantite-v${version}-unknown-linux-gnu.tar.gz > dist/SHA256SUMS 
