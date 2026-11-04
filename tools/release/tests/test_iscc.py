from __future__ import annotations

from pathlib import Path

from aurelia_release.util import find_iscc


def test_finds_the_nonstandard_localappdata_programs_inno_layout(tmp_path):
    target = tmp_path / "Local" / "Programs" / "Inno" / "ISCC.exe"
    target.parent.mkdir(parents=True)
    target.write_bytes(b"exe")

    found = find_iscc(
        env={"LOCALAPPDATA": str(tmp_path / "Local")},
        which=lambda name: None,
    )
    assert found == target


def test_environment_override_wins(tmp_path):
    target = tmp_path / "custom" / "ISCC.exe"
    target.parent.mkdir()
    target.write_bytes(b"exe")

    found = find_iscc(env={"ISCC": str(target)}, which=lambda name: None)
    assert found == target


def test_path_lookup_is_used(tmp_path):
    target = tmp_path / "ISCC.exe"
    target.write_bytes(b"exe")

    found = find_iscc(env={}, which=lambda name: str(target) if name == "ISCC.exe" else None)
    assert found == target


def test_program_files_is_searched(tmp_path):
    target = tmp_path / "PF" / "Inno Setup 6" / "ISCC.exe"
    target.parent.mkdir(parents=True)
    target.write_bytes(b"exe")

    found = find_iscc(env={"ProgramFiles": str(tmp_path / "PF")}, which=lambda name: None)
    assert found == target


def test_returns_none_when_missing(tmp_path):
    assert find_iscc(env={"LOCALAPPDATA": str(tmp_path)}, which=lambda name: None) is None
