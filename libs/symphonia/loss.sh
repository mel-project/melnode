#!/usr/bin/env bash

RUST_BACKTRACE=1 && cargo run --release -- test-cases -f params/loss/very_low.toml -r 10 -t 2000 2> trace.loss_very_low.log 1> output.loss_very_low.log
RUST_BACKTRACE=1 && cargo run --release -- test-cases -f params/loss/low.toml -r 10 -t 2000 2> trace.loss_low.log 1> output.loss_low.log
RUST_BACKTRACE=1 && cargo run --release -- test-cases -f params/loss/med.toml -r 10 -t 2000 2> trace.loss_med.log 1> output.loss_med.log
RUST_BACKTRACE=1 && cargo run --release -- test-cases -f params/loss/high.toml -r 10 -t 2000 2> trace.loss_high.log 1> output.loss_high.log
RUST_BACKTRACE=1 && cargo run --release -- test-cases -f params/loss/very_high.toml -r 10 -t 2000 2> trace.loss_very_high.log 1> output.loss_very_high.log
