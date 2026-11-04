"""Load and validate the single release configuration (release.toml)."""

from __future__ import annotations

import tomllib
from dataclasses import asdict, dataclass, field, replace
from pathlib import Path
from typing import Any

from .errors import ConfigError
from .util import fingerprint

KNOWN_STAGES: tuple[str, ...] = (
    "preflight",
    "version",
    "quality",
    "build",
    "package",
    "sign",
    "scan",
    "tag",
    "publish",
    "release",
    "verify",
)

DEFAULT_VERSION_FILES: tuple[str, ...] = (
    "Cargo.toml",
    "crates/aurelia-core/Cargo.toml",
    "crates/aurelia-render/Cargo.toml",
    "crates/aurelia-io/Cargo.toml",
    "crates/aurelia-ui/Cargo.toml",
    "crates/aurelia-app/Cargo.toml",
    "README.md",
    "INSTALL.md",
    "Makefile",
    "assets/banner.svg",
    "installer/aurelia.iss",
)

DEFAULT_PROFILES: dict[str, tuple[str, ...]] = {
    "git": ("preflight", "version", "tag", "publish", "verify"),
    "full": (
        "preflight",
        "version",
        "quality",
        "build",
        "package",
        "sign",
        "scan",
        "tag",
        "publish",
        "release",
        "verify",
    ),
}


@dataclass(frozen=True)
class GitConfig:
    remote: str = "origin"
    branch: str = "main"
    sign_tags: bool = True


@dataclass(frozen=True)
class ScanConfig:
    mode: str = "threshold"  # off | advisory | threshold | strict
    threshold: int = 3
    blocking_engines: tuple[str, ...] = ()
    allowlist: tuple[str, ...] = ()
    api_key_env: str = "VIRUSTOTAL_API_KEY"


@dataclass(frozen=True)
class PublishConfig:
    github: bool = True
    draft: bool = True
    repo: str = ""
    token_env: str = "GITHUB_TOKEN"
    assets: tuple[str, ...] = ("dist/{tag}/*",)


@dataclass(frozen=True)
class SigningConfig:
    enabled: bool = False
    private_key: str = "keys/aurelia-publisher.priv"
    public_key: str = "keys/aurelia-publisher.pub"
    passphrase_env: str = ""
    sign_assets: bool = True


@dataclass(frozen=True)
class ReleaseConfig:
    profile: str
    tag_prefix: str
    version_files: tuple[str, ...]
    profiles: dict[str, tuple[str, ...]]
    auto_compensate: str
    git: GitConfig
    scan: ScanConfig
    publish: PublishConfig
    signing: SigningConfig
    commands: dict[str, str]
    outputs: dict[str, tuple[str, ...]]
    digest: str = ""

    def stages(self) -> tuple[str, ...]:
        return self.profiles[self.profile]

    def command(self, stage: str) -> str:
        try:
            return self.commands[stage]
        except KeyError as exc:
            raise ConfigError(
                f"profile {self.profile!r} enables stage {stage!r} but [commands].{stage} is missing"
            ) from exc


def _tuple(value: Any) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, str):
        return (value,)
    if not isinstance(value, list):
        raise ConfigError(f"expected a list or string, got {type(value).__name__}")
    return tuple(str(item) for item in value)


def _string_map(value: Any, section: str) -> dict[str, str]:
    if value is None:
        return {}
    if not isinstance(value, dict):
        raise ConfigError(f"[{section}] must be a table")
    return {str(key): str(item) for key, item in value.items()}


def _build(data: dict[str, Any], profile_override: str | None) -> ReleaseConfig:
    release = data.get("release", {})
    if not isinstance(release, dict):
        raise ConfigError("[release] must be a table")

    profiles = dict(DEFAULT_PROFILES)
    raw_profiles = data.get("profiles", {})
    if not isinstance(raw_profiles, dict):
        raise ConfigError("[profiles] must be a table")
    for name, stages in raw_profiles.items():
        profiles[str(name)] = _tuple(stages)

    profile = profile_override or str(release.get("profile", "git"))
    if profile not in profiles:
        raise ConfigError(
            f"unknown profile {profile!r}; configured: {', '.join(sorted(profiles))}"
        )
    for stage in profiles[profile]:
        if stage not in KNOWN_STAGES:
            raise ConfigError(f"profile {profile!r} references unknown stage {stage!r}")

    auto_compensate = str(release.get("auto_compensate", "local"))
    if auto_compensate not in {"none", "local", "all"}:
        raise ConfigError("release.auto_compensate must be none, local, or all")

    raw_git = data.get("git", {})
    raw_scan = data.get("scan", {})
    raw_publish = data.get("publish", {})
    raw_sign = data.get("sign", {})
    for name, value in (
        ("git", raw_git),
        ("scan", raw_scan),
        ("publish", raw_publish),
        ("sign", raw_sign),
    ):
        if not isinstance(value, dict):
            raise ConfigError(f"[{name}] must be a table")

    scan_mode = str(raw_scan.get("mode", "threshold"))
    if scan_mode not in {"off", "advisory", "threshold", "strict"}:
        raise ConfigError("scan.mode must be off, advisory, threshold, or strict")

    raw_outputs = data.get("outputs", {})
    if not isinstance(raw_outputs, dict):
        raise ConfigError("[outputs] must be a table")
    outputs = {str(key): _tuple(value) for key, value in raw_outputs.items()}

    cfg = ReleaseConfig(
        profile=profile,
        tag_prefix=str(release.get("tag_prefix", "v")),
        version_files=_tuple(release.get("version_files", list(DEFAULT_VERSION_FILES))),
        profiles=profiles,
        auto_compensate=auto_compensate,
        git=GitConfig(
            remote=str(raw_git.get("remote", "origin")),
            branch=str(raw_git.get("branch", "main")),
            sign_tags=bool(raw_git.get("sign_tags", True)),
        ),
        scan=ScanConfig(
            mode=scan_mode,
            threshold=int(raw_scan.get("threshold", 3)),
            blocking_engines=_tuple(raw_scan.get("blocking_engines", [])),
            allowlist=_tuple(raw_scan.get("allowlist", [])),
            api_key_env=str(raw_scan.get("api_key_env", "VIRUSTOTAL_API_KEY")),
        ),
        publish=PublishConfig(
            github=bool(raw_publish.get("github", True)),
            draft=bool(raw_publish.get("draft", True)),
            repo=str(raw_publish.get("repo", "")),
            token_env=str(raw_publish.get("token_env", "GITHUB_TOKEN")),
            assets=_tuple(raw_publish.get("assets", ["dist/{tag}/*"])),
        ),
        signing=SigningConfig(
            enabled=bool(raw_sign.get("enabled", False)),
            private_key=str(raw_sign.get("private_key", "keys/aurelia-publisher.priv")),
            public_key=str(raw_sign.get("public_key", "keys/aurelia-publisher.pub")),
            passphrase_env=str(raw_sign.get("passphrase_env", "")),
            sign_assets=bool(raw_sign.get("sign_assets", True)),
        ),
        commands=_string_map(data.get("commands", {}), "commands"),
        outputs=outputs,
    )

    if "release" in cfg.stages() and not cfg.publish.github:
        raise ConfigError(
            "profile enables the 'release' stage but [publish].github is false; "
            "remove that stage or enable GitHub publishing"
        )

    payload = asdict(cfg)
    payload.pop("digest", None)
    return replace(cfg, digest=fingerprint(payload))


def load_config(
    repo: Path,
    *,
    path: Path | None = None,
    profile: str | None = None,
) -> ReleaseConfig:
    """Load release.toml, falling back to safe defaults when it is absent."""
    config_path = path or (repo / "release.toml")
    if config_path.exists():
        try:
            with open(config_path, "rb") as handle:
                data = tomllib.load(handle)
        except tomllib.TOMLDecodeError as exc:
            raise ConfigError(f"{config_path}: {exc}") from exc
    else:
        data = {}

    return _build(data, profile)


def config_payload(cfg: ReleaseConfig) -> dict[str, Any]:
    """Stable representation for fingerprints and plan output."""
    payload = asdict(cfg)
    payload.pop("digest", None)
    return payload
