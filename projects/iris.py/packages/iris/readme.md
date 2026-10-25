# `iris` (Python)

Python binding skeleton for Iris ORM, built with `maturin` from the
`iris-pyo3` crate.

It currently provides version and capability probing only:

```python
import iris

iris.version()           # Rust facade version
iris.contract_version()  # accepted contract envelope format
iris.capabilities()      # capability ids exposed by this binding
```

There is no `Database` entry, no query DSL, no SQL entry, and no `.iris`
parsing in this package. Schema authoring, generation, and runtime data access
remain in the Rust and Node surfaces.

## Development

```bash
python -m venv .venv
maturin develop            # run next to pyproject.toml
python -m unittest discover -s tests
python examples/embedded/main.py
```

`IrisError` is raised when the native module is missing.
