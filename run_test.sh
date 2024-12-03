#/bin/bash

dir=$1

mkdir -p $dir/output
cargo run -- -s $dir/spec.openapi.yaml -o $dir/output -c $dir/config.json > $dir/output/generate.log
cargo build --manifest-path=$dir/output/Cargo.toml