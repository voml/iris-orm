"""Smoke tests for the `iris` runtime package boundary."""

from __future__ import annotations

import ast
import sys
import unittest
from pathlib import Path
from unittest import mock

try:
    import iris
except ImportError:  # running the suite from a source checkout
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))
    import iris

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


INTERNAL_SESSION_NAMES = {
    "Session",
    "open_memory",
    "open_sqlite",
    "open_yydb",
    "open_postgres",
    "open_mysql",
    "open_dsn",
    "openMemory",
    "openSqlite",
    "openYydb",
    "openPostgres",
    "openMysql",
}


class PackageTests(unittest.TestCase):
    def test_public_names_are_exported(self) -> None:
        self.assertEqual(sorted(iris.__all__), ["IrisError", "contract_version", "version"])
        for name in iris.__all__:
            self.assertTrue(hasattr(iris, name), name)

    def test_session_surface_is_not_public(self) -> None:
        for name in ["Session", "capabilities", "openMemory", "openSqlite", "openYydb", "openPostgres", "openMysql"]:
            self.assertFalse(hasattr(iris, name), f"`iris.{name}` belongs to `iris._runtime`")

    def test_runtime_internals_stay_out_of_the_stub(self) -> None:
        stub = Path(iris.__file__).resolve().parent / "__init__.pyi"
        self.assertTrue(stub.is_file(), f"{stub} is missing from the installed package")
        declared = {
            node.name for node in ast.parse(stub.read_text(encoding="utf-8")).body if isinstance(node, (ast.ClassDef, ast.FunctionDef))
        }
        self.assertEqual(declared, {"IrisError", "version", "contract_version"})
        self.assertEqual(declared & INTERNAL_SESSION_NAMES, set())
        self.assertEqual(set(iris.__all__) & INTERNAL_SESSION_NAMES, set())

    def test_iris_error_is_an_exception(self) -> None:
        self.assertTrue(issubclass(iris.IrisError, Exception))
        self.assertEqual(str(iris.IrisError("boom")), "boom")


class ErrorMappingTests(unittest.TestCase):
    def test_unimportable_native_module_raises_iris_error(self) -> None:
        with mock.patch.dict(sys.modules, {"iris_pyo3": None}):
            with self.assertRaises(iris.IrisError):
                iris.version()


@unittest.skipUnless(NATIVE_AVAILABLE, NATIVE_REASON)
class NativeTests(unittest.TestCase):
    def test_version_is_a_non_empty_string(self) -> None:
        value = iris.version()
        self.assertIsInstance(value, str)
        self.assertTrue(value.strip())

    def test_contract_version_is_the_v0_envelope(self) -> None:
        self.assertEqual(iris.contract_version(), "vos/iris.contract/v0")

    def test_capabilities_are_an_internal_list(self) -> None:
        from iris import _runtime

        self.assertEqual(
            _runtime.native().capabilities(),
            [
                "version",
                "contract_version",
                "capabilities",
                "open_memory",
                "open_sqlite",
                "open_yydb",
                "open_postgres",
                "open_mysql",
                "open_dsn",
            ],
        )

    def test_iris_error_carries_a_code(self) -> None:
        self.assertEqual(iris.IrisError("boom").code, "IRIS-INTERNAL")
        self.assertEqual(
            iris.IrisError("boom", code="IRIS-INVALID-ARGUMENT", diagnostic_code="IRIS-SESSION-CLOSED").code,
            "IRIS-INVALID-ARGUMENT",
        )


if __name__ == "__main__":
    unittest.main()
