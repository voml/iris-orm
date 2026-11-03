"""Errors raised by the Iris Python binding."""

from __future__ import annotations


class IrisError(Exception):
    """Base error raised by the Iris Python binding.

    ``code`` carries the stable machine-readable diagnostic code produced by
    the Rust session core (for example ``IRIS-SESSION-CLOSED``). Session
    operations do not raise for backend failures: they return the structured
    envelope described in :class:`iris.SessionResult` instead.
    """

    def __init__(self, message: str, *, code: str = "IRIS-EXEC-FAILED") -> None:
        super().__init__(message)
        self.code = code
