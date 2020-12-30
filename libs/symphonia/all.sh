#!/usr/bin/env bash

RUST_BACKTRACE=1 && cargo run --release -- test-cases -f params/all.toml -r 10 -t 10000 2> trace.all.log 1> output.all.log


