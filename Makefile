.PHONY: build install uninstall

BIN_DIR ?= $(HOME)/.local/bin

build:
	cargo build --release

install: build
	mkdir -p $(BIN_DIR)
	install -m 0755 target/release/dots $(BIN_DIR)/dots

uninstall:
	rm -f $(BIN_DIR)/dots
