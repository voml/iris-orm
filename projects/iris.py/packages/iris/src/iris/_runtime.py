"""Internal runtime plumbing kept next to the Python binding.

This module is not part of the authoring surface and is not imported by
generated packages: generated clients talk to the compiled ``iris_pyo3``
extension directly, so a generated package named after the project cannot
shadow the installed ``iris`` package. ``_runtime`` exists for the binding's
own tests and for hosts that want the low-level profile entry points.

``iris_pyo3`` owns DSN parsing, adapter selection and session lifetime in Rust,
so no Python-side DSN parser can drift from the Node one.
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
    payload: dict[str, Any] = {}
    if error.args and isinstance(error.args[0], str):
        try:
            payload = _decode(error.args[0])
        except ValueError:
            payload = {}
    raise IrisError(
        payload.get("message") or str(error),
        code=payload.get("code", "IRIS-INTERNAL"),
        kind=payload.get("kind", "internal"),
        diagnostic_code=payload.get("diagnosticCode"),
    ) from error


def generate(source: str, target: str, out_dir: str, datasource: str | None = None) -> dict[str, Any]:
    """Generate a client from VOS schema source through the facade generator."""
    return _decode(native().generate(source, target, out_dir, datasource))


class Session:
    """Session bound to a reference or foreign adapter.

    Every fallible call returns the structured envelope
    ``{"ok": bool, "rows": list, "error": {"kind": str, "code": str,
    "message": str, "diagnosticCode": str | None} | None}``
    so internal callers branch on the kind / code pair instead of display text.
    """

    def __init__(self, native_session: Any) -> None:
        self._session = native_session

    def query(self, source: str, parameters_json: str | None = None) -> dict[str, Any]:
        """Plan and execute VOS DML, returning the encoded row set."""
        return _decode(self._session.query(source, parameters_json))

    def execute(self, source: str, parameters_json: str | None = None) -> dict[str, Any]:
        """Execute unit-valued or DDL-shaped VOS."""
        return _decode(self._session.execute(source, parameters_json))

    def executeOperation(self, request_json: str) -> dict[str, Any]:
        """Execute a structured Iris operation payload (generated client ABI)."""
        return _decode(self._session.execute_operation(request_json))

    def managedPush(self, schema: str) -> dict[str, Any]:
        """Apply the managed-push schema to a managed session."""
        return _decode(self._session.managed_push(schema))

    def begin(self) -> dict[str, Any]:
        """Open a transaction on adapters that expose transaction control."""
        return _decode(self._session.begin())

    def commit(self) -> dict[str, Any]:
        """Commit the active transaction."""
        return _decode(self._session.commit())

    def rollback(self) -> dict[str, Any]:
        """Roll back the active transaction."""
        return _decode(self._session.rollback())

    def sqliteEngineVersion(self) -> str:
        """Return the SQLite engine version for a sqlite session."""
        try:
            return self._session.sqlite_engine_version()
        except Exception as error:  # noqa: BLE001 - normalized to IrisError
            _raise_native(error)
            raise AssertionError("unreachable")

    def sqliteSourceId(self) -> str:
        """Return the SQLite engine source id for a sqlite session."""
        try:
            return self._session.sqlite_source_id()
        except Exception as error:  # noqa: BLE001 - normalized to IrisError
            _raise_native(error)
            raise AssertionError("unreachable")

    def close(self) -> None:
        """Release the bound datasource."""
        self._session.close()


def open_memory() -> Session:
    """Open the reference profile (read-only and unseeded)."""
    return Session(native().open_memory())


openMemory = open_memory


def open_sqlite(path: str = ":memory:") -> Session:
    """Open a SQLite profile (`:memory:` or a file path)."""
    try:
        return Session(native().open_sqlite(path))
    except Exception as error:  # noqa: BLE001 - normalized to IrisError
        _raise_native(error)


openSqlite = open_sqlite


def open_yydb(path: str = ":memory:") -> Session:
    """Open a YYDB profile over a `.yydb` file or `.yydx` directory layout."""
    try:
        return Session(native().open_yydb(path))
    except Exception as error:  # noqa: BLE001 - normalized to IrisError
        _raise_native(error)


openYydb = open_yydb


def open_postgres(url: str) -> Session:
    """Open a PostgreSQL profile over a real server URL."""
    try:
        return Session(native().open_postgres(url))
    except Exception as error:  # noqa: BLE001 - normalized to IrisError
        _raise_native(error)


openPostgres = open_postgres


def open_mysql(url: str) -> Session:
    """Open a MySQL profile over a real server URL."""
    try:
        return Session(native().open_mysql(url))
    except Exception as error:  # noqa: BLE001 - normalized to IrisError
        _raise_native(error)


openMysql = open_mysql
