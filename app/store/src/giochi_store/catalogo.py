"""Leitura e validacao dos pacotes `.giochi` (lado PC).

Nada aqui importa SDL, launcher ou buildroot: o app de loja roda no PC do
mantenedor e tem de funcionar com Python puro, como o giochi-forge.
"""

from __future__ import annotations

import hashlib
import json
import tarfile
import time
from dataclasses import dataclass, field
from pathlib import Path

SCHEMA = "1.0"

# O Launcher so' aceita aarch64 (ADR-13 proibe emulacao).
ARCH_OK = "aarch64"
# `native` entrou no schema do Giochi app em 02/11/2026 (jogos compilados aqui e
# as proprias ferramentas). O forge foi corrigido em `package.ALLOWED_RUNTIMES`;
# esta lista e' a segunda copia do mesmo enum, e ela ficou para tras — o store
# recusou os dois pacotes do proprio projeto com "runtime 'native' fora de
# godot|retroarch|proton". Duas listas envelhecem diferente, por isso existe
# `tests/consistencia-runtimes.test.mjs`, que compara as duas com o schema.
RUNTIMES_OK = ("godot", "retroarch", "proton", "native")

# `id` NAO esta' aqui de proposito: o schemas/metadata.schema.json nao o exige
# (required = title, developer, version, arch, runtime, entrypoint) e o
# giochi-forge segue o schema. Exigir `id` na leitura fazia o store RECUSAR um
# pacote feito pelo forge — medido em 2026-11-01, de ponta a ponta:
#
#   forge: gerado arcade-volleyball.giochi (90 KB)
#   store: pulado arcade-volleyball.giochi  metadata.json sem 'id'
#
# A identidade ja' tem fallback no nome do arquivo (Pacote.id), que e' estavel
# porque o forge grava <slug>.giochi. Quem quiser um id proprio continua
# podendo declara-lo no metadata.
CAMPOS_OBRIGATORIOS = ("title", "version", "entrypoint")


@dataclass
class Pacote:
    """Um `.giochi` lido do disco, com o veredito da validacao."""

    caminho: Path
    sha256: str
    tamanho: int
    ok: bool
    motivo: str = ""
    meta: dict = field(default_factory=dict)

    @property
    def id(self) -> str:
        return str(self.meta.get("id") or self.caminho.stem)

    @property
    def titulo(self) -> str:
        return str(self.meta.get("title") or self.caminho.stem)


def sha256_de(caminho: Path) -> str:
    h = hashlib.sha256()
    with caminho.open("rb") as fh:
        for bloco in iter(lambda: fh.read(1 << 20), b""):
            h.update(bloco)
    return h.hexdigest()


def ler_metadata(caminho: Path) -> tuple[dict | None, str]:
    """Le `metadata.json` de dentro do tar (ADR-11: e' o primeiro membro).

    Devolve (meta, motivo_de_erro). Um tar que nao abre, ou sem metadata.json,
    e' recusado com o motivo — nunca com excecao vazando para o chamador.
    """
    try:
        with tarfile.open(caminho, "r:*") as tar:
            for membro in tar:
                if membro.name != "metadata.json":
                    # metadata.json tem de ser o PRIMEIRO (ADR-11). Aceitar
                    # tar fora de ordem esconde pacote gerado por ferramenta
                    # errada, e o `giochi-extract` do console le em streaming.
                    return (None,
                            "metadata.json nao e' o primeiro membro "
                            f"(achei '{membro.name}')")
                extraido = tar.extractfile(membro)
                if extraido is None:
                    return None, "metadata.json ilegivel"
                return json.loads(extraido.read().decode("utf-8")), ""
    except (tarfile.TarError, OSError, json.JSONDecodeError, UnicodeDecodeError) as exc:
        return None, f"arquivo ilegivel: {exc}"
    return None, "sem metadata.json"


def validar(caminho: Path) -> Pacote:
    """Valida um `.giochi`. Todo caminho de recusa preenche `motivo`."""
    tamanho = caminho.stat().st_size
    digest = sha256_de(caminho)
    meta, erro = ler_metadata(caminho)

    if meta is None:
        return Pacote(caminho, digest, tamanho, False, erro)

    arch = str(meta.get("arch", ""))
    if arch != ARCH_OK:
        return Pacote(caminho, digest, tamanho, False,
                      f"arch '{arch}' nao e' {ARCH_OK} (ADR-13 proibe emulacao)",
                      meta)

    runtime = str(meta.get("runtime", ""))
    if runtime not in RUNTIMES_OK:
        return Pacote(caminho, digest, tamanho, False,
                      f"runtime '{runtime}' fora de {'|'.join(RUNTIMES_OK)}",
                      meta)

    for campo in CAMPOS_OBRIGATORIOS:
        if not str(meta.get(campo, "")).strip():
            return Pacote(caminho, digest, tamanho, False,
                          f"metadata.json sem '{campo}'", meta)

    entry = str(meta["entrypoint"])
    if not entry.startswith("bin/") or ".." in entry:
        return Pacote(caminho, digest, tamanho, False,
                      f"entrypoint '{entry}' tem de ser bin/<algo> sem '..'",
                      meta)

    return Pacote(caminho, digest, tamanho, True, "", meta)


def varrer(origem: Path) -> list[Pacote]:
    """Todos os `.giochi` da pasta, em ordem estavel de nome."""
    if not origem.is_dir():
        return []
    return [validar(p) for p in sorted(origem.glob("*.giochi"))]


def catalogo(pacotes: list[Pacote]) -> dict:
    """`store.json`: o que o app da Loja mostra. Ordem estavel por titulo."""
    itens = []
    for p in sorted((x for x in pacotes if x.ok),
                    key=lambda x: x.titulo.lower()):
        itens.append({
            "id": p.id,
            "title": p.titulo,
            "version": p.meta.get("version", ""),
            "runtime": p.meta.get("runtime", ""),
            "author": p.meta.get("author", ""),
            "description": p.meta.get("description", ""),
            "size_bytes": p.tamanho,
            "sha256": p.sha256,
            "arquivo": p.caminho.name,
        })
    return {
        "schema_version": SCHEMA,
        "gerado_em": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "total": len(itens),
        "itens": itens,
    }


def novidades(pacotes: list[Pacote], limite: int = 5) -> dict:
    """`news.json`: o arquivo que o Launcher le na Home e que ninguem escrevia.

    docs/APPS.md registrava o buraco: `news.json` era lido com fallback de
    dicas compiladas porque nenhum produtor existia. Este e' o produtor.
    """
    aceitos = [p for p in pacotes if p.ok]
    recentes = sorted(aceitos, key=lambda x: x.caminho.stat().st_mtime,
                      reverse=True)[:limite]
    linhas = [{
        "title": p.titulo,
        "body": (f"{p.meta.get('version', '')} • "
                 f"{p.tamanho // 1024} KB • {p.meta.get('runtime', '')}"),
        "kind": "novidade",
    } for p in recentes]

    if not linhas:
        linhas = [{
            "title": "Loja vazia",
            "body": "Nenhum pacote .giochi na pasta de origem.",
            "kind": "dica",
        }]
    return {"schema_version": SCHEMA, "items": linhas}
