#!/bin/bash -e
#
# script to run project tests and report code coverage
# uses llvm-cov (https://github.com/taiki-e/cargo-llvm-cov)

LLVM_COV_OPTS=()
CARGO_TEST_OPTS=("--")
COV="cargo llvm-cov"

_die() {
    echo "err $*"
    exit 1
}

_tit() {
    echo
    echo "========================================"
    echo "$@"
    echo "========================================"
}

help() {
    echo "$NAME [-h|--help] [-t|--test] [--ci] [--no-clean]"
    echo ""
    echo "options:"
    echo "    -h --help     show this help message"
    echo "    -t --test     only run these test(s)"
    echo "       --ci       run for the CI"
    echo "       --no-clean don't cleanup before the run"
}

# cmdline arguments
while [ -n "$1" ]; do
    case $1 in
        -h|--help)
            help
            exit 0
            ;;
        -t|--test)
            CARGO_TEST_OPTS+=("$2")
            shift
            ;;
        --ci)
            COV_CI="$COV --lcov --output-path coverage.lcov"
            INDEXER=esplora $COV_CI
            INDEXER=electrum $COV_CI --no-clean
            exit 0
            ;;
        *)
            help
            _die "unsupported argument \"$1\""
            ;;
    esac
    shift
done

_tit "installing requirements"
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov

_tit "generating coverage report"
# shellcheck disable=2086
INDEXER=esplora $COV --html "${LLVM_COV_OPTS[@]}" "${CARGO_TEST_OPTS[@]}"
INDEXER=electrum $COV --no-clean --html "${LLVM_COV_OPTS[@]}" "${CARGO_TEST_OPTS[@]}"

## show html report location
echo "generated html report: target/llvm-cov/html/index.html"
