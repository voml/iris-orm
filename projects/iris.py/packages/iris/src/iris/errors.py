"""Errors raised by the Iris Python binding."""

from __future__ import annotations


class IrisError(Exception):
    """Base error raised by the Iris Python binding.

    ``kind`` and ``code`` are the author-facing pair produced by the Rust
    session core. ``diagnostic_code`` keeps the internal detail for logs.
    Session operations do not raise for backend failures: they return the
    structured envelope described in :class:`iris.SessionResult` instead.
    """

    def __init__(
        self,
        message: str,
        *,
        code: str = "IRIS-INTERNAL",
        kind: str = "internal",
        diagnostic_code: str | None = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.kind = kind
        self.diagnostic_code = diagnostic_code
