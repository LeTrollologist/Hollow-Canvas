from __future__ import annotations

import pytest

from aurelia_release.config import load_config
from aurelia_release.errors import ConfigError


def test_defaults_are_the_git_profile(tmp_path):
    cfg = load_config(tmp_path)
    assert cfg.profile == "git"
    assert cfg.stages() == ("preflight", "version", "tag", "publish", "verify")
    assert cfg.git.sign_tags is True
    assert cfg.scan.mode == "threshold"
    assert cfg.digest


def test_custom_profiles_and_config(tmp_path):
    (tmp_path / "release.toml").write_text(
        """
[release]
profile = "custom"
version_files = ["VERSION"]
auto_compensate = "all"

[profiles]
custom = ["preflight", "tag"]

[git]
sign_tags = false
branch = "trunk"

[scan]
mode = "advisory"
threshold = 5
blocking_engines = ["BadEngine"]

[publish]
github = false

[commands]
quality = "true"

[outputs]
package = ["dist/*.zip"]
""",
        encoding="utf-8",
    )
    cfg = load_config(tmp_path, path=tmp_path / "release.toml")
    assert cfg.profile == "custom"
    assert cfg.stages() == ("preflight", "tag")
    assert cfg.git.sign_tags is False
    assert cfg.git.branch == "trunk"
    assert cfg.scan.mode == "advisory"
    assert cfg.scan.threshold == 5
    assert cfg.scan.blocking_engines == ("BadEngine",)
    assert cfg.publish.github is False
    assert cfg.outputs["package"] == ("dist/*.zip",)


def test_unknown_profile_is_rejected(tmp_path):
    with pytest.raises(ConfigError):
        load_config(tmp_path, profile="nope")


def test_unknown_stage_is_rejected(tmp_path):
    (tmp_path / "release.toml").write_text(
        """
[release]
profile = "bad"
[profiles]
bad = ["preflight", "not-a-stage"]
""",
        encoding="utf-8",
    )
    with pytest.raises(ConfigError):
        load_config(tmp_path, path=tmp_path / "release.toml")


def test_digest_changes_when_config_changes(tmp_path):
    first = load_config(tmp_path)
    (tmp_path / "release.toml").write_text(
        """
[release]
profile = "git"
[git]
branch = "main"
sign_tags = false
""",
        encoding="utf-8",
    )
    second = load_config(tmp_path, path=tmp_path / "release.toml")
    assert first.digest != second.digest
