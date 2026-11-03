"""Error surface of the generated client.

`IrisError` carries the stable author-facing `kind` and `code`
(`unique_conflict` / `IRIS-UNIQUE-CONFLICT`, `not_found` / `IRIS-NOT-FOUND`,
…). `diagnostic_code` keeps the internal detail (`IRIS-PLAN-REJECTED`,
`23505`, `yydb.*`) for logs and tests. A backend error number never becomes
`code`.
"""

from __future__ import annotations


class IrisError(Exception):
    """Semantic failure raised by this generated client."""

    def __init__(
        self,
        message: str,
        *,
        code: str = "IRIS-INTERNAL",
        kind: str = "internal",
        diagnostic_code: str | None = None,
    ) -> None:
        super().__init__(message)
        self.message = message
        self.code = code
        self.kind = kind
        self.diagnostic_code = diagnostic_code


__all__ = ["IrisError"]
