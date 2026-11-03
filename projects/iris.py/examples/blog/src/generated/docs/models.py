"""Row shapes returned by the generated client.

Fields are optional because a `select` projection may omit any column. Carrier
values (`bytes`, `vector`, `file`) surface as the tagged wire mapping the
binding returns.
"""

from __future__ import annotations

from typing import Any, Mapping, NotRequired, TypedDict


class Doc(TypedDict):
    doc_id: NotRequired[int]
    label: NotRequired[str]
    embedding: NotRequired[Mapping[str, Any]]
