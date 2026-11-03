"""Generated entity delegates — the authoring surface of this schema.

`create_client(dsn)` opens one client, `client.tables.<Entity>` exposes the
entity delegates and `client.transaction()` groups writes. No method here
accepts VOS text, a backend name, or a raw SQL statement.
"""

from __future__ import annotations

from typing import Any, Sequence

from ._internal.executor import OperationExecutor, open_executor
from ._internal.synthesize import (
    build_declared_operation_request,
    evaluate_vector_near_rows,
    synthesize_create,
    synthesize_delete,
    synthesize_find_many,
    synthesize_find_unique,
    synthesize_operation_call,
    synthesize_update,
    synthesize_vector_near,
)
from .errors import IrisError
from .inputs import (

    DocCreateInput,
    DocSelect,
    DocUpdateInput,
    DocWhereInput,

)
from .models import (

    Doc,

)


def _require_match(rows: Sequence[Any], entity: str, operation: str) -> None:
    """Raise `IRIS-NOT-FOUND` when the unique `where` matched no row."""
    if rows:
        return
    raise IrisError(
        f"`{entity}.{operation}` matched no row",
        code="IRIS-NOT-FOUND",
        kind="not_found",
    )



class DocDelegate:
    """Entity delegate for `Doc`."""

    def __init__(self, executor: OperationExecutor) -> None:
        self._executor = executor

    def find_many(
        self,
        *,
        where: DocWhereInput | None = None,
        select: DocSelect | None = None,
        take: int | None = None,
    ) -> list[Doc]:
        synthesis = synthesize_find_many("Doc", where, select, take)
        return self._executor.execute(
            build_declared_operation_request("Doc.findMany", synthesis)
        )

    def find_first(
        self,
        *,
        where: DocWhereInput | None = None,
        select: DocSelect | None = None,
    ) -> Doc | None:
        synthesis = synthesize_find_many("Doc", where, select, 1)
        rows = self._executor.execute(
            build_declared_operation_request("Doc.findFirst", synthesis)
        )
        return rows[0] if rows else None

    def find_unique(
        self,
        *,
        where: DocWhereInput,
        select: DocSelect | None = None,
    ) -> Doc | None:
        synthesis = synthesize_find_unique("Doc", where, select)
        rows = self._executor.execute(
            build_declared_operation_request("Doc.findUnique", synthesis)
        )
        return rows[0] if rows else None

    def create(
        self,
        *,
        data: DocCreateInput,
        select: DocSelect | None = None,
    ) -> Doc | None:
        """Insert one row and return it when the adapter echoes the row back."""
        synthesis = synthesize_create("Doc", data, select)
        rows = self._executor.execute(
            build_declared_operation_request("Doc.create", synthesis)
        )
        return rows[0] if rows else None

    def update(
        self,
        *,
        where: DocWhereInput,
        data: DocUpdateInput,
        select: DocSelect | None = None,
    ) -> Doc | None:
        """Update matching rows and return the first echoed row.

        Raises `IrisError` with `IRIS-NOT-FOUND` when `where` matches no row.
        """
        _require_match(
            self._executor.execute(
                build_declared_operation_request(
                    "Doc.findMany",
                    synthesize_find_many("Doc", where, None, 1),
                )
            ),
            "Doc",
            "update",
        )
        synthesis = synthesize_update("Doc", where, data, select)
        rows = self._executor.execute(
            build_declared_operation_request("Doc.update", synthesis)
        )
        return rows[0] if rows else None

    def delete(self, *, where: DocWhereInput) -> None:
        """Delete matching rows.

        Raises `IrisError` with `IRIS-NOT-FOUND` when `where` matches no row.
        """
        _require_match(
            self._executor.execute(
                build_declared_operation_request(
                    "Doc.findMany",
                    synthesize_find_many("Doc", where, None, 1),
                )
            ),
            "Doc",
            "delete",
        )
        synthesis = synthesize_delete("Doc", where)
        self._executor.execute_unit(
            build_declared_operation_request("Doc.delete", synthesis)
        )

    def find_near(
        self,
        *,
        field: str,
        vector: Sequence[float],
        metric: str,
        threshold: float,
        where: DocWhereInput | None = None,
        take: int | None = None,
    ) -> list[Doc]:
        synthesis = synthesize_vector_near("Doc", field, vector, metric, threshold, where)
        rows = self._executor.execute(
            build_declared_operation_request("Doc.findNear", synthesis)
        )
        return list(
            evaluate_vector_near_rows("Doc", rows, field, vector, metric, threshold, take)
        )



class Tables:
    """Entity namespace of one client: `client.tables.<Entity>`."""


    Doc: DocDelegate


    def __init__(self, executor: OperationExecutor) -> None:
        self._executor = executor

        self.Doc = DocDelegate(self._executor)



class Transaction:
    """Transaction scope: normal exit commits, an exception rolls back."""

    def __init__(self, executor: OperationExecutor) -> None:
        self._executor = executor
        self.tables = Tables(executor)

    def __enter__(self) -> "Transaction":
        self._executor.begin()
        return self

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> bool:
        if exc_type is None:
            self._executor.commit()
        else:
            self._executor.rollback()
        return False


class Client:
    """One generated client bound to the adapter its DSN selects."""

    def __init__(self, executor: OperationExecutor) -> None:
        self._executor = executor
        self.tables = Tables(executor)


    def transaction(self) -> Transaction:
        return Transaction(self._executor)


def create_client(dsn: str, **options: Any) -> Client:
    """Open a generated client for `dsn`.

    The DSN selects the adapter internally (`sqlite:`, `file:`, `mysql://`,
    `postgres://`). Managed adapters apply the schema this module was
    generated from. Unknown options are rejected rather than ignored.
    """
    from .metadata import SCHEMA_SOURCE

    return Client(open_executor(dsn, SCHEMA_SOURCE, options))
