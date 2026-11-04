# `iris-pyo3`

`pyo3` binding skeleton for Iris ORM.

The crate is a thin `cdylib` over the Rust semantic core and exposes three
read-only entries: `version()`, `contract_version()`, and `capabilities()`.
It adds no query DSL, no SQL entry, and no `.iris` parsing.

Build the Python package with `maturin` from `projects/iris.py/packages/iris`.
