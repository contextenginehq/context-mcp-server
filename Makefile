.PHONY: build test clean release check

BINARY := mcp-context-server
DIST   := dist

build:
	cargo build

test:
	cargo test

check:
	cargo check
	cargo clippy -- -D warnings

clean:
	cargo clean
	rm -rf $(DIST)

release:
	cargo build --release
	mkdir -p $(DIST)
	cp target/release/$(BINARY) $(DIST)/
