"""Iris ORM Python binding.

The binding currently exposes version and capability probing only. Schema
authoring, generation, and runtime data access stay in the Rust and Node
surfaces until they are bound here.
"""

from __future__ import annotations

import importlib
from typing import Any

from .errors import IrisError

__all__ = ["IrisError", "capabilities", "contract_version", "version"]

_NATIVE_MODULE = "iris_pyo3"

_MISSING_NATIVE = (
    "the `iris_pyo3` native module is not importable. Install the built wheel or run "
    "`maturin develop` before calling the binding"
)


def _native() -> Any:
    """Return the compiled `iris_pyo3` module."""
    try:
        return importlib.import_module(_NATIVE_MODULE)
    except ImportError as error:
        raise IrisError(_MISSING_NATIVE) from error


def version() -> str:
    """Return the version of the Iris Rust facade."""
    return _native().version()


def contract_version() -> str:
    """Return the contract envelope format accepted by Iris."""
    return _native().contract_version()


def capabilities() -> list[str]:
    """Return the capability ids exposed by this binding."""
    return list(_native().capabilities())
