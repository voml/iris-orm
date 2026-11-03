# `iris` (Python)

Runtime package for Iris ORM for Python, built with `maturin` from the
`iris-pyo3` crate.

The public surface is intentionally tiny:

```python
import iris

iris.version()           # Rust facade version
iris.contract_version()  # accepted contract envelope format
iris.IrisError           # error type raised by generated clients
```

Data access comes from a **generated client**, one per project, produced from
the project's VOS schema:

```python
from generated.iris import create_client

db = create_client("sqlite:./app.sqlite")
db.tables.User.find_many(where={"user_name": {"contains": "ali"}})
with db.transaction() as tx:
    tx.tables.User.create(data={"user_name": "ali"})
```

Session plumbing, DSN parsing and adapter selection live in `iris._runtime` and
are not part of the authoring surface. There is no query DSL, no SQL entry and
no `.iris` parsing in this package.

## Development

```bash
python -m venv .venv
maturin develop            # run next to pyproject.toml
python -m unittest discover -s tests
python examples/embedded/main.py
```

`IrisError` is raised when the native module is missing.
