"""Expected failures for the Aurelia release orchestrator."""

from __future__ import annotations


class ReleaseError(Exception):
    """Base class for failures the operator can act on."""


class ConfigError(ReleaseError):
    """release.toml is missing, malformed, or inconsistent."""


class PreflightError(ReleaseError):
    """The repository or environment is not ready for a release."""


class JournalError(ReleaseError):
    """The release journal is corrupt or cannot be trusted."""


class LockError(ReleaseError):
    """Another release run holds the lock."""


class VerificationError(ReleaseError):
    """A mutation completed but its independent verification failed."""


class StageError(ReleaseError):
    """A stage failed while executing; the journal holds the intent and error."""


class CompensationError(ReleaseError):
    """A remote or local mutation could not be undone."""


class CommandError(ReleaseError):
    """An external command failed."""

    def __init__(self, message: str, *, stdout: str = "", stderr: str = "") -> None:
        super().__init__(message)
        self.stdout = stdout
        self.stderr = stderr
