#!/usr/bin/env python3
"""Runs every tools/art/models/*.py and tools/art/blocks/*.py; each writes into assets/."""
import glob
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
env = dict(os.environ, PYTHONPATH=HERE)
for kind in ("models", "blocks"):
    for script in sorted(glob.glob(os.path.join(HERE, kind, "*.py"))):
        if os.path.basename(script).startswith("_"):
            continue
        print("art:", os.path.relpath(script, HERE))
        subprocess.run([sys.executable, script], check=True, env=env)
