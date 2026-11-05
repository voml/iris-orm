"""Type stub for the `iris` Python runtime package.

`IrisError`, `version`, `contract_version` and `generate` are public. Generated
clients provide `create_client` and the typed entity delegates.
"""

from __future__ import annotations

from typing import Any

class IrisError(Exception):
    """Base error raised by the Iris Python binding."""

    code: str
    def __init__(self, message: str, *, code: str = ...) -> None: ...

def version() -> str: ...
def contract_version() -> str: ...
def generate(source: str, target: str, out_dir: str, datasource: str | None = ...) -> dict[str, Any]: ...
