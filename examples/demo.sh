#!/usr/bin/env bash

set -x

rm -rf examples/data/bitcoin.testnet/*.issuer
rm -rf examples/data/bitcoin.testnet/DemoToken.*.contract
rm -rf examples/data/rgb.contracts/alice.stock/DemoToken.*
rm -rf examples/data/rgb.contracts/alice.stock/DemoToken.*.contract
rm -rf examples/data/rgb.contracts/bob.stock/DemoToken.*.contract
rm examples/data/bitcoin.testnet/alice.wallet/transfer.psbt
rm examples/data/rgb.contracts/alice.stock/transfer.rgb

cargo build --workspace --all-targets  || exit 1
export RUST_BACKTRACE=1
ALICE="./target/debug/rgb -d examples/data"
BOB="./target/debug/rgb -d examples/data"

$ALICE init 2>/dev/null
$BOB init 2>/dev/null

$ALICE import examples/data/rgb.issuers/RGB20-FNA.issuer

$ALICE issue -w alice examples/data/rgb.issuers/IssueCmd.yaml
$ALICE backup -f DemoToken examples/data/rgb.contracts/alice.stock/DemoToken.rgb

$BOB accept -w bob -u examples/data/rgb.contracts/alice.stock/DemoToken.rgb

$ALICE contracts
$ALICE state -go -w alice
$BOB state -go -w bob

AUTH_TOKEN=$($BOB invoice -w bob --nonce 0 --seal-only DemoToken)
INVOICE=$($BOB invoice -w bob --nonce 0 DemoToken 10)

$BOB invoice -w bob --nonce 0 DemoToken 10

$ALICE pay --esplora -w alice "$INVOICE" examples/data/rgb.contracts/alice.stock/transfer.rgb examples/data/bitcoin.testnet/alice.wallet/transfer.psbt || exit 1
$ALICE state -goa -w alice

$BOB accept -w bob examples/data/rgb.contracts/alice.stock/transfer.rgb || exit 1
$BOB state -go -w bob
