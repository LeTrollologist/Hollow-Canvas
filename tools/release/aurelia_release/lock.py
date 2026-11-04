"""Single-writer lock so two releases cannot mutate the same repository."""

from __future__ import annotations

import json
import os
import socket
import time
import uuid
from pathlib import Path
from types import TracebackType

from .errors import LockError
from .util import canonical_json, utc_now

STALE_AFTER_SECONDS = 24 * 60 * 60


class ReleaseLock:
    def __init__(self, path: Path, *, break_stale: bool = False) -> None:
        self.path = path
        self.break_stale = break_stale
        self._token: str | None = None

    def __enter__(self) -> "ReleaseLock":
        self.path.parent.mkdir(parents=True, exist_ok=True)
        if self.path.exists() and self.break_stale:
            age = time.time() - self.path.stat().st_mtime
            if age > STALE_AFTER_SECONDS:
                self.path.unlink(missing_ok=True)

        token = uuid.uuid4().hex
        payload = canonical_json(
            {
                "pid": os.getpid(),
                "host": socket.gethostname(),
                "started": utc_now(),
                "token": token,
            }
        )
        try:
            fd = os.open(self.path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
        except FileExistsError as exc:
            try:
                owner = json.loads(self.path.read_text(encoding="utf-8"))
                holder = f"pid {owner.get('pid')} on {owner.get('host')} since {owner.get('started')}"
            except (OSError, ValueError):
                holder = "unknown holder"
            raise LockError(
                f"another release run holds {self.path} ({holder}); "
                "use --break-lock only if that process is gone"
            ) from exc

        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(payload + "\n")
            handle.flush()
            os.fsync(handle.fileno())
        self._token = token
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        if not self._token:
            return
        try:
            owner = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            owner = {}
        if owner.get("token") == self._token:
            self.path.unlink(missing_ok=True)
        self._token = None
