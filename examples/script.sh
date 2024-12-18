#!/usr/bin/env bash

set -x

cargo build --workspace --all-targets --all-features || exit 1
export RUST_BACKTRACE=1
RGB="./target/debug/rgb -d examples/data"

rm -rf examples/data/bcor/DemoToken.contract

$RGB --seal bcor issue -w alice examples/DemoToken.yaml
$RGB contracts
$RGB --seal bcor state -w alice
#$RGB --seal bcor fund alice
$RGB --seal bcor seal -w bob 0
$RGB --seal bcor exec -w bob examples/Transfer.yaml examples/transfer.pfab 1000 examples/transfer.psbt
