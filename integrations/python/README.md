# Oicana Python Integration

> Contributor documentation for building and publishing the Python integration.
> Using Oicana in a Python project? Start at [`oicana-python/README.md`](oicana-python/README.md)
> or the [Python getting started guide](https://oicana.com/docs/getting-started/4-6-python/).

Two packages: `oicana-native` (PyO3 Rust extension) + `oicana` (Python wrapper).

## Structure

```
python/
├── pyproject.toml              # uv workspace config
├── oicana-python-native/       # Native extension (Rust)
│   ├── src/lib.rs              # PyO3 lib
│   └── pyproject.toml          # maturin, abi3-py39
└── oicana-python/              # Wrapper
    └── src/oicana/
        ├── template.py         # Template class
        └── types.py            # Types
```

## Development

Requires [uv](https://github.com/astral-sh/uv):

```bash
# Install uv once
curl -LsSf https://astral.sh/uv/install.sh | sh

# From integrations/python/
uv sync                                         # Install both packages + dev deps
uv run pytest oicana-python/tests/ -v           # Run tests
uv run mypy ./*/src/oicana                      # Type check
uv run ruff check ./*/src/                      # Lint
```

Workspace auto-links local package. Published version uses PyPI dependencies.

## Publishing

```bash
cd oicana-python-native && maturin publish      # abi3 wheel for the local host
cd ../oicana-python && uv build && uv publish   # Python package
```
