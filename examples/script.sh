#!/usr/bin/env bash

set -x

cargo build --workspace --all-targets --all-features || exit 1
export RUST_BACKTRACE=1
RGB="./target/debug/rgb -d examples/data"

rm -rf examples/data/bcor/DemoToken.contract

$RGB --seal bcor issue -w alice examples/Demo.yaml
$RGB contracts
$RGB --seal bcor state -w alice
#$RGB --seal bcor fund alice
$RGB --seal bcor invoice alice 0
