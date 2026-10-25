//! Python (`pyo3`) binding surface for Iris ORM.
//!
//! The binding is a thin wrapper over the Rust semantic core. It exposes the
//! facade version, the accepted contract envelope format, and a read-only
//! capability list. It does not parse `.iris` sources, does not accept SQL, and
//! does not define a second query DSL.
//!
//! Every export here is infallible. A fallible entry, when one is added, must
//! map `iris_types::Error` through `Display` into a Python exception so Rust
//! debug formatting never reaches Python.

#![deny(clippy::all)]

use pyo3::prelude::*;

/// Capability ids exported by this binding.
const CAPABILITIES: [&str; 3] = ["version", "contract_version", "capabilities"];

/// Iris facade version (`iris::version()`).
#[pyfunction]
fn version() -> &'static str {
    iris::version()
}

/// Contract envelope format accepted by Iris (`iris_types::ACCEPTED_CONTRACT_FORMAT`).
#[pyfunction]
fn contract_version() -> &'static str {
    iris_types::ACCEPTED_CONTRACT_FORMAT
}

/// Read-only capability ids exposed by this binding.
#[pyfunction]
fn capabilities() -> Vec<&'static str> {
    CAPABILITIES.to_vec()
}

/// Native module entry point (`import iris_pyo3`).
#[pymodule]
fn iris_pyo3(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(version, module)?)?;
    module.add_function(wrap_pyfunction!(contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(capabilities, module)?)?;
    Ok(())
}
