TAG ?= v0.11.0

.PHONY: release check build verify dry-run clean

release:
	python scripts/pipeline.py $(TAG)

draft:
	python scripts/pipeline.py $(TAG) --draft

build:
	python scripts/pipeline.py $(TAG) --no-publish

check:
	cargo check --workspace
	cargo test --workspace

clean:
	rm -rf dist/$(TAG)
	cargo clean
