"""Operation channel between the generated client and the Iris runtime.

Every call is one declared operation: one request, one envelope, one result.
The backend name never reaches this layer — the DSN selects the adapter,
and DSN parsing lives in the Rust session core so both languages agree.
"""

from __future__ import annotations

import importlib
import json
from typing import Any, NoReturn

from ..errors import IrisError

_NATIVE_MODULE = "iris_pyo3"

_MISSING_NATIVE = (
    "the `iris_pyo3` native module is not importable. Install the built wheel or run "
    "`maturin develop` before calling the generated client"
)

_KNOWN_OPTIONS: frozenset[str] = frozenset()


def _native() -> Any:
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
        code=payload.get("code") or "IRIS-INTERNAL",
        kind=payload.get("kind") or "internal",
        diagnostic_code=payload.get("diagnosticCode"),
    ) from error


def _raise_envelope(envelope: dict[str, Any]) -> None:
    error = envelope.get("error") or {}
    raise IrisError(
        error.get("message") or "iris operation failed",
        code=error.get("code") or "IRIS-INTERNAL",
        kind=error.get("kind") or "internal",
        diagnostic_code=error.get("diagnosticCode"),
    )


def _require_ok(envelope: dict[str, Any]) -> dict[str, Any]:
    if not envelope.get("ok"):
        _raise_envelope(envelope)
    return envelope


class OperationExecutor:
    """Thin request/envelope bridge onto one open session."""

    def __init__(self, session: Any) -> None:
        self._session = session

    def execute(self, request: dict[str, Any]) -> list[dict[str, Any]]:
        envelope = _require_ok(_decode(self._session.execute_operation(json.dumps(request))))
        rows = envelope.get("rows") or []
        return list(rows)

    def execute_unit(self, request: dict[str, Any]) -> None:
        _require_ok(_decode(self._session.execute_operation(json.dumps(request))))

    def begin(self) -> None:
        _require_ok(_decode(self._session.begin()))

    def commit(self) -> None:
        _require_ok(_decode(self._session.commit()))

    def rollback(self) -> None:
        _require_ok(_decode(self._session.rollback()))

    def close(self) -> None:
        self._session.close()


def open_executor(dsn: str, schema_source: str = "", options: dict[str, Any] | None = None) -> OperationExecutor:
    """Open the adapter `dsn` selects and wrap it in an executor."""
    unknown = set(options or {}) - _KNOWN_OPTIONS
    if unknown:
        raise IrisError(
            f"unknown client option(s): {', '.join(sorted(unknown))}",
            code="IRIS-INVALID-ARGUMENT",
        )
    try:
        session = _native().open_dsn(dsn, schema_source or None)
    except Exception as error:  # noqa: BLE001 - normalized to IrisError
        _raise_native(error)
    return OperationExecutor(session)
