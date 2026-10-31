TAG ?= v1.0.4-alpha


.PHONY: release check build verify dry-run clean

release:
	python scripts/pipeline.py $(TAG)

draft:
	python scripts/pipeline.py $(TAG) --draft

build:
	python scripts/pipeline.py $(TAG) --no-publish

check:
	cargo check --workspace --all-targets
	cargo test --workspace

# Fully local CI equivalent: build + test + audit + package (no upload)
verify:
	python scripts/pipeline.py $(TAG) --no-publish

clean:
	rm -rf dist/$(TAG)
	cargo clean
