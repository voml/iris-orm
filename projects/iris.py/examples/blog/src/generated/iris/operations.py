"""Generated entity delegates — the authoring surface of this schema.

`create_client(dsn)` opens one client, `client.tables.<entity>` exposes the
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

    UserCreateInput,
    UserSelect,
    UserUpdateInput,
    UserWhereInput,

)
from .models import (

    User,

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



class UserDelegate:
    """Entity delegate for `User`."""

    def __init__(self, executor: OperationExecutor) -> None:
        self._executor = executor

    def find_many(
        self,
        *,
        where: UserWhereInput | None = None,
        select: UserSelect | None = None,
        take: int | None = None,
    ) -> list[User]:
        synthesis = synthesize_find_many("User", where, select, take)
        rows = self._executor.execute(
            build_declared_operation_request("User.findMany", synthesis)
        )
        return [User(row) for row in rows]

    def find_first(
        self,
        *,
        where: UserWhereInput | None = None,
        select: UserSelect | None = None,
    ) -> User | None:
        synthesis = synthesize_find_many("User", where, select, 1)
        rows = self._executor.execute(
            build_declared_operation_request("User.findFirst", synthesis)
        )
        return User(rows[0]) if rows else None

    def find_unique(
        self,
        *,
        where: UserWhereInput,
        select: UserSelect | None = None,
    ) -> User | None:
        synthesis = synthesize_find_unique("User", where, select)
        rows = self._executor.execute(
            build_declared_operation_request("User.findUnique", synthesis)
        )
        return User(rows[0]) if rows else None

    def create(
        self,
        *,
        data: UserCreateInput,
        select: UserSelect | None = None,
    ) -> User | None:
        """Insert one row and return it when the adapter echoes the row back."""
        synthesis = synthesize_create("User", data, select)
        rows = self._executor.execute(
            build_declared_operation_request("User.create", synthesis)
        )
        return User(rows[0]) if rows else None

    def update(
        self,
        *,
        where: UserWhereInput,
        data: UserUpdateInput,
        select: UserSelect | None = None,
    ) -> User | None:
        """Update matching rows and return the first echoed row.

        Raises `IrisError` with `IRIS-NOT-FOUND` when `where` matches no row.
        """
        _require_match(
            self._executor.execute(
                build_declared_operation_request(
                    "User.findMany",
                    synthesize_find_many("User", where, None, 1),
                )
            ),
            "User",
            "update",
        )
        synthesis = synthesize_update("User", where, data, select)
        rows = self._executor.execute(
            build_declared_operation_request("User.update", synthesis)
        )
        return User(rows[0]) if rows else None

    def delete(self, *, where: UserWhereInput) -> None:
        """Delete matching rows.

        Raises `IrisError` with `IRIS-NOT-FOUND` when `where` matches no row.
        """
        _require_match(
            self._executor.execute(
                build_declared_operation_request(
                    "User.findMany",
                    synthesize_find_many("User", where, None, 1),
                )
            ),
            "User",
            "delete",
        )
        synthesis = synthesize_delete("User", where)
        self._executor.execute_unit(
            build_declared_operation_request("User.delete", synthesis)
        )

    def find_near(
        self,
        *,
        field: str,
        vector: Sequence[float],
        metric: str,
        threshold: float,
        where: UserWhereInput | None = None,
        take: int | None = None,
    ) -> list[User]:
        synthesis = synthesize_vector_near("User", field, vector, metric, threshold, where)
        rows = self._executor.execute(
            build_declared_operation_request("User.findNear", synthesis)
        )
        return [
            User(row)
            for row in evaluate_vector_near_rows(
                "User", rows, field, vector, metric, threshold, take
            )
        ]



class Tables:
    """Entity namespace of one client: `client.tables.<entity>`."""


    user: UserDelegate


    def __init__(self, executor: OperationExecutor) -> None:
        self._executor = executor

        self.user = UserDelegate(self._executor)



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
