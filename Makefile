.PHONY: build test release package package-chrome package-safari clean

build:
	cd sensor && npm install && npm run build
	cargo build

test:
	cd sensor && npm install && npm run build && npm test
	cargo fmt --check
	cargo test
	cargo clippy --all-targets -- -D warnings
	./scripts/test-gui.sh

release:
	cd sensor && npm install && npm run build && npm test
	cargo build --release --locked
	./scripts/build-gui.sh

package: release
	./scripts/package.sh

package-chrome:
	./scripts/package-chrome-extension.sh

package-safari:
	./scripts/package-safari-extension.sh

clean:
	cargo clean
	rm -rf sensor/dist sensor/node_modules release
