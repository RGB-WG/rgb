#!/usr/bin/env bash

set -x

cargo build --workspace --all-targets --all-features || exit 1
export RUST_BACKTRACE=1
RGB="./target/debug/rgb -d examples/data"

rm -rf examples/data/bcor/DemoToken.contract

$RGB --seal bcor issue -w alice examples/DemoToken.yaml
$RGB contracts
$RGB --seal bcor state -go -w alice
#$RGB --seal bcor fund alice
$RGB --seal bcor seal -w bob 0

rm examples/transfer.psbt
$RGB --seal bcor exec -w alice examples/Transfer.yaml examples/transfer.pfab 1000 examples/transfer.psbt

rm examples/transfer.rgb
$RGB --seal bcor consign y1RBm~7f-hGoESyj-KPU1sNF-C7RFtm1-S4UobVz-Fu1dV5s -t 5WIb5EMY-RCLbO3Wq-hGdddRP4-IeCQzP1y-S5H_UKzd-XJ6ejw examples/transfer.rgb
