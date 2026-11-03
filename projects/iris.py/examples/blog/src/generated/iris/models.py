"""Row models returned by the generated client.

Every read returns the entity model, so authors reach columns as attributes
(`row.article_id`). A field is only set when the row carries it, because a
`select` projection may omit any column. Carrier values (`bytes`, `vector`,
`file`) surface as the tagged wire mapping the binding returns.
"""

from __future__ import annotations

from typing import Any, Mapping


class User:
    """Row model of `User`."""

    user_id: str
    user_name: str
    active: bool

    def __init__(self, row: Mapping[str, Any]) -> None:
        if "user_id" in row:
            self.user_id = row["user_id"]
        if "user_name" in row:
            self.user_name = row["user_name"]
        if "active" in row:
            self.active = row["active"]
