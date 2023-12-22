build:
	cargo build
	cd challenge-handler&&cargo build

run:build
	./start.sh