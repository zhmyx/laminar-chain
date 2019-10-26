run: githooks
	SKIP_WASM_BUILD= cargo run -- --dev --execution native

toolchain:
	./scripts/init.sh

build-wasm: githooks
	WASM_BUILD_TYPE=release cargo build

check: githooks
	SKIP_WASM_BUILD= cargo check

build: githooks
	SKIP_WASM_BUILD= cargo build

purge: target/debug/flowchain
	target/debug/flowchain purge-chain --dev -y

restart: purge run

target/debug/flowchain: build

GITHOOKS = $(wildcard githooks/*)
GITHOOKS_SRC = $(wildcard githooks/*)
GITHOOKS_DEST = $(patsubst githooks/%, .git/hooks/%, $(GITHOOKS_SRC))

.git/hooks:
	mkdir .git/hooks

.git/hooks/%: githooks/%
	cp $^ $@

githooks: .git/hooks $(GITHOOKS_DEST)

init: toolchain build-wasm
