"""Import shim for the compiled ``iris_pyo3`` extension module."""

from __future__ import annotations

from .iris_pyo3 import capabilities, contract_version, version

__all__ = ["capabilities", "contract_version", "version"]
