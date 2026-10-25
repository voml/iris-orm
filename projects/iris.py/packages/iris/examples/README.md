# Examples

`embedded/main.py` probes the installed binding and prints its version, the
accepted contract envelope, and the capability ids.

Install the wheel (or run `maturin develop` next to `pyproject.toml`), then:

```bash
python examples/embedded/main.py
```

The example does not touch a database. Schema authoring, generation, and
runtime data access are not bound yet, so no CRUD example exists.
