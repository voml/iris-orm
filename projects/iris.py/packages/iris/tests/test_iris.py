"""Smoke tests for the Iris Python binding skeleton."""

from __future__ import annotations

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


class PackageTests(unittest.TestCase):
    def test_public_names_are_exported(self) -> None:
        self.assertEqual(sorted(iris.__all__), ["IrisError", "capabilities", "contract_version", "version"])
        for name in iris.__all__:
            self.assertTrue(hasattr(iris, name), name)

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

    def test_capabilities_are_a_read_only_list(self) -> None:
        self.assertEqual(iris.capabilities(), ["version", "contract_version", "capabilities"])


if __name__ == "__main__":
    unittest.main()
