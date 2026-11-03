# Examples

`packages/iris/examples/embedded/main.py` probes the installed binding and
prints its version and the accepted contract envelope.

`examples/blog` is a generated project: `schemas/*.iris` are compiled into
`src/generated/` and the tests exercise the generated authoring surface
(`create_client`, `client.tables.<entity>`, `client.transaction`) against an
installed wheel.

```bash
# from the repository root, after building the wheel and installing it
cargo run -q -p iris-generator --bin iris-python-generate -- \
    projects/iris.py/examples/blog/schemas/blog.iris \
    projects/iris.py/examples/blog/src/generated/iris
cargo run -q -p iris-generator --bin iris-python-generate -- \
    projects/iris.py/examples/blog/schemas/docs.iris \
    projects/iris.py/examples/blog/src/generated/docs

cd projects/iris.py/examples/blog
PYTHONPATH=src python -m unittest discover -s tests -v
```
