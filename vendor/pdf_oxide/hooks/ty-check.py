"""
ty type checker wrapper.
Only runs if .venv exists, otherwise skips (for Rust-only changes).
"""

import subprocess
import sys
from pathlib import Path


PRJ_DIR = Path(__file__).resolve().parent.parent


def main():
    venv_dir = PRJ_DIR / ".venv"

    if not venv_dir.exists():
        print(".venv not found, skipping ty type check (Rust-only mode)")
        return 0

    print("Running ty type checker...")
    try:
        result = subprocess.run(
            ["ty", "check", "."],
            cwd=PRJ_DIR,
            capture_output=False,
        )
        return result.returncode
    except FileNotFoundError:
        print("ty command not found, please install it with 'uv tool install ty'", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
