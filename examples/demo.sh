#!/usr/bin/env bash

cargo build --workspace --all-targets --all-features || exit 1
export RUST_BACKTRACE=1
RGB="./target/debug/rgb -d examples/data"
RGB_2="./target/debug/rgb -d examples/data2"

$RGB init 2>/dev/null
$RGB_2 init 2>/dev/null

set -x

$RGB import examples/RGB20-NFA.issuer

rm -rf examples/data/bitcoin.testnet/DemoToken.contract
rm -rf examples/data2/bitcoin.testnet/DemoToken.contract
$RGB issue -w alice examples/DemoToken.yaml
cp -r examples/data/bitcoin.testnet/DemoToken.contract examples/data2/bitcoin.testnet/

$RGB contracts
$RGB state -go -w alice
#$RGB fund alice
AUTH_TOKEN=$($RGB_2 invoice -w bob --nonce 0 --seal-only DemoToken)
INVOICE=$($RGB_2 invoice -w bob --nonce 0 DemoToken 10)

rm examples/transfer.psbt examples/Transfer.yaml examples/transfer.rgb
$RGB pay -w alice "$INVOICE" examples/transfer.rgb examples/transfer.psbt || exit 1
$RGB state -goa -w alice --sync --mempool

$RGB_2 accept -w bob examples/transfer.rgb || exit 1

$RGB_2 state -go -w bob --sync --mempool
