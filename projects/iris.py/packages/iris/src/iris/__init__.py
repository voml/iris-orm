"""Iris ORM for Python — runtime package.

Public surface: `IrisError`, `version` and `contract_version`. Everything else
is generated per project from its VOS schema: `create_client(dsn)` and
`client.tables.<Entity>`.

Session plumbing, DSN parsing and adapter selection live in the Rust session
core (`iris_pyo3`); generated clients call `iris_pyo3` directly and never
import this runtime package.
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
