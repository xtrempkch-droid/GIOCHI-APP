"""giochi_store — catalogo e instalacao da Loja, do lado do PC (v1.2)."""

from .destino import (
    Destino,
    bytes_legivel,
    descobrir,
    espaco_livre,
    games_de,
    resumo,
    todos,
)
from .catalogo import (
    ARCH_OK,
    ORIGEM_LOJA,
    ORIGEM_SIDELOAD,
    RUNTIMES_OK,
    Pacote,
    catalogo,
    ler_metadata,
    novidades,
    sha256_de,
    tem_assinatura,
    validar,
    varrer,
)
from . import sideload

__all__ = [
    "Destino",
    "bytes_legivel",
    "descobrir",
    "espaco_livre",
    "games_de",
    "resumo",
    "todos",
    "ARCH_OK",
    "ORIGEM_LOJA",
    "ORIGEM_SIDELOAD",
    "RUNTIMES_OK",
    "Pacote",
    "catalogo",
    "ler_metadata",
    "novidades",
    "sha256_de",
    "sideload",
    "tem_assinatura",
    "validar",
    "varrer",
]
