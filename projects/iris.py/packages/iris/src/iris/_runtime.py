"""Internal runtime plumbing behind generated Iris clients.

This module is not part of the authoring surface: generated packages import it,
authors do not. It owns DSN parsing, adapter selection, session lifetime and the
operation channel onto the compiled ``iris_pyo3`` extension.

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


class Session:
    """Session bound to a reference or foreign adapter.

    Every fallible call returns the structured envelope
    ``{"ok": bool, "rows": list, "error": {"code": str, "message": str} | None}``
    so internal callers branch on the code instead of display text.
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


def _path_from_dsn(dsn: str) -> str:
    return dsn.split(":", 1)[1]


def open_session(dsn: str) -> Session:
    """Open the adapter the DSN selects and return the session.

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
    if text in ("memory:", "memory"):
        return open_memory()
    if text.startswith("sqlite:"):
        return open_sqlite(_path_from_dsn(text) or ":memory:")
    if text.startswith("file:"):
        path = _path_from_dsn(text)
        if not path:
            raise IrisError("`file:` DSN requires a path", code="IRIS-INVALID-ARGUMENT")
        return open_yydb(path)
    if text.startswith("mysql://"):
        return open_mysql(text)
    if text.startswith(("postgres://", "postgresql://")):
        return open_postgres(text)
    raise IrisError(f"unrecognized DSN `{text}`", code="IRIS-INVALID-ARGUMENT")


def execute_operation(session: Session, request_json: str) -> dict[str, Any]:
    """Run one structured operation request and return its envelope."""
    return session.executeOperation(request_json)


def execute_unit(session: Session, request_json: str) -> dict[str, Any]:
    """Run one unit-valued operation request and return its envelope."""
    return session.executeOperation(request_json)


def begin(session: Session) -> dict[str, Any]:
    """Open a transaction on the bound adapter."""
    return session.begin()


def commit(session: Session) -> dict[str, Any]:
    """Commit the active transaction."""
    return session.commit()


def rollback(session: Session) -> dict[str, Any]:
    """Roll back the active transaction."""
    return session.rollback()


def close_session(session: Session) -> None:
    """Release the bound datasource."""
    session.close()


def managed_push(session: Session, schema: str) -> dict[str, Any]:
    """Apply a managed-push schema (integration fixtures only)."""
    return session.managedPush(schema)
