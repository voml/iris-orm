"""Type stub for the `iris` Python runtime package.

Only `IrisError`, `version` and `contract_version` are public. Generated
clients provide `create_client` and the typed entity delegates.
"""

from __future__ import annotations

class IrisError(Exception):
    """Base error raised by the Iris Python binding."""

    code: str
    def __init__(self, message: str, *, code: str = ...) -> None: ...

def version() -> str: ...
def contract_version() -> str: ...
