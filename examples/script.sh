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
AUTH_TOKEN=$($RGB_2 invoice -w bob --nonce 0 --seal-only DemoToken)
INVOICE=$($RGB_2 invoice -w bob --nonce 0 DemoToken 10)

rm examples/transfer.psbt examples/Transfer.yaml
$RGB script -w alice "$INVOICE" examples/Transfer.yaml || exit 1
$RGB exec -w alice examples/Transfer.yaml examples/transfer.pfab 1000 examples/transfer.psbt || exit 1

$RGB complete -w alice examples/transfer.pfab examples/transfer.psbt || exit 1

rm examples/transfer.rgb
$RGB consign DemoToken -t "$AUTH_TOKEN" examples/transfer.rgb || exit 1
$RGB state -go -w alice

$RGB_2 accept -w bob examples/transfer.rgb || exit 1

$RGB_2 state -go -w bob
