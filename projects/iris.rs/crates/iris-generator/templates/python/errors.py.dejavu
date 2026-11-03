"""Error surface of the generated client.

`IrisError` carries the stable author-facing `code`
(`IRIS-UNIQUE-CONFLICT`, `IRIS-NOT-FOUND`, …) produced by the Rust session
core. Diagnostic detail such as a backend error number never appears as
`code`; it stays inside the envelope for logs.
"""

from __future__ import annotations


class IrisError(Exception):
    """Semantic failure raised by this generated client."""

    def __init__(self, message: str, *, code: str = "IRIS-INTERNAL") -> None:
        super().__init__(message)
        self.message = message
        self.code = code


__all__ = ["IrisError"]
