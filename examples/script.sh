#!/usr/bin/env bash

set -x

cargo build --workspace --all-targets --all-features || exit 1
export RUST_BACKTRACE=1
RGB="./target/debug/rgb -d examples/data"
RGB_2="./target/debug/rgb -d examples/data2"

rm -rf examples/data/bcor/DemoToken.contract
rm -rf examples/data2/bcor/DemoToken.contract
$RGB --seal bcor issue -w alice examples/DemoToken.yaml
$RGB_2 --seal bcor issue -w alice examples/DemoToken.yaml

$RGB contracts
$RGB --seal bcor state -go -w alice
#$RGB --seal bcor fund alice
$RGB_2 --seal bcor seal -w bob 0

rm examples/transfer.psbt
$RGB --seal bcor exec -w alice examples/Transfer.yaml examples/transfer.pfab 1000 examples/transfer.psbt

$RGB --seal bcor complete -w alice examples/transfer.pfab examples/transfer.psbt

rm examples/transfer.rgb
$RGB --seal bcor consign gDmGtRAO-gp3AQ78-jqEzM8S-_u8FVot-g2WaGXD-xLdIWXQ -t 5WIb5EMY-RCLbO3Wq-hGdddRP4-IeCQzP1y-S5H_UKzd-XJ6ejw examples/transfer.rgb

$RGB_2 --seal bcor accept -w bob examples/transfer.rgb
