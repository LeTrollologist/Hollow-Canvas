from __future__ import annotations

import os
from pathlib import Path

import pytest

pytest.importorskip("cryptography")

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

from aurelia_release.config import load_config
from aurelia_release.engine import Context, Engine
from aurelia_release.journal import Journal
from aurelia_release.signing import (
    SigningError,
    load_keypair,
    sign_bytes,
    verify_bytes,
    verify_checksums,
    write_checksums,
)
from aurelia_release.stages import build_operations


class FakeGit:
    def head(self) -> str:
        return "a" * 40


def write_raw_keys(root: Path) -> None:
    keys = root / "keys"
    keys.mkdir()
    private = Ed25519PrivateKey.generate()
    raw_private = private.private_bytes(
        serialization.Encoding.Raw,
        serialization.PrivateFormat.Raw,
        serialization.NoEncryption(),
    )
    raw_public = private.public_key().public_bytes(
        serialization.Encoding.Raw, serialization.PublicFormat.Raw
    )
    (keys / "aurelia-publisher.priv").write_text(raw_private.hex() + "\n", encoding="ascii")
    (keys / "aurelia-publisher.pub").write_text(raw_public.hex() + "\n", encoding="ascii")


def make_ctx(tmp_path: Path, *, enabled: bool = True) -> Context:
    (tmp_path / "dist" / "v1.0.0").mkdir(parents=True)
    (tmp_path / "dist" / "v1.0.0" / "a.zip").write_bytes(b"artifact-a")
    (tmp_path / "dist" / "v1.0.0" / "b.zip").write_bytes(b"artifact-b")
    write_raw_keys(tmp_path)
    (tmp_path / "release.toml").write_text(
        f"""
[release]
profile = "signed"
version_files = []

[profiles]
signed = ["sign"]

[publish]
assets = ["dist/{{tag}}/*"]

[sign]
enabled = {"true" if enabled else "false"}
private_key = "keys/aurelia-publisher.priv"
public_key = "keys/aurelia-publisher.pub"
""",
        encoding="utf-8",
    )
    cfg = load_config(tmp_path, path=tmp_path / "release.toml")
    return Context(
        repo=tmp_path,
        cfg=cfg,
        tag="v1.0.0",
        version="1.0.0",
        journal=Journal(tmp_path / ".release" / "v1.0.0.jsonl"),
        git=FakeGit(),
        profile=cfg.profile,
        log=lambda *_: None,
    )


def test_keypair_matches_and_detached_signatures_verify(tmp_path):
    write_raw_keys(tmp_path)
    cfg = load_config(tmp_path)
    # Default config has signing disabled but the key files are what matters here.
    from aurelia_release.config import SigningConfig

    keypair = load_keypair(tmp_path, SigningConfig())
    signature = sign_bytes(keypair.private, b"payload")
    verify_bytes(keypair.public, b"payload", signature)
    with pytest.raises(SigningError):
        verify_bytes(keypair.public, b"tampered", signature)


def test_signing_stage_writes_checksums_and_detached_signatures(tmp_path):
    ctx = make_ctx(tmp_path)
    operations = build_operations(ctx)
    operation = next(op for op in operations if op.name == "sign")
    result = operation.run(ctx)
    paths = set(result["paths"])
    assert "dist/v1.0.0/SHA256SUMS.txt" in paths
    assert "dist/v1.0.0/SHA256SUMS.txt.sig" in paths
    assert "dist/v1.0.0/a.zip.sig" in paths
    assert "dist/v1.0.0/b.zip.sig" in paths
    operation.verify(ctx, {"files": {}, "meta": result})
    verify_checksums(tmp_path / "dist" / "v1.0.0" / "SHA256SUMS.txt", [
        tmp_path / "dist" / "v1.0.0" / "a.zip",
        tmp_path / "dist" / "v1.0.0" / "b.zip",
    ])


def test_signing_stage_detects_tampering(tmp_path):
    ctx = make_ctx(tmp_path)
    operations = build_operations(ctx)
    operation = next(op for op in operations if op.name == "sign")
    result = operation.run(ctx)
    (tmp_path / "dist" / "v1.0.0" / "a.zip").write_bytes(b"tampered")
    with pytest.raises(SigningError):
        operation.verify(ctx, {"files": {}, "meta": result})


def test_encrypted_pem_private_key_uses_passphrase_env(tmp_path):
    keys = tmp_path / "keys"
    keys.mkdir()
    private = Ed25519PrivateKey.generate()
    pem = private.private_bytes(
        serialization.Encoding.PEM,
        serialization.PrivateFormat.PKCS8,
        serialization.BestAvailableEncryption(b"secret"),
    )
    (keys / "aurelia-publisher.priv").write_text(pem.decode("ascii"), encoding="ascii")
    public = private.public_key().public_bytes(
        serialization.Encoding.Raw, serialization.PublicFormat.Raw
    )
    (keys / "aurelia-publisher.pub").write_text(public.hex() + "\n", encoding="ascii")

    os.environ["AURELIA_KEY_PASSPHRASE"] = "secret"
    try:
        from aurelia_release.config import SigningConfig

        keypair = load_keypair(
            tmp_path, SigningConfig(passphrase_env="AURELIA_KEY_PASSPHRASE")
        )
        signature = sign_bytes(keypair.private, b"payload")
        verify_bytes(keypair.public, b"payload", signature)
    finally:
        os.environ.pop("AURELIA_KEY_PASSPHRASE", None)


def test_signing_stage_runs_in_the_engine(tmp_path):
    ctx = make_ctx(tmp_path)
    Engine(ctx, build_operations(ctx)).run()
    state = ctx.journal.state()["sign"]
    assert state.verified is True
    assert (tmp_path / "dist" / "v1.0.0" / "a.zip.sig").is_file()
