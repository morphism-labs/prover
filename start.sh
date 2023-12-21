# RUST_LOG=trace RUST_BACKTRACE=full nohup ./target/release/prover_server >out.log 2>&1 &
pkill -9 prover_server
pkill -9 challenge

RUST_BACKTRACE=full nohup ./target/release/prover_server >prover.log 2>&1 &
cd ./challenge-handler
RUST_LOG=debug RUST_BACKTRACE=full nohup ./target/release/challenge-handler >handler.log 2>&1 &