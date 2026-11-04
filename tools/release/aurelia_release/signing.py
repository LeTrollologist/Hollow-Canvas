"""Ed25519 artifact signing and checksum verification.

The private key may be a raw 32-byte seed (hex or binary), an OpenSSH-style
ed25519 private key exported as PEM, or an encrypted PKCS#8 PEM whose passphrase
comes from an environment variable. Signatures are detached raw 64-byte files
(`<artifact>.sig`), and `SHA256SUMS.txt` is signed as well.
"""

from __future__ import annotations

import hashlib
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from .config import SigningConfig
from .errors import ReleaseError
from .util import sha256_file


class SigningError(ReleaseError):
    """Signing or signature verification failed."""


@dataclass(frozen=True)
class KeyPair:
    private: object
    public: object


def _crypto():
    try:
        from cryptography.hazmat.primitives import serialization
        from cryptography.hazmat.primitives.asymmetric.ed25519 import (
            Ed25519PrivateKey,
            Ed25519PublicKey,
        )
    except ImportError as exc:  # pragma: no cover - environment dependent
        raise SigningError(
            "artifact signing requires the 'cryptography' package "
            "(install with: python -m pip install cryptography)"
        ) from exc
    return serialization, Ed25519PrivateKey, Ed25519PublicKey


def _read_seed(data: bytes) -> bytes:
    try:
        text = data.decode("ascii").strip()
    except UnicodeDecodeError:
        text = ""
    if len(text) == 64:
        try:
            return bytes.fromhex(text)
        except ValueError as exc:
            raise SigningError("private key hex is invalid") from exc
    if len(data) == 32:
        return data
    raise SigningError(
        "private key must be a 32-byte Ed25519 seed, 64 hex characters, or PEM"
    )


def load_private_key(path: Path, passphrase: str | None = None):
    serialization, Ed25519PrivateKey, _ = _crypto()
    if not path.is_file():
        raise SigningError(f"private key not found: {path}")
    data = path.read_bytes()
    if b"BEGIN" in data:
        password = passphrase.encode("utf-8") if passphrase else None
        try:
            key = serialization.load_pem_private_key(data, password=password)
        except (ValueError, TypeError) as exc:
            raise SigningError(f"cannot read private key {path}: {exc}") from exc
    else:
        try:
            key = Ed25519PrivateKey.from_private_bytes(_read_seed(data))
        except ValueError as exc:
            raise SigningError(f"cannot read private key {path}: {exc}") from exc
    if not isinstance(key, Ed25519PrivateKey):
        raise SigningError(f"{path} is not an Ed25519 private key")
    return key


def load_public_key(path: Path):
    serialization, _, Ed25519PublicKey = _crypto()
    if not path.is_file():
        raise SigningError(f"public key not found: {path}")
    data = path.read_bytes()
    if b"BEGIN" in data:
        try:
            key = serialization.load_pem_public_key(data)
        except ValueError as exc:
            raise SigningError(f"cannot read public key {path}: {exc}") from exc
    else:
        try:
            text = data.decode("ascii").strip()
        except UnicodeDecodeError:
            text = ""
        try:
            raw = bytes.fromhex(text) if len(text) == 64 else data
            key = Ed25519PublicKey.from_public_bytes(raw)
        except (ValueError, TypeError) as exc:
            raise SigningError(f"cannot read public key {path}: {exc}") from exc
    if not isinstance(key, Ed25519PublicKey):
        raise SigningError(f"{path} is not an Ed25519 public key")
    return key


def load_keypair(repo: Path, cfg: SigningConfig) -> KeyPair:
    passphrase = os.environ.get(cfg.passphrase_env, "") if cfg.passphrase_env else None
    private = load_private_key(repo / cfg.private_key, passphrase)
    public = load_public_key(repo / cfg.public_key)

    serialization, _, _ = _crypto()
    derived = private.public_key().public_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PublicFormat.Raw,
    )
    expected = public.public_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PublicFormat.Raw,
    )
    if derived != expected:
        raise SigningError(
            f"{cfg.public_key} does not match the public key derived from "
            f"{cfg.private_key}"
        )
    return KeyPair(private=private, public=public)


def sign_bytes(private, data: bytes) -> bytes:
    return private.sign(data)


def verify_bytes(public, data: bytes, signature: bytes) -> None:
    try:
        public.verify(signature, data)
    except Exception as exc:  # cryptography raises InvalidSignature
        raise SigningError("signature does not match the artifact") from exc


def write_checksums(directory: Path, files: Iterable[Path]) -> Path:
    target = directory / "SHA256SUMS.txt"
    lines = [f"{sha256_file(path)}  {path.name}" for path in sorted(files)]
    target.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    return target


def verify_checksums(path: Path, files: Iterable[Path]) -> None:
    expected = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        digest, _, name = line.partition("  ")
        expected[name] = digest.strip().lower()
    for item in files:
        digest = expected.get(item.name)
        if digest is None:
            raise SigningError(f"SHA256SUMS.txt is missing {item.name}")
        actual = hashlib.sha256(item.read_bytes()).hexdigest()
        if digest != actual:
            raise SigningError(f"{item.name}: checksum mismatch")
