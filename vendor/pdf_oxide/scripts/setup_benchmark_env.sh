#!/bin/bash
# Setup environment for benchmarking

echo "Setting up benchmark environment..."
echo

# Create virtual environment if it doesn't exist
if [ ! -d "benchmark_venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv benchmark_venv
fi

# Activate virtual environment
echo "Activating virtual environment..."
source benchmark_venv/bin/activate

# Upgrade pip
echo "Upgrading pip..."
pip install --upgrade pip

# Install benchmark dependencies
echo "Installing PDF libraries..."
pip install -r scripts/benchmark_requirements.txt

echo
echo "Installing our Rust library..."
maturin develop --release

echo
echo "Setup complete!"
echo
echo "To run benchmark:"
echo "  source benchmark_venv/bin/activate"
echo "  python3 scripts/benchmark_all_libraries.py --limit 10  # Test with 10 PDFs"
echo "  python3 scripts/benchmark_all_libraries.py              # Full benchmark (362 PDFs)"
