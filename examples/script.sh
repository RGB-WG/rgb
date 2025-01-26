#!/usr/bin/env bash

set -x

cargo build --workspace --all-targets --all-features || exit 1
export RUST_BACKTRACE=1
RGB="./target/debug/rgb -d examples/data"
RGB_2="./target/debug/rgb -d examples/data2"

rm -rf examples/data/bitcoin.testnet/DemoToken.contract
rm -rf examples/data2/bitcoin.testnet/DemoToken.contract
$RGB issue -w alice examples/DemoToken.yaml
cp -r examples/data/bitcoin.testnet/DemoToken.contract examples/data2/bitcoin.testnet/

$RGB contracts
$RGB state -go -w alice
#$RGB fund alice
$RGB_2 seal -w bob 0

rm examples/transfer.psbt
$RGB exec -w alice examples/Transfer.yaml examples/transfer.pfab 1000 examples/transfer.psbt

$RGB complete -w alice examples/transfer.pfab examples/transfer.psbt

rm examples/transfer.rgb
$RGB consign qKpMlzOe-Imn6ysZ-a8JjG2p-WHWvaFm-BWMiPi3-_LvnfRw -t at:5WIb5EMY-RCLbO3Wq-hGdddRP4-IeCQzP1y-S5H_UKzd-ViYmlA examples/transfer.rgb

$RGB_2 accept -w bob examples/transfer.rgb

$RGB_2 state -go -w bob
$RGB state -go -w alice
