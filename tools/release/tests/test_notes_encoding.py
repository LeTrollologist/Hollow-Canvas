from __future__ import annotations

from aurelia_release.stages import notes_encoding_problem


def test_clean_utf8_notes_have_no_problem():
    text = "# Aurelia v9.9.9 \u2014 workflows\n\nArrows \u2192 and dashes \u2013 stay intact.\n"
    assert notes_encoding_problem(text) is None


def test_bom_is_rejected():
    assert "BOM" in (notes_encoding_problem("\ufeff# Notes\n") or "")


def test_mojibake_arrows_and_dashes_are_rejected_with_the_fix():
    for text in (
        "File \u00e2\u2020\u2019 Preferences\n",
        "interval 15\u00e2\u20ac\u201c3600 s\n",
        "\u00c3\u2014 1080 px\n",
        "lost \ufffd byte\n",
    ):
        problem = notes_encoding_problem(text)
        assert problem, text
        assert "mojibake" in problem
        assert "PowerShell 5.1" in problem


def test_real_release_notes_in_the_repository_are_clean():
    from pathlib import Path

    notes = Path(__file__).resolve().parents[3] / "release-notes" / "v3.6.0.md"
    if notes.is_file():
        assert notes_encoding_problem(notes.read_text(encoding="utf-8")) is None
