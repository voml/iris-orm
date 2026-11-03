"""Internal runtime plumbing behind generated Iris clients.

This module is not part of the authoring surface: generated packages import it,
authors do not. It owns DSN parsing, adapter selection and the operation
channel onto the compiled ``iris_pyo3`` extension.

Dropping to this layer is what keeps generated code host-neutral: the same
generated module runs against any adapter the DSN selects, and the backend name
never appears in a generated signature.
"""

from __future__ import annotations

import importlib
import json
from typing import Any, NoReturn

from .errors import IrisError

_NATIVE_MODULE = "iris_pyo3"

_MISSING_NATIVE = (
    "the `iris_pyo3` native module is not importable. Install the built wheel or run "
    "`maturin develop` before calling the binding"
)


def native() -> Any:
    """Return the compiled `iris_pyo3` module."""
    try:
        return importlib.import_module(_NATIVE_MODULE)
    except ImportError as error:
        raise IrisError(_MISSING_NATIVE, code="IRIS-CONNECTION-FAILED") from error


def _decode(raw: str) -> dict[str, Any]:
    """Decode a structured session envelope."""
    return json.loads(raw)


def _raise_native(error: Exception) -> NoReturn:
    """Re-raise a native failure as `IrisError` with its structured code."""
    payload = _decode(error.args[0]) if error.args else {}
    raise IrisError(payload.get("message") or str(error), code=payload.get("code", "IRIS-INTERNAL")) from error


def _path_from_dsn(dsn: str) -> str:
    return dsn.split(":", 1)[1]


def open_session(dsn: str) -> Any:
    """Open the adapter the DSN selects and return the native session.

    Accepted shapes:

    - ``memory:`` — reference profile (generator conformance only)
    - ``sqlite:<path>`` / ``sqlite::memory:``
    - ``file:<path>.yydb`` / ``file:<path>.yydx``
    - ``mysql://user@host:port``
    - ``postgres://user@host:port/database?sslmode=disable``
    """
    if not isinstance(dsn, str) or not dsn.strip():
        raise IrisError("`dsn` must be a non-empty string", code="IRIS-INVALID-ARGUMENT")
    text = dsn.strip()
    binding = native()
    try:
        if text == "memory:" or text == "memory":
            return binding.open_memory()
        if text.startswith("sqlite:"):
            path = _path_from_dsn(text) or ":memory:"
            return binding.open_sqlite(path)
        if text.startswith("file:"):
            path = _path_from_dsn(text)
            if not path:
                raise IrisError("`file:` DSN requires a path", code="IRIS-INVALID-ARGUMENT")
            return binding.open_yydb(path)
        if text.startswith("mysql://"):
            return binding.open_mysql(text)
        if text.startswith(("postgres://", "postgresql://")):
            return binding.open_postgres(text)
    except IrisError:
        raise
    except Exception as error:  # noqa: BLE001 - normalized to IrisError
        _raise_native(error)
        raise AssertionError("unreachable")
    raise IrisError(f"unrecognized DSN `{text}`", code="IRIS-INVALID-ARGUMENT")


def execute_operation(session: Any, request_json: str) -> dict[str, Any]:
    """Run one structured operation request and return its envelope."""
    return _decode(session.execute_operation(request_json))


def execute_unit(session: Any, request_json: str) -> dict[str, Any]:
    """Run one unit-valued operation request and return its envelope."""
    return _decode(session.execute_operation(request_json))


def begin(session: Any) -> dict[str, Any]:
    """Open a transaction on the bound adapter."""
    return _decode(session.begin())


def commit(session: Any) -> dict[str, Any]:
    """Commit the active transaction."""
    return _decode(session.commit())


def rollback(session: Any) -> dict[str, Any]:
    """Roll back the active transaction."""
    return _decode(session.rollback())


def close_session(session: Any) -> None:
    """Release the bound datasource."""
    session.close()


def managed_push(session: Any, schema: str) -> dict[str, Any]:
    """Apply a managed-push schema (integration fixtures only)."""
    return _decode(session.managed_push(schema))
