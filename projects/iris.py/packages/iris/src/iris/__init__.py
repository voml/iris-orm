"""Iris ORM for Python — runtime package.

Public surface: `IrisError`, `version`, `contract_version` and `generate`.
Everything else is generated per project from its VOS schema: `create_client(dsn)`
and `client.tables.<Entity>`.

`generate` is a build-time primitive backed by the facade generator: it writes
a client package from schema source. Session plumbing, DSN parsing and adapter
selection live in the Rust session core (`iris_pyo3`); generated clients call
`iris_pyo3` directly and never import this runtime package.
"""

from __future__ import annotations

from typing import Any

from ._runtime import generate as _generate
from ._runtime import native
from .errors import IrisError

__all__ = ["IrisError", "contract_version", "generate", "version"]


def version() -> str:
    """Return the version of the Iris Rust facade."""
    return native().version()


def contract_version() -> str:
    """Return the contract envelope format accepted by Iris."""
    return native().contract_version()


def generate(source: str, target: str, out_dir: str, datasource: str | None = None) -> dict[str, Any]:
    """Generate a client from VOS schema source into `out_dir`.

    Returns the native JSON envelope: `ok`, `output_path`,
    `schema_fingerprint`, `files` and `error`. Parsing and emission stay in the
    Rust facade; this function does not re-implement them.
    """
    return _generate(source, target, out_dir, datasource)
