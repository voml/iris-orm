# `iris` (Python)

Runtime package for Iris ORM for Python, built with `maturin` from the
`iris-pyo3` crate.

The public surface is intentionally tiny:

```python
import iris

iris.version()           # Rust facade version
iris.contract_version()  # accepted contract envelope format
iris.IrisError           # error type raised by this runtime package
```

Data access comes from a **generated client**, one per project, produced from
the project's VOS schema:

```python
from generated.iris import IrisError, create_client

db = create_client("sqlite:./app.sqlite")
db.tables.user.find_many(where={"user_name": {"contains": "ali"}})
with db.transaction() as tx:
    tx.tables.user.create(data={"user_name": "ali"})
```

Connection plumbing, DSN parsing and adapter selection stay behind the generated
client's `_internal` modules and the Rust session core, and are not part of the
authoring surface. Generated clients define their own `IrisError`, so a
generated package named `iris` under `src/generated/` never shadows this
runtime package. There is no query DSL, no SQL entry and no `.iris` parsing in
this package.

## Development

```bash
python -m venv .venv
maturin develop            # run next to pyproject.toml
python -m unittest discover -s tests
python examples/embedded/main.py
```

`IrisError` is raised when the native module is missing.
