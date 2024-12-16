#!/usr/bin/env bash

cargo build --workspace --all-targets --all-features || exit 1
export RUST_BACKTRACE=1
RGB="./target/debug/rgb -d examples/data"

rm -rf examples/data/bcor/DemoToken.contract

$RGB --seal bcor issue -w alice examples/Demo.yaml
$RGB contracts
