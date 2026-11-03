"""Generated Python client against a live yyds SQL gateway.

Mirrors `suite.ts` case for case: same schema, same rows, same assertions.
The printed `code` values are compared byte-for-byte with the TypeScript run.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / ".generated" / "py"))

from generated.iris import IrisError, create_client  # noqa: E402

DSN = os.environ.get("IRIS_E2E_DSN")
if not DSN:
    raise SystemExit("IRIS_E2E_DSN is required")

db = create_client(DSN)
codes: dict[str, str | None] = {}

SEED = [
    {"article_id": "a1", "slug": "alpha-one", "body": "needle in the alpha body"},
    {"article_id": "a2", "slug": "beta-two", "body": "beta body"},
    {"article_id": "a3", "slug": "gamma-three", "body": "gamma needle tail"},
]

for row in SEED:
    db.tables.Article.create(data=row)

unique = db.tables.Article.find_unique(where={"article_id": "a1"})
assert unique is not None and unique["slug"] == "alpha-one"

contains = db.tables.Article.find_many(where={"body": {"contains": "needle"}})
assert sorted(row["article_id"] for row in contains) == ["a1", "a3"]

startsWith = db.tables.Article.find_many(where={"slug": {"starts_with": "alpha"}})
assert [row["article_id"] for row in startsWith] == ["a1"]

endsWith = db.tables.Article.find_many(where={"slug": {"ends_with": "-three"}})
assert [row["article_id"] for row in endsWith] == ["a3"]

try:
    db.tables.Doc.create(data={"doc_id": "d1", "label": "probe", "embedding": [1.0, 0.0, 0.0]})
    db.tables.Doc.find_near(
        field="embedding",
        vector=[1.0, 0.0, 0.0],
        metric="cosine",
        threshold=0.1,
    )
    codes["vector_payload"] = None
except IrisError as error:
    codes["vector_payload"] = error.code

with db.transaction() as tx:
    tx.tables.Article.create(data={"article_id": "a4", "slug": "delta-four", "body": "committed row"})
assert db.tables.Article.find_unique(where={"article_id": "a4"}) is not None

try:
    with db.transaction() as tx:
        tx.tables.Article.create(data={"article_id": "a5", "slug": "epsilon-five", "body": "rolled back"})
        raise RuntimeError("rollback probe")
except RuntimeError:
    pass
assert db.tables.Article.find_unique(where={"article_id": "a5"}) is None

try:
    db.tables.Article.create(data={"article_id": "a6", "slug": "alpha-one", "body": "duplicate"})
    codes["unique_conflict"] = None
except IrisError as error:
    expected = os.environ.get("IRIS_E2E_UNIQUE_DIAGNOSTIC")
    diagnostic = error.diagnostic_code or ""
    assert expected is None or expected in diagnostic, f"unique conflict diagnostic {diagnostic} must contain {expected}"
    codes["unique_conflict"] = error.code
    codes["unique_conflict_diagnostic"] = "present"

try:
    db.tables.Article.update(where={"article_id": "missing"}, data={"body": "no match"})
    codes["not_found"] = None
except IrisError as error:
    codes["not_found"] = error.code

print(f"RESULT {json.dumps({'codes': codes}, sort_keys=True)}")


