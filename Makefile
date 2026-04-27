.PHONY: build up down shell logs restart cargo-fmt cargo-fmt-check cargo-clippy cargo-test cargo-run cargo-build launch setup-hooks

build:
	docker-compose build

up:
	docker-compose up -d

down:
	docker-compose down

shell:
	docker exec -it rust_ubuntu bash

logs:
	docker-compose logs -f

restart: down up

cargo-fmt:
	docker exec rust_ubuntu bash -c "cd /wasabi && cargo fmt"

cargo-fmt-check:
	docker exec rust_ubuntu bash -c "cd /wasabi && cargo fmt -- --check"

cargo-clippy:
	docker exec rust_ubuntu bash -c "cd /wasabi && cargo clippy -- -D warnings"

cargo-test:
	docker exec rust_ubuntu bash -c "cd /wasabi && cargo test"

cargo-run:
	docker exec rust_ubuntu bash -c "cd /wasabi && cargo run"

cargo-build:
	docker exec rust_ubuntu bash -c "cd /wasabi && cargo build --target x86_64-unknown-uefi"

launch:
	bash wasabi/scripts/launch_qeme.sh

setup-hooks:
	git config core.hooksPath .githooks
