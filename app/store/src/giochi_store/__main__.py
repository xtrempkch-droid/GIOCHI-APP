"""Ponto de entrada: `python3 -m giochi_store ...` (com apps/store no PYTHONPATH)."""

import sys

from .cli import main

if __name__ == "__main__":
    sys.exit(main())
