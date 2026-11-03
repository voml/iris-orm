"""Row models returned by the generated client.

Every read returns the entity model, so authors reach columns as attributes
(`row.article_id`). A field is only set when the row carries it, because a
`select` projection may omit any column. Carrier values (`bytes`, `vector`,
`file`) surface as the tagged wire mapping the binding returns.
"""

from __future__ import annotations

from typing import Any, Mapping


class Doc:
    """Row model of `Doc`."""

    doc_id: int
    label: str
    embedding: Mapping[str, Any]

    def __init__(self, row: Mapping[str, Any]) -> None:
        if "doc_id" in row:
            self.doc_id = row["doc_id"]
        if "label" in row:
            self.label = row["label"]
        if "embedding" in row:
            self.embedding = row["embedding"]
