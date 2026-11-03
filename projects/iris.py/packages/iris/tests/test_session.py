"""Session tests for the Iris Python binding.

Covers the reference (`memory`) profile, SQLite query/write/transaction and
close-reopen, YYDB `.yydb` / `.yydx` layouts with `vector` and `file` payload
types, and the structured error envelope down to the diagnostic code.
"""

from __future__ import annotations

import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

try:
    import iris
except ImportError:  # running the suite from a source checkout
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))
    import iris

from iris import IrisError, _runtime

try:
    import iris_pyo3  # noqa: F401
except ImportError:
    NATIVE_AVAILABLE = False
else:
    NATIVE_AVAILABLE = True
NATIVE_REASON = (
    "the `iris_pyo3` native module is not built. Install the wheel or run "
    "`maturin develop` in `projects/iris.py/packages/iris` first"
)

USER_SCHEMA = """
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"""

DOC_SCHEMA = """
table Doc {
    @@doc_id: i64,
    label: utf8,
    embedding: vector<3>,
}
"""

ASSET_SCHEMA = """
table Asset {
    @@asset_id: i64,
    label: utf8,
    payload: file,
}
"""

VECTOR_THREE = "0000803f0000004000004040"


def _descriptor(content: bytes, name: str, media_type: str | None = None) -> dict[str, object]:
    digest = hashlib.sha256(content).hexdigest()
    manifest = hashlib.sha256(content + b"manifest").hexdigest()
    payload: dict[str, object] = {
        "$yydb": "file",
        "digest": digest,
        "octets": len(content),
        "name": name,
        "manifest": {"hash": manifest, "size": len(content)},
    }
    if media_type is not None:
        payload["mediaType"] = media_type
    return payload


def _error_code(result: dict[str, object]) -> str:
    error = result["error"]
    assert isinstance(error, dict), result
    return str(error["code"])


@unittest.skipUnless(NATIVE_AVAILABLE, NATIVE_REASON)
class MemoryProfileTests(unittest.TestCase):
    def test_query_on_unseeded_reference_profile_reports_code(self) -> None:
        session = _runtime.openMemory()
        result = session.query("User.collect()")
        self.assertFalse(result["ok"])
        self.assertEqual(result["rows"], [])
        self.assertEqual(_error_code(result), "IRIS-EXEC-FAILED")

    def test_reference_profile_rejects_writes_at_plan_time(self) -> None:
        session = _runtime.openMemory()
        result = session.execute('User::insert({ user_id: "u1", user_name: "alice", active: true })')
        self.assertFalse(result["ok"])
        self.assertEqual(_error_code(result), "IRIS-PLAN-REJECTED")

    def test_transactions_are_unsupported_on_the_reference_profile(self) -> None:
        session = _runtime.openMemory()
        self.assertEqual(_error_code(session.begin()), "IRIS-TRANSACTION-UNSUPPORTED")

    def test_closed_session_returns_a_session_closed_code(self) -> None:
        session = _runtime.openMemory()
        session.close()
        result = session.query("User.collect()")
        self.assertFalse(result["ok"])
        self.assertEqual(_error_code(result), "IRIS-SESSION-CLOSED")


@unittest.skipUnless(NATIVE_AVAILABLE, NATIVE_REASON)
class SqliteProfileTests(unittest.TestCase):
    def _open(self) -> "_runtime.Session":
        session = _runtime.openSqlite()
        pushed = session.managedPush(USER_SCHEMA)
        self.assertTrue(pushed["ok"], pushed)
        return session

    def test_write_query_and_predicate_semantics(self) -> None:
        session = self._open()
        created = session.execute(
            'User::insert({ user_id: "u1", user_name: "alice", active: true })'
        )
        self.assertTrue(created["ok"], created)
        session.execute('User::insert({ user_id: "u2", user_name: "Alice", active: false })')

        all_rows = session.query("User.collect()")
        self.assertTrue(all_rows["ok"], all_rows)
        self.assertEqual(len(all_rows["rows"]), 2)
        names = sorted(row["user_name"] for row in all_rows["rows"])
        self.assertEqual(names, ["Alice", "alice"])

        exact = session.query('User.filter(x => x.user_name == "alice").collect()')
        self.assertEqual([row["user_id"] for row in exact["rows"]], ["u1"])
        case_sensitive = session.query('User.filter(x => x.user_name == "ALICE").collect()')
        self.assertEqual(case_sensitive["rows"], [])

        active = session.query("User.filter(x => x.active).collect()")
        self.assertEqual([row["user_id"] for row in active["rows"]], ["u1"])

    def test_transaction_rollback_and_commit(self) -> None:
        session = self._open()
        self.assertTrue(session.begin()["ok"])
        session.execute('User::insert({ user_id: "u1", user_name: "alice", active: true })')
        self.assertTrue(session.rollback()["ok"])
        self.assertEqual(session.query("User.collect()")["rows"], [])

        self.assertTrue(session.begin()["ok"])
        session.execute('User::insert({ user_id: "u2", user_name: "bob", active: true })')
        self.assertTrue(session.commit()["ok"])
        self.assertEqual(len(session.query("User.collect()")["rows"]), 1)

    def test_close_and_reopen_the_same_sqlite_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = str(Path(directory) / "app.sqlite")
            session = _runtime.openSqlite(path)
            session.managedPush(USER_SCHEMA)
            session.execute('User::insert({ user_id: "u1", user_name: "alice", active: true })')
            session.close()

            reopened = _runtime.openSqlite(path)
            rows = reopened.query('User.filter(x => x.user_id == "u1").collect()')
            self.assertTrue(rows["ok"], rows)
            self.assertEqual(len(rows["rows"]), 1)
            self.assertEqual(rows["rows"][0]["user_name"], "alice")
            reopened.close()

    def test_sqlite_identity_is_exposed(self) -> None:
        session = self._open()
        self.assertTrue(session.sqliteEngineVersion().strip())
        self.assertTrue(session.sqliteSourceId().strip())

    def test_unknown_source_keeps_a_diagnostic_code(self) -> None:
        session = self._open()
        result = session.query("User.filter(x => ")
        self.assertFalse(result["ok"])
        self.assertTrue(_error_code(result))


@unittest.skipUnless(NATIVE_AVAILABLE, NATIVE_REASON)
class ErrorEnvelopeTests(unittest.TestCase):
    def test_bind_failure_is_a_param_encoding_error(self) -> None:
        session = _runtime.openSqlite()
        result = session.query("User.filter(x => x.user_id == $id).collect()", "{not json")
        self.assertFalse(result["ok"])
        self.assertEqual(_error_code(result), "IRIS-PARAM-ENCODING")

    def test_invalid_operation_payload_is_an_operation_error(self) -> None:
        session = _runtime.openSqlite()
        result = session.executeOperation("{")
        self.assertFalse(result["ok"])
        self.assertEqual(_error_code(result), "IRIS-OPERATION-INVALID")

    def test_open_failure_raises_iris_error_with_code(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            blocker = Path(directory) / "blocked"
            blocker.write_text("not a database", encoding="utf-8")
            with self.assertRaises(IrisError) as caught:
                _runtime.openYydb(str(blocker / "app.yydb"))
            self.assertTrue(caught.exception.code.startswith("IRIS-"))


@unittest.skipUnless(NATIVE_AVAILABLE, NATIVE_REASON)
class YydbProfileTests(unittest.TestCase):
    def _layout_path(self, directory: str, layout: str) -> str:
        return str(Path(directory) / "data" / f"app.{layout}")

    def test_vector_payload_round_trips_in_yydb_and_yydx(self) -> None:
        for layout in ("yydb", "yydx"):
            with self.subTest(layout=layout), tempfile.TemporaryDirectory() as directory:
                session = _runtime.openYydb(self._layout_path(directory, layout))
                pushed = session.managedPush(DOC_SCHEMA)
                self.assertTrue(pushed["ok"], pushed)

                parameters = json.dumps({"doc_id": 1, "label": "alpha", "embedding": {"$yydb": "vector", "dimension": 3, "components": VECTOR_THREE}})
                inserted = session.execute(
                    "Doc::insert({ doc_id: $doc_id, label: $label, embedding: $embedding })",
                    parameters,
                )
                self.assertTrue(inserted["ok"], inserted)

                rows = session.query("Doc.collect()")
                self.assertTrue(rows["ok"], rows)
                self.assertEqual(len(rows["rows"]), 1)
                embedding = rows["rows"][0]["embedding"]
                self.assertEqual(embedding["$yydb"], "vector")
                self.assertEqual(embedding["dimension"], 3)
                self.assertEqual(embedding["components"], VECTOR_THREE)
                session.close()

                reopened = _runtime.openYydb(self._layout_path(directory, layout))
                rows = reopened.query("Doc.collect()")
                self.assertEqual(len(rows["rows"]), 1)
                reopened.close()

    def test_file_descriptor_round_trips_and_filters_by_digest(self) -> None:
        alpha = _descriptor(b"alpha-content", "alpha.bin", "application/octet-stream")
        beta = _descriptor(b"beta-content", "beta.bin", "application/octet-stream")
        relabeled = _descriptor(b"alpha-content", "unrelated.dat", "application/x-other")

        for layout in ("yydb", "yydx"):
            with self.subTest(layout=layout), tempfile.TemporaryDirectory() as directory:
                session = _runtime.openYydb(self._layout_path(directory, layout))
                pushed = session.managedPush(ASSET_SCHEMA)
                self.assertTrue(pushed["ok"], pushed)

                for asset_id, label, payload in ((1, "alpha", alpha), (2, "beta", beta)):
                    parameters = json.dumps({"asset_id": asset_id, "label": label, "payload": payload})
                    inserted = session.execute(
                        "Asset::insert({ asset_id: $asset_id, label: $label, payload: $payload })",
                        parameters,
                    )
                    self.assertTrue(inserted["ok"], inserted)

                rows = session.query("Asset.collect()")
                self.assertEqual(len(rows["rows"]), 2)
                first = rows["rows"][0]
                self.assertEqual(first["payload"]["$yydb"], "file")
                self.assertEqual(first["payload"]["digest"], alpha["digest"])
                self.assertEqual(first["payload"]["name"], "alpha.bin")

                probe = session.query(
                    "Asset.filter(x => x.payload == $payload).collect()",
                    json.dumps({"payload": relabeled}),
                )
                self.assertTrue(probe["ok"], probe)
                self.assertEqual([row["asset_id"] for row in probe["rows"]], [1])
                session.close()
