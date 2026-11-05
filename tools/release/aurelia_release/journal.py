"""Append-only, checksummed release journal.

The journal is the authoritative record of what the orchestrator intended,
what it actually did, what it verified, and what it compensated. It is written
before and after every mutation so a crash leaves enough evidence to recover
without guessing which script already ran.
"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .errors import JournalError
from .util import canonical_json, fingerprint, sha256_text, utc_now


@dataclass
class OpState:
    name: str
    status: str = "pending"  # pending | running | ok | error
    fp: str | None = None
    before: dict[str, Any] = field(default_factory=dict)
    outputs: dict[str, Any] = field(default_factory=dict)
    verified: bool = False
    verify_status: str | None = None
    verify_details: dict[str, Any] = field(default_factory=dict)
    compensated: bool = False
    compensation_error: str | None = None


class Journal:
    """One journal file per release tag."""

    def __init__(self, path: Path) -> None:
        self.path = path

    # ── writing ─────────────────────────────────────────────────────────
    def append(self, record_type: str, **fields: Any) -> dict[str, Any]:
        records = self.records()
        record: dict[str, Any] = {
            "seq": len(records) + 1,
            "ts": utc_now(),
            "type": record_type,
            **fields,
        }
        record["checksum"] = self._checksum(record)

        self.path.parent.mkdir(parents=True, exist_ok=True)
        with open(self.path, "a", encoding="utf-8", newline="\n") as handle:
            handle.write(canonical_json(record) + "\n")
            handle.flush()
            os.fsync(handle.fileno())
        return record

    @staticmethod
    def _checksum(record: dict[str, Any]) -> str:
        payload = {key: value for key, value in record.items() if key != "checksum"}
        return fingerprint(payload)

    # ── reading ─────────────────────────────────────────────────────────
    def records(self) -> list[dict[str, Any]]:
        if not self.path.exists():
            return []

        raw = self.path.read_bytes()
        if not raw:
            return []

        chunks = raw.splitlines(keepends=True)
        records: list[dict[str, Any]] = []
        for index, chunk in enumerate(chunks):
            is_last = index == len(chunks) - 1
            if not chunk.endswith(b"\n"):
                # A crash mid-append leaves a partial final line. Ignore it; the
                # intent for the operation is already durable on the previous line.
                if is_last:
                    break
                raise JournalError(f"{self.path}: line {index + 1} is not newline terminated")
            try:
                record = json.loads(chunk)
            except json.JSONDecodeError as exc:
                raise JournalError(f"{self.path}: line {index + 1}: {exc}") from exc
            if not isinstance(record, dict):
                raise JournalError(f"{self.path}: line {index + 1} is not an object")
            stored = record.pop("checksum", None)
            if stored != self._checksum(record):
                raise JournalError(f"{self.path}: line {index + 1} failed its checksum")
            records.append(record)
        return records

    def state(self) -> dict[str, OpState]:
        states: dict[str, OpState] = {}
        for record in self.records():
            kind = record.get("type")
            op = record.get("op")
            if kind == "intent" and op:
                states[op] = OpState(
                    name=str(op),
                    status="running",
                    fp=record.get("fp"),
                    before=record.get("before") or {},
                )
            elif kind == "result" and op:
                state = states.setdefault(op, OpState(name=str(op)))
                state.status = "ok" if record.get("status") == "ok" else "error"
                state.outputs = record.get("outputs") or {}
            elif kind == "verify" and op:
                state = states.setdefault(op, OpState(name=str(op)))
                state.verify_status = str(record.get("status") or "failed")
                state.verified = state.verify_status == "passed"
                state.verify_details = record.get("details") or {}
            elif kind == "compensate" and op:
                state = states.setdefault(op, OpState(name=str(op)))
                state.compensated = record.get("status") == "ok"
                if not state.compensated:
                    state.compensation_error = str(record.get("error", "compensation failed"))
        return states

    # ── views ───────────────────────────────────────────────────────────
    def release_record(self) -> dict[str, Any] | None:
        for record in self.records():
            if record.get("type") == "release":
                return record
        return None

    def base_commit(self) -> str | None:
        record = self.release_record()
        return str(record["base_commit"]) if record and record.get("base_commit") else None

    def yanked(self) -> bool:
        return any(record.get("type") == "yank" for record in self.records())

    def status(self) -> str:
        records = self.records()
        if not records:
            return "empty"
        if any(record.get("type") == "yank" for record in records):
            return "yanked"
        if any(record.get("type") == "rollback" for record in records):
            return "rolled_back"
        states = self.state()
        # Only the latest verification for an operation matters. A failed verify that was
        # retried successfully during a resume must not keep the whole release failed forever.
        if any(state.verify_status == "failed" for state in states.values()):
            return "failed"
        if any(state.status == "error" for state in states.values()):
            return "failed"
        if any(record.get("type") == "compensate" for record in records):
            if any(state.status != "ok" for state in states.values() if state.compensated):
                return "needs_manual"
            return "compensated"
        verify_states = [
            state for name, state in states.items() if name.startswith("verify.")
        ]
        if (
            verify_states
            and all(state.verified for state in verify_states)
            and all(state.status == "ok" for state in states.values())
        ):
            return "verified"
        if states.get("publish.push") or states.get("release.publish"):
            return "published"
        return "running"

    def last_error(self) -> str | None:
        if self.status() != "failed":
            return None
        for record in reversed(self.records()):
            if record.get("type") == "result" and record.get("status") == "error":
                return str(record.get("error", "unknown failure"))
            if record.get("type") == "verify" and record.get("status") == "failed":
                return str(record.get("error", "verification failed"))
        return None
