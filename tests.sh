#/bin/bash
set -e

export RUSTC_WRAPPER=sccache


for dir in tests/projects/*; 
do
    
    echo "### Start test ${dir}"
    rm -rf $dir/output
    mkdir -p $dir/output
    cargo run -- -s $dir/spec.openapi.yaml -o $dir/output -c $dir/config.json > $dir/output/generate.log
    cargo build --manifest-path=$dir/output/Cargo.toml
    echo "### End test ${dir}"
done

# cargo run -- -s tests/resources/inline_object_same_name.openapi.yaml -o tests/output/inline_object_same_name -c tests/resources/inline_object_same_name.json
# cd tests/output/inline_object_same_name
# cargo build