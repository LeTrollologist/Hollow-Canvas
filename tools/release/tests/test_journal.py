from __future__ import annotations

import json

import pytest

from aurelia_release.errors import JournalError
from aurelia_release.journal import Journal


def _release(journal: Journal) -> None:
    journal.append(
        "release",
        release="v1.0.0",
        version="1.0.0",
        profile="git",
        base_commit="a" * 40,
        config_digest="cfg",
    )


def test_roundtrip_and_state(tmp_path):
    journal = Journal(tmp_path / "v1.0.0.jsonl")
    _release(journal)
    journal.append("intent", op="tag.create", stage="tag", fp="fp1", before={"existed": False})
    journal.append("result", op="tag.create", stage="tag", status="ok", outputs={"files": {}})
    journal.append("verify", op="tag.create", stage="tag", status="passed", details={"tag": "v1.0.0"})

    state = journal.state()["tag.create"]
    assert state.status == "ok"
    assert state.verified is True
    assert state.before == {"existed": False}
    assert journal.base_commit() == "a" * 40
    assert journal.status() == "running"


def test_every_record_is_checksummed(tmp_path):
    journal = Journal(tmp_path / "j.jsonl")
    _release(journal)
    raw = journal.path.read_text(encoding="utf-8")
    assert '"checksum"' in raw
    tampered = raw.replace('"profile":"git"', '"profile":"full"')
    journal.path.write_text(tampered, encoding="utf-8")
    with pytest.raises(JournalError):
        journal.records()


def test_partial_trailing_line_is_ignored(tmp_path):
    journal = Journal(tmp_path / "j.jsonl")
    _release(journal)
    with open(journal.path, "a", encoding="utf-8") as handle:
        handle.write('{"seq":2,"type":"intent"')
    records = journal.records()
    assert len(records) == 1


def test_missing_checksum_is_rejected(tmp_path):
    journal = Journal(tmp_path / "j.jsonl")
    journal.path.write_text(json.dumps({"seq": 1, "type": "release"}) + "\n", encoding="utf-8")
    with pytest.raises(JournalError):
        journal.records()


def test_yank_is_terminal_view(tmp_path):
    journal = Journal(tmp_path / "j.jsonl")
    _release(journal)
    journal.append("yank", release="v1.0.0", reason="bad build")
    assert journal.status() == "yanked"
    assert journal.yanked() is True
