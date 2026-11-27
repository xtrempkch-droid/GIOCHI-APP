"""Artefatos de terceiros que **nao** sao `.giochi` — o caso do PortMaster.

## Por que isto existe, e por que e' uma lista separada

O contrato das duas chaves (`/etc/giochi/loja-publica.pem` e
`/etc/giochi/licenca-publica.pem`) cobre o que a Giochi assina. O PortMaster
nao cabe nele: baixa `.zip` de um repositorio publico **sem assinatura**,
traz GPL-2.0/LGPL-2.1 dentro e executa o `.sh` de dentro como root (dentro do
`bwrap`, com rootfs somente-leitura — o sandbox e' do sistema, o conteudo e'
deles).

O que o sistema garante e' o sandbox; o que ele **nao** garante e' o conteudo.
Por isso o artefato nao entra em `itens` (a lista que a loja mostra como
conteudo proprio): ele entra em `sideload`, com `origem: "sideload"`, `licenca`
e `aviso` obrigatorios. Quem desenha a lista nao consegue publica-lo como
conteudo da loja por esquecimento — a separacao e' do formato, nao da interface.

## O que este modulo se recusa a aceitar, e por que cada um

- **`url` sem `https`**: a loja recusa `http://` (`Item::url_valida`), entao um
  manifesto com http produziria uma lista que a propria loja nao abre.
- **`sha256` que nao e' 64 hex**: o download passa a nao ser conferivel, e
  "baixar e confiar" e' exatamente o que o PortMaster nao pode exigir de nos.
- **`aviso` vazio**: a interface precisa de uma frase para dizer que aquilo nao
  veio assinado. Sem texto, o item apareceria com o mesmo selo dos outros.
- **campo desconhecido**: `sha` no lugar de `sha256` seria ignorado em silencio
  e o item passaria sem hash. O validador do `metadata.json` ja' e'
  `additionalProperties: false` pelo mesmo motivo.

O manifesto **nao** e' assinado, e nao precisa ser: ele nao promete autoria, so'
declara o que sera' baixado. Quem confere o hash e' o `instalar` no momento da
copia.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

SCHEMA = "1.0"

ORIGEM = "sideload"

# Para onde o artefato vai em `/user`. `ports` e' o PortMaster; `apps` e' o que
# vira cartao na aba Apps; `games` e' o que o usuario instala como jogo.
DESTINOS = ("ports", "apps", "games")

CAMPOS = (
    "id",
    "titulo",
    "developer",
    "url",
    "sha256",
    "tamanho_bytes",
    "destino",
    "licenca",
    "aviso",
)

_SLUG = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
_SHA256 = re.compile(r"^[0-9a-f]{64}$")


class SideloadError(ValueError):
    """O manifesto nao serve: caminho, JSON ou item invalido."""


def _texto(item: dict, campo: str) -> str:
    valor = item.get(campo)
    if not isinstance(valor, str) or not valor.strip():
        raise SideloadError(f"'{campo}' ausente ou vazio")
    return valor.strip()


def validar_item(item: dict) -> dict:
    """Valida um item e devolve a forma canonica que vai para `store.json`."""
    if not isinstance(item, dict):
        raise SideloadError("item nao e' um objeto")

    desconhecidos = sorted(set(item) - set(CAMPOS))
    if desconhecidos:
        # Ignorar em silencio um `sha` no lugar de `sha256` faria o item passar
        # sem hash — o mesmo padrao do `buttons.map` que descartava simbolo.
        raise SideloadError(
            f"campo(s) desconhecido(s): {', '.join(desconhecidos)} "
            f"(validos: {', '.join(CAMPOS)})")

    ident = _texto(item, "id")
    if not _SLUG.match(ident):
        raise SideloadError(f"id '{ident}' nao e' kebab-case ASCII")

    url = _texto(item, "url")
    if not url.startswith("https://"):
        raise SideloadError(
            f"url tem de ser https:// (a loja recusa http): {url}")

    digest = _texto(item, "sha256").lower()
    if not _SHA256.match(digest):
        raise SideloadError(
            f"sha256 '{digest}' nao e' 64 caracteres hexadecimais")

    tamanho = item.get("tamanho_bytes")
    if not isinstance(tamanho, int) or isinstance(tamanho, bool) or tamanho <= 0:
        raise SideloadError(f"tamanho_bytes '{tamanho}' tem de ser inteiro > 0")

    destino = _texto(item, "destino")
    if destino not in DESTINOS:
        raise SideloadError(
            f"destino '{destino}' fora de {'|'.join(DESTINOS)}")

    # `licenca` e `aviso` sao o que a interface mostra para dizer que o item
    # nao veio assinado; sem eles o selo seria o mesmo dos pacotes da loja.
    return {
        "id": ident,
        "titulo": _texto(item, "titulo"),
        "developer": _texto(item, "developer"),
        "url": url,
        "sha256": digest,
        "tamanho_bytes": tamanho,
        "destino": destino,
        "licenca": _texto(item, "licenca"),
        "aviso": _texto(item, "aviso"),
        "origem": ORIGEM,
        "assinado": False,
    }


def ler(caminho: Path) -> list[dict]:
    """Le e valida `sideload.json`. Arquivo ausente devolve lista vazia.

    Ausente **nao** e' erro: a maioria das pastas de pacotes nao tem artefato de
    terceiro. Um arquivo presente e invalido e' erro, e o motivo e' o do item —
    nao um "manifesto invalido" generico que nao diz o que corrigir.
    """
    if not caminho.is_file():
        return []
    try:
        doc = json.loads(caminho.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError, UnicodeDecodeError) as exc:
        raise SideloadError(f"{caminho.name}: ilegivel ({exc})") from exc

    if not isinstance(doc, dict):
        raise SideloadError(f"{caminho.name}: o topo tem de ser um objeto")
    if doc.get("schema_version") != SCHEMA:
        raise SideloadError(
            f"{caminho.name}: schema_version '{doc.get('schema_version')}' "
            f"nao e' '{SCHEMA}'")

    itens = doc.get("itens", [])
    if not isinstance(itens, list):
        raise SideloadError(f"{caminho.name}: 'itens' tem de ser uma lista")

    validados = []
    vistos = set()
    for indice, item in enumerate(itens):
        try:
            pronto = validar_item(item)
        except SideloadError as exc:
            ident = item.get("id") if isinstance(item, dict) else indice
            raise SideloadError(f"{caminho.name}: item '{ident}': {exc}") from exc
        if pronto["id"] in vistos:
            # Duplicata no manifesto faria o mesmo artefato aparecer duas vezes
            # com destinos possivelmente diferentes.
            raise SideloadError(
                f"{caminho.name}: id '{pronto['id']}' repetido")
        vistos.add(pronto["id"])
        validados.append(pronto)
    return _ordenado(validados)


def _ordenado(itens: list[dict]) -> list[dict]:
    """Ordem estavel por id — o `store.json` nao pode depender da ordem do arquivo."""
    return sorted(itens, key=lambda i: i["id"])


def caminho_padrao(origem: Path) -> Path:
    """`sideload.json` ao lado dos pacotes (a pasta de onde saem os `.giochi`)."""
    return origem / "sideload.json"


__all__ = [
    "CAMPOS",
    "DESTINOS",
    "ORIGEM",
    "SCHEMA",
    "SideloadError",
    "caminho_padrao",
    "ler",
    "validar_item",
]
