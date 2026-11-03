"""Install-state checks for a generated Iris Python client.

Run these against an installed wheel (not a source checkout). The clients in
``src/generated`` are produced from ``schemas`` by ``iris-python-generate``:
``generated.iris`` for the SQLite CRUD surface and ``generated.docs`` for the
YYDB vector surface.
"""

from __future__ import annotations

import importlib
import importlib.machinery
import sys
import tempfile
import unittest
from pathlib import Path

PROJECT_DIR = Path(__file__).resolve().parents[1]
SRC_DIR = PROJECT_DIR / "src"
sys.path.insert(0, str(SRC_DIR))

import iris_pyo3  # noqa: E402
from generated.docs import IrisError as DocsIrisError  # noqa: E402
from generated.docs import create_client as create_docs_client  # noqa: E402
from generated.iris import IrisError, create_client  # noqa: E402


def native_extension_path() -> Path:
    """Path of the compiled ``iris_pyo3`` extension module."""
    return Path(importlib.import_module("iris_pyo3.iris_pyo3").__file__)


class InstalledWheelTest(unittest.TestCase):
    def test_native_module_is_a_compiled_extension(self) -> None:
        path = native_extension_path()
        self.assertTrue(
            path.name.endswith(tuple(importlib.machinery.EXTENSION_SUFFIXES)),
            f"{path} is not a compiled extension",
        )

    def test_generated_package_is_project_local(self) -> None:
        module = importlib.import_module("generated.iris")
        self.assertEqual(Path(module.__file__).resolve().parent, SRC_DIR / "generated" / "iris")

    def test_compiled_extension_exposes_the_client_entry(self) -> None:
        self.assertTrue(callable(iris_pyo3.open_dsn))
        self.assertIn("open_dsn", iris_pyo3.capabilities())


class GeneratedCrudTest(unittest.TestCase):
    def setUp(self) -> None:
        self.db = create_client("sqlite::memory:")

    def test_create_read_update_delete(self) -> None:
        created = self.db.tables.User.create(
            data={"user_id": "u1", "user_name": "alice", "active": True}
        )
        self.assertEqual("alice", created["user_name"])

        self.assertEqual(["u1"], [row["user_id"] for row in self.db.tables.User.find_many()])
        self.assertEqual(
            "alice",
            self.db.tables.User.find_unique(where={"user_id": "u1"})["user_name"],
        )
        self.assertEqual(
            "alice",
            self.db.tables.User.find_first(where={"user_name": {"contains": "lic"}})["user_name"],
        )

        updated = self.db.tables.User.update(
            where={"user_id": "u1"},
            data={"user_name": {"set": "grace"}},
        )
        self.assertEqual("grace", updated["user_name"])

        self.db.tables.User.delete(where={"user_id": "u1"})
        self.assertEqual([], self.db.tables.User.find_many())
        self.assertIsNone(self.db.tables.User.find_unique(where={"user_id": "missing"}))

    def test_update_and_delete_missing_row_are_not_found(self) -> None:
        with self.assertRaises(IrisError) as caught:
            self.db.tables.User.update(
                where={"user_id": "missing"},
                data={"user_name": {"set": "nobody"}},
            )
        self.assertEqual("IRIS-NOT-FOUND", caught.exception.code)
        self.assertEqual("not_found", caught.exception.kind)

        with self.assertRaises(IrisError) as caught:
            self.db.tables.User.delete(where={"user_id": "missing"})
        self.assertEqual("IRIS-NOT-FOUND", caught.exception.code)
        self.assertEqual("not_found", caught.exception.kind)

    def test_transaction_commits_and_rolls_back(self) -> None:
        with self.db.transaction() as tx:
            tx.tables.User.create(data={"user_id": "u1", "user_name": "alice", "active": True})
        self.assertEqual(1, len(self.db.tables.User.find_many()))

        with self.assertRaises(RuntimeError):
            with self.db.transaction() as tx:
                tx.tables.User.create(data={"user_id": "u2", "user_name": "bob", "active": False})
                raise RuntimeError("rollback")
        self.assertEqual(["u1"], [row["user_id"] for row in self.db.tables.User.find_many()])


class GeneratedVectorTest(unittest.TestCase):
    def test_find_near_ranks_scanned_rows(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            db = create_docs_client(f"file:{Path(directory) / 'docs.yydb'}")
            db.tables.Doc.create(data={"doc_id": 1, "label": "east", "embedding": [1.0, 0.0, 0.0]})
            db.tables.Doc.create(data={"doc_id": 2, "label": "north", "embedding": [0.0, 1.0, 0.0]})

            rows = db.tables.Doc.find_near(
                field="embedding",
                vector=[1.0, 0.0, 0.0],
                metric="cosine",
                threshold=0.5,
                take=1,
            )
            self.assertEqual(["east"], [row["label"] for row in rows])


class GeneratedErrorTest(unittest.TestCase):
    def test_unknown_dsn_is_a_semantic_error(self) -> None:
        with self.assertRaises(IrisError) as caught:
            create_client("iri://localhost/app")
        self.assertEqual("IRIS-INVALID-ARGUMENT", caught.exception.code)


class GeneratedAuthorCodeTest(unittest.TestCase):
    """Author codes for the converged vector and filter surfaces."""

    def test_vector_dimension_mismatch_is_a_data_error(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            db = create_docs_client(f"file:{Path(directory) / 'docs.yydb'}")
            with self.assertRaises(DocsIrisError) as caught:
                db.tables.Doc.find_near(
                    field="embedding",
                    vector=[1.0, 0.0],
                    metric="cosine",
                    threshold=0.5,
                )
        self.assertEqual("IRIS-VECTOR-DIMENSION", caught.exception.code)
        self.assertEqual("IRIS-VECTOR-NEAR-DIMENSION", caught.exception.diagnostic_code)

    def test_unknown_filter_field_is_a_call_shape_error(self) -> None:
        db = create_client("sqlite::memory:")
        with self.assertRaises(IrisError) as caught:
            db.tables.User.find_many(where={"not_a_field": "x"})
        self.assertEqual("IRIS-INVALID-ARGUMENT", caught.exception.code)
        self.assertEqual("IRIS-FILTER-FIELD-UNKNOWN", caught.exception.diagnostic_code)

    def test_unsupported_filter_operator_is_a_call_shape_error(self) -> None:
        db = create_client("sqlite::memory:")
        with self.assertRaises(IrisError) as caught:
            db.tables.User.find_many(where={"active": {"gt": True}})
        self.assertEqual("IRIS-INVALID-ARGUMENT", caught.exception.code)
        self.assertEqual("IRIS-FILTER-OPERATOR-NOT-SUPPORTED", caught.exception.diagnostic_code)
