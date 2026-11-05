# Python 侧脚本

安装态验收不依赖内核 `cargo run --bin`。`generate.py` 只读 schema 文件并调用
已安装 wheel 的 `iris.generate` 原语，解析与产出都在 `iris` 门面的 `generator`
模块里，本仓不保留第二套生成实现。

```bash
# 仓库根，先构建并安装 wheel
python projects/iris.py/scripts/generate.py \
    projects/iris.py/examples/blog/schemas/blog.iris \
    projects/iris.py/examples/blog/src/generated/iris
python projects/iris.py/scripts/generate.py \
    projects/iris.py/examples/blog/schemas/docs.iris \
    projects/iris.py/examples/blog/src/generated/docs
```

博客示例的生成物已提交在 `examples/blog/src/generated/`；CI 用提交的产物跑安装态
测试，并对重新生成的结果执行 `git diff --exit-code`。更新 schema 后按上面的命令
刷新，再跑：

```bash
cd projects/iris.py/examples/blog
PYTHONPATH=src python -m unittest discover -s tests -v
```
