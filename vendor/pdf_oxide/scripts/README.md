# PDF Library Scripts

Utility scripts for development, testing, and analysis.

## Cleanup

### `cleanup_for_release.sh`
Cleans repository for public release by removing development artifacts.

```bash
./scripts/cleanup_for_release.sh
```

**What it does:**
- Removes development markdown files from root
- Removes Python comparison scripts from root
- Removes log and JSON analysis files
- Cleans docs/development/sessions/
- Cleans docs/issues/
- Prunes docs/quality/ to keep only summaries

## Download Scripts

### `download/`
Scripts for downloading test PDFs from various sources.

Located in `../test_datasets/download.py`

## Analysis Scripts

Utility scripts for analyzing PDF content and quality.

### Usage
Most scripts are Python-based:
```bash
python scripts/analyze_something.py input.pdf
```

## Python type stubs

Type stub files (`.pyi`) are generated from the Rust PyO3 source with [Rylai](https://github.com/monchin/Rylai) (no compilation). Use this after changing the Python API in `src/python.rs`:

```bash
# From project root (runs uvx rylai -o python/pdf_oxide/)
pdm run stub_gen
```

Config is in `rylai.toml` at the crate root. Output is written under `python/pdf_oxide/` and bundled into the wheel by maturin.

## Benchmark Scripts

Performance benchmarking and comparison utilities.

### Running Benchmarks
```bash
cargo bench
```

For detailed benchmarking, see `../test_datasets/` and comparison scripts.

## Running Python Scripts
```bash
# Install corresponding python dependencies
uv sync --group {group_name} # Refer to [dependency-groups] in pyproject.toml
uv run scripts/{script_name}.py {args}
```


## Development

These scripts are for development purposes and are not part of the public API.

## Contributing

When adding new scripts:
1. Add clear documentation in this README
2. Include usage examples
3. Add error handling
4. Follow existing patterns

See [../CONTRIBUTING.md](../CONTRIBUTING.md) for general guidelines.

## License

All scripts are licensed under MIT OR Apache-2.0, same as the main library.
