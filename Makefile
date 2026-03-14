.PHONY: build up down shell logs restart cargo-fmt cargo-clippy cargo-run cargo-build launch

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

cargo-clippy:
	docker exec rust_ubuntu bash -c "cd /wasabi && cargo clippy"

cargo-run:
	docker exec rust_ubuntu bash -c "cd /wasabi && cargo run"

cargo-build:
	docker exec rust_ubuntu bash -c "cd /wasabi && cargo build --target x86_64-unknown-uefi"

launch:
	bash wasabi/scripts/launch_qeme.sh
