build:
	cargo build --release
	cd challenge-handler&&cargo build --release

run:build
	./start.sh