.PHONY: build test release package clean

build:
	cd sensor && npm install && npm run build
	cargo build

test:
	cd sensor && npm install && npm test && npm run build
	cargo fmt --check
	cargo test
	cargo clippy --all-targets -- -D warnings

release:
	cd sensor && npm install && npm test && npm run build
	cargo build --release --locked
	./scripts/build-gui.sh

package: release
	./scripts/package.sh

clean:
	cargo clean
	rm -rf sensor/dist sensor/node_modules release
