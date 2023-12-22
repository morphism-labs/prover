#!/bin/bash

pkill -9 prover_server
pkill -9 challenge

# Start prover
RUST_BACKTRACE=full nohup ./target/debug/prover_server >prover.log 2>&1 &

# Start handler
RUST_LOG=debug RUST_BACKTRACE=full nohup ./challenge-handler/target/debug/challenge-handler >handler.log 2>&1 &
# Start challenger
RUST_LOG=debug RUST_BACKTRACE=full nohup ./challenge-handler/target/debug/auto_challenge >challenge.log 2>&1 &