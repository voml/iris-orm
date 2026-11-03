"""Iris ORM for Python — runtime package.

Public surface: `IrisError`, `version` and `contract_version`. Everything else
is generated per project from its VOS schema: `create_client(dsn)` and
`client.tables.<Entity>`.

Session plumbing, DSN parsing and adapter selection live in `iris._runtime`.
"""

from __future__ import annotations

from ._runtime import native
from .errors import IrisError

__all__ = ["IrisError", "contract_version", "version"]


def version() -> str:
    """Return the version of the Iris Rust facade."""
    return native().version()


def contract_version() -> str:
    """Return the contract envelope format accepted by Iris."""
    return native().contract_version()
