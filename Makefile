# Aurelia Local Release & Packaging (adopted from the Morrow pipeline)
# Stages: preflight -> version -> build -> test -> package -> verify -> publish -> ship
TAG ?= v2.5.0

.PHONY: release check version build test package portable scan verify ship republish dry-run clean

# Full release: bump versions, build, test, package (zip + Inno Setup installer), publish
release:
	python scripts/pipeline.py $(TAG)

# Full local verification and packaging without tagging/pushing or uploading
check:
	python scripts/pipeline.py $(TAG) --no-publish

# Bump every version declaration to $(TAG) without building
version:
	python scripts/pipeline.py $(TAG) --from version --skip build --skip test --skip package --skip verify --skip publish --skip ship

# Release build only
build:
	python scripts/pipeline.py $(TAG) --from build --skip test --skip package --skip verify --skip publish --skip ship

# Workspace tests only
test:
	python scripts/pipeline.py $(TAG) --from test --skip package --skip verify --skip publish --skip ship

# Create the portable zip, the installer and release notes in dist/$(TAG)/
package:
	python scripts/pipeline.py $(TAG) --from package --skip verify --skip publish --skip ship

# Package a portable zip only (no Inno Setup required)
portable:
	python scripts/pipeline.py $(TAG) --from package --skip verify --skip publish --skip ship --skip-installer

# Verify checksums, archive integrity and the installer image
verify:
	python scripts/pipeline.py $(TAG) --from verify --skip publish --skip ship

# Scan the packaged artifacts with the VirusTotal API (needs VIRUSTOTAL_API_KEY)
scan:
	python scripts/pipeline.py $(TAG) --from virustotal --skip verify --skip publish --skip ship

# Attach already-verified artifacts to the GitHub release page (REST, no CLI)
ship:
	python scripts/pipeline.py $(TAG) --from ship

# Republish an already-released version (rebuild + record another publication)
republish:
	python scripts/pipeline.py $(TAG) --clobber

# Preview pipeline execution without modifying anything
dry-run:
	python scripts/pipeline.py $(TAG) --dry-run

# Remove build and distribution artifacts
clean:
	python -c "import shutil; shutil.rmtree('target', ignore_errors=True); shutil.rmtree('dist/$(TAG)', ignore_errors=True)"
