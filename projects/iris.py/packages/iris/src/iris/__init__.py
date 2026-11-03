"""Iris ORM Python binding.

The binding drives real Iris sessions over the same Rust session core as the
Node N-API binding: reference `memory`, `sqlite`, YYDB (`.yydb` and `.yydx`),
PostgreSQL and MySQL profiles all accept VOS programs, structured operation
payloads and typed parameter carriers.

Session calls never raise for backend, planning or binding failures. They
return ``{"ok": bool, "rows": list, "error": {"code": str, "message": str} |
None}`` so callers can branch on the diagnostic code. Session construction
failures raise :class:`IrisError` with the same code.
"""

from __future__ import annotations

import importlib
import json
from typing import Any, NoReturn

from .errors import IrisError

__all__ = [
    "IrisError",
    "Session",
    "capabilities",
    "contract_version",
    "openMemory",
    "openMysql",
    "openPostgres",
    "openSqlite",
    "openYydb",
    "version",
]

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


def _decode(raw: str) -> dict[str, Any]:
    """Decode a structured session envelope."""
    return json.loads(raw)


def _raise_native(error: Exception) -> NoReturn:
    """Re-raise a native construction failure as `IrisError` with its code."""
    payload = _decode(error.args[0]) if error.args else {}
    message = payload.get("message") or str(error)
    raise IrisError(message, code=payload.get("code", "IRIS-EXEC-FAILED")) from error


def version() -> str:
    """Return the version of the Iris Rust facade."""
    return _native().version()


def contract_version() -> str:
    """Return the contract envelope format accepted by Iris."""
    return _native().contract_version()


def capabilities() -> list[str]:
    """Return the capability ids exposed by this binding."""
    return list(_native().capabilities())


class Session:
    """Iris session bound to a reference or foreign adapter."""

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
        """Apply the managed-push schema to a YYDB or SQLite session."""
        return _decode(self._session.managed_push(schema))

    def begin(self) -> dict[str, Any]:
        """Open a transaction on profiles that expose transaction control."""
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
        """Close the session, releasing the bound datasource."""
        self._session.close()


def _open(native_session: Any) -> Session:
    return Session(native_session)


def openMemory() -> Session:
    """Open the reference profile (read-only and unseeded)."""
    return _open(_native().open_memory())


def openSqlite(path: str = ":memory:") -> Session:
    """Open a SQLite profile (`:memory:` or a file path)."""
    try:
        return _open(_native().open_sqlite(path))
    except Exception as error:  # noqa: BLE001 - normalized to IrisError
        _raise_native(error)


def openYydb(path: str = ":memory:") -> Session:
    """Open a YYDB profile over a `.yydb` file or `.yydx` directory layout."""
    try:
        return _open(_native().open_yydb(path))
    except Exception as error:  # noqa: BLE001 - normalized to IrisError
        _raise_native(error)


def openPostgres(url: str) -> Session:
    """Open a PostgreSQL profile over a real server URL."""
    try:
        return _open(_native().open_postgres(url))
    except Exception as error:  # noqa: BLE001 - normalized to IrisError
        _raise_native(error)


def openMysql(url: str) -> Session:
    """Open a MySQL profile over a real server URL.

    The endpoint and authentication parameters are supplied by the YYDS
    protocol frontend once it publishes the MySQL subset contract.
    """
    try:
        return _open(_native().open_mysql(url))
    except Exception as error:  # noqa: BLE001 - normalized to IrisError
        _raise_native(error)
