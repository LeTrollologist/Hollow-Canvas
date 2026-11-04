from __future__ import annotations

from aurelia_release.versioning import (
    bump_files,
    check_files,
    declared_versions,
    normalize_text,
)


def test_normalizes_product_declarations_but_not_dependencies(tmp_path):
    text = """[package]
name = "aurelia-io"
version = "3.0.2"

[dependencies]
aurelia-core = { path = "../aurelia-core", version = "3.0.2" }
serde = "1.0"
"""
    updated, count = normalize_text(text, "3.1.0")
    assert count == 2
    assert 'version = "3.1.0"' in updated
    assert 'version = "3.1.0" }' in updated
    assert 'serde = "1.0"' in updated
    assert '3.0.2' not in updated

    again, _ = normalize_text(updated, "3.1.0")
    assert again == updated


def test_declared_versions_collects_docs_and_installer():
    text = """
Release-v3.0.2
Aurelia-v3.0.2-win64.zip
Aurelia-Setup-3.0.2.exe
TAG ?= v3.0.2
v3.0.2 · PRODUCTION
MyAppVersion "3.0.2"
MyAppVersionNumeric "3.0.2.0"
"""
    assert declared_versions(text) == {"3.0.2"}


def test_files_must_all_agree(tmp_path):
    (tmp_path / "a.toml").write_text('[package]\nversion = "3.0.2"\n', encoding="utf-8")
    (tmp_path / "b.md").write_text("Release-v3.0.2\n", encoding="utf-8")
    assert check_files(tmp_path, ["a.toml", "b.md"], "3.0.2") == []

    (tmp_path / "b.md").write_text("Release-v3.0.1\n", encoding="utf-8")
    problems = check_files(tmp_path, ["a.toml", "b.md"], "3.0.2")
    assert any("b.md" in problem for problem in problems)


def test_bump_files_reports_only_real_changes(tmp_path):
    (tmp_path / "a.toml").write_text('[package]\nversion = "3.0.2"\n', encoding="utf-8")
    changed = bump_files(tmp_path, ["a.toml"], "3.1.0")
    assert changed == ["a.toml"]
    assert '3.1.0' in (tmp_path / "a.toml").read_text(encoding="utf-8")
