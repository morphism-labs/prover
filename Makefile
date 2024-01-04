build:
	cargo build --release
	cp `find ./target/release/ | grep libzktrie.so` /usr/local/lib/
	echo "/usr/local/lib" >> /etc/ld.so.conf && ldconfig -v
	cd challenge-handler&&cargo build --release

clean:
	rm -fr target

run:build
	./start.sh