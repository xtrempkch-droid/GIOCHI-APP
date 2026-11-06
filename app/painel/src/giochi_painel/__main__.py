"""CLI do painel: `python -m giochi_painel [--porta N] [--sem-navegador]`."""

from __future__ import annotations

import argparse
import sys

from . import __version__, acoes, servidor


def _saida_utf8() -> None:
    """Ver `_saida_utf8` do forge: no Windows o pipe usa cp1252, não UTF-8."""
    for fluxo in (sys.stdout, sys.stderr):
        if hasattr(fluxo, "reconfigure"):
            try:
                fluxo.reconfigure(encoding="utf-8", errors="replace")
            except (ValueError, OSError):
                pass


def main(argv: list[str] | None = None) -> int:
    _saida_utf8()
    ap = argparse.ArgumentParser(
        prog="giochi-painel",
        description="Interface web local da loja e do forge. Só escuta em 127.0.0.1.",
    )
    ap.add_argument("--porta", type=int, default=8710, help="porta (padrão: 8710)")
    ap.add_argument(
        "--sem-navegador",
        action="store_true",
        help="não tenta abrir o navegador (útil em SSH e no CI)",
    )
    ap.add_argument("--versao", action="version", version=f"giochi-painel {__version__}")
    args = ap.parse_args(argv)

    try:
        return servidor.servir(porta=args.porta, abrir=not args.sem_navegador)
    except acoes.ErroDeAcao as exc:
        print(f"giochi-painel: {exc}", file=sys.stderr)
        return 2
    except OSError as exc:
        print(f"giochi-painel: não consegui abrir a porta {args.porta}: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
