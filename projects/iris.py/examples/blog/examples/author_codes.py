"""下游验收示例：生成客户端在典型错误路径上稳定暴露作者码。

每个用例只断言作者面 `code` / `kind`，并检查 `diagnostic_code` 与两者分离
（内部细节不得冒充作者码）。期望值来自生成客户端的公开契约，不读取宿主内部表。

运行：`.venv-python/Scripts/python.exe projects/iris.py/examples/blog/examples/author_codes.py`
（需要已安装的 `iris_pyo3`，模块路径由本文件自行注入。）
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

PROJECT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_DIR / "src"))

from generated.docs import IrisError as DocsIrisError  # noqa: E402
from generated.docs import create_client as create_docs_client  # noqa: E402
from generated.iris import IrisError, create_client  # noqa: E402

CASES: list[tuple[str, str]] = []


def expect(label: str, code: str, kind: str, action) -> None:
    try:
        action()
    except (IrisError, DocsIrisError) as error:
        assert error.code == code, f"{label}: code {error.code} != {code}"
        assert error.kind == kind, f"{label}: kind {error.kind} != {kind}"
        if error.diagnostic_code is not None:
            assert error.diagnostic_code != error.code, f"{label}: diagnostic_code leaked the author code"
        CASES.append((label, error.code))
        return
    raise AssertionError(f"{label}: expected {code}, nothing was raised")


def main() -> int:
    db = create_client("sqlite::memory:")
    db.tables.user.create(data={"user_id": "u1", "user_name": "alice", "active": True})

    expect(
        "not_found",
        "IRIS-NOT-FOUND",
        "not_found",
        lambda: db.tables.user.update(where={"user_id": "missing"}, data={"user_name": {"set": "x"}}),
    )
    expect(
        "invalid_argument",
        "IRIS-INVALID-ARGUMENT",
        "invalid_argument",
        lambda: db.tables.user.find_many(where={"not_a_field": "x"}),
    )
    expect(
        "unique_conflict",
        "IRIS-UNIQUE-CONFLICT",
        "unique_conflict",
        lambda: db.tables.user.create(data={"user_id": "u1", "user_name": "dup", "active": True}),
    )
    with tempfile.TemporaryDirectory() as directory:
        docs = create_docs_client(f"file:{Path(directory) / 'docs.yydb'}")
        expect(
            "vector_dimension",
            "IRIS-VECTOR-DIMENSION",
            "vector_dimension_mismatch",
            lambda: docs.tables.doc.find_near(field="embedding", vector=[1.0, 0.0], metric="cosine", threshold=0.5),
        )

    # 未能在此处触发、因此不计入覆盖的码：
    # - `IRIS-SCHEMA-MISMATCH` 当前由文档客户端在 `sqlite::memory:` 上落到 `IRIS-INTERNAL`，
    #   与生成客户端契约不符，先用 `test_author_surface` 的既有断言跟踪，不在此复制错误期望。
    # - `IRIS-CONNECTION-FAILED` 需要真实远端节点（本仓 `e2e/yyds/reconnect.ts` 覆盖）。
    # - `IRIS-TRANSACTION-FAILED` 需要已失效的事务会话，生成客户端未公开该入口。
    # - `IRIS-INTERNAL` 只在宿主缺陷时出现，示例不应构造。
    for label, code in CASES:
        print(f"PASS {label}: {code}")
    print(f"author codes covered {len(CASES)}/8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
