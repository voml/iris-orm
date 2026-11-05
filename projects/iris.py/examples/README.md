# Examples

`packages/iris/examples/embedded/main.py` probes the installed binding and
prints its version and the accepted contract envelope.

`examples/blog` is a generated project: `schemas/*.iris` compile into
`src/generated/` and the tests exercise the generated authoring surface
(`create_client`, `client.tables.<entity>`, `client.transaction`) against an
installed wheel.

生成 CLI 已从内核下沉；安装态 CI 使用提交的 `src/generated`。刷新生成物用已安装
wheel 的 `projects/iris.py/scripts/generate.py`：

```bash
python projects/iris.py/scripts/generate.py \
    projects/iris.py/examples/blog/schemas/blog.iris \
    projects/iris.py/examples/blog/src/generated/iris
python projects/iris.py/scripts/generate.py \
    projects/iris.py/examples/blog/schemas/docs.iris \
    projects/iris.py/examples/blog/src/generated/docs
```

```bash
cd projects/iris.py/examples/blog
PYTHONPATH=src python -m unittest discover -s tests -v
```
