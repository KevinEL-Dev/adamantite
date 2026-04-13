#!/bin/bash

cargo build --release

version="$(toml get Cargo.toml package.version -r)"

mkdir -p adamantite-v${version}-unknown-linux-gnu

cp target/release/adamantite adamantite-v${version}-unknown-linux-gnu/
cp README.md adamantite-v${version}-unknown-linux-gnu/
cp LICENSE adamantite-v${version}-unknown-linux-gnu/

tar czf adamantite-v${version}-unknown-linux-gnu.tar.gz adamantite-v${version}-unknown-linux-gnu/

sha256sum adamantite-v${version}-unknown-linux-gnu.tar.gz > SHA256SUMS 
