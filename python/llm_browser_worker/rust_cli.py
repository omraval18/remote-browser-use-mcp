from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def main() -> None:
    _exec_rust_binary("browser-use-cli", sys.argv[1:])


def tui_main() -> None:
    _exec_rust_binary("browser-use-tui", sys.argv[1:])


def _exec_rust_binary(package: str, args: list[str]) -> None:
    repo_root = Path(__file__).resolve().parents[2]
    if (repo_root / "Cargo.toml").exists():
        os.chdir(repo_root)
        raise SystemExit(subprocess.call(["cargo", "run", "-q", "-p", package, "--", *args]))
    binary = repo_root / "target" / "debug" / package
    if binary.exists():
        os.execv(str(binary), [str(binary), *args])
    raise SystemExit(f"could not find Rust binary for {package}")
