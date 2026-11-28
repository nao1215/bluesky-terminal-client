#!/usr/bin/env python3
"""Measure the client's first post using the shared PTY probe."""
from pathlib import Path
import runpy
import sys

here = Path(__file__).resolve().parent
sys.argv[1:1] = ["first-post", str(here / "mock.py")]
runpy.run_path(str(here.parents[2] / "scripts/bench/runner.py"), run_name="__main__")
