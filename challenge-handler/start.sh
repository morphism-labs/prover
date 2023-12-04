# RUST_LOG=trace RUST_BACKTRACE=full nohup ./target/release/prover_server >out.log 2>&1 &
RUST_LOG=debug RUST_BACKTRACE=full nohup ./target/debug/challenge-handler >challenge.log 2>&1 &