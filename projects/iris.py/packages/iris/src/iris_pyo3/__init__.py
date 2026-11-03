"""Import shim for the compiled ``iris_pyo3`` extension module."""

from __future__ import annotations

from .iris_pyo3 import (
    Session,
    capabilities,
    contract_version,
    open_memory,
    open_mysql,
    open_postgres,
    open_sqlite,
    open_yydb,
    version,
)

__all__ = [
    "Session",
    "capabilities",
    "contract_version",
    "open_memory",
    "open_mysql",
    "open_postgres",
    "open_sqlite",
    "open_yydb",
    "version",
]
