from __future__ import annotations

import shutil
import sys
import uuid
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

_WORK_ROOT = Path(__file__).resolve().parent / "_work"


@pytest.fixture
def tmp_path():
    """A normal-mode workspace directory.

    The default pytest temp factory creates 0o700 directories, which the Windows
    sandbox used for this repository denies listing; tests only need a fresh
    writable directory, not that specific mode.
    """
    _WORK_ROOT.mkdir(exist_ok=True)
    path = _WORK_ROOT / uuid.uuid4().hex
    path.mkdir()
    try:
        yield path
    finally:
        shutil.rmtree(path, ignore_errors=True)
