#/bin/bash
set -e
dir=$1

mkdir -p $dir/output
cargo run -- -s $dir/spec.openapi.yaml -o $dir/output -c $dir/config.json > $dir/output/generate.log
cargo fmt --manifest-path=$dir/output/Cargo.toml
cargo build --manifest-path=$dir/output/Cargo.toml
