#!/usr/bin/env bash

RUST_BACKTRACE=1 && cargo run --release -- test-cases -f params/participants/very_low.toml -r 10 -t 200 2> trace.participants_very_low.log 1> output.participants_very_low.log &
RUST_BACKTRACE=1 && cargo run --release -- test-cases -f params/participants/low.toml -r 10 -t 200 2> trace.participants_low.log 1> output.participants_low.log &
RUST_BACKTRACE=1 && cargo run --release -- test-cases -f params/participants/med.toml -r 10 -t 200 2> trace.participants_med.log 1> output.participants_med.log &
RUST_BACKTRACE=1 && cargo run --release -- test-cases -f params/participants/high.toml -r 10 -t 200 2> trace.participants_high.log 1> output.participants_high.log &
RUST_BACKTRACE=1 && cargo run --release -- test-cases -f params/participants/very_high.toml -r 10 -t 200 2> trace.participants_very_high.log 1> output.participants_very_high.log
