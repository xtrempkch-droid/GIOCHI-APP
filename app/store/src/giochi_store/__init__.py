"""giochi_store — catalogo e instalacao da Loja, do lado do PC (v1.2)."""

from .destino import Destino, descobrir, games_de, resumo, todos
from .catalogo import (
    ARCH_OK,
    RUNTIMES_OK,
    Pacote,
    catalogo,
    ler_metadata,
    novidades,
    sha256_de,
    validar,
    varrer,
)

__all__ = [
    "Destino",
    "descobrir",
    "games_de",
    "resumo",
    "todos",
    "ARCH_OK",
    "RUNTIMES_OK",
    "Pacote",
    "catalogo",
    "ler_metadata",
    "novidades",
    "sha256_de",
    "validar",
    "varrer",
]
