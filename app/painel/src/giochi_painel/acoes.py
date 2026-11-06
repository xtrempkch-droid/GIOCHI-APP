"""As operações do painel, sem HTTP.

Separado do servidor de propósito: o que precisa de teste é **o que acontece**
(classificar, empacotar, listar), não o roteamento. Assim os testes rodam sem
abrir socket, e o servidor fica pequeno o bastante para ser lido de uma vez.
"""

from __future__ import annotations

import json
from dataclasses import asdict
from pathlib import Path
from typing import Any

from giochi_forge.classify import GameRequirements, classify
from giochi_forge.package import PackageError, build_package, sign_package
from giochi_forge.profile import MANUAL_PROFILES
from giochi_store.catalogo import varrer
from giochi_store import destino as destino_mod


class ErroDeAcao(ValueError):
    """Entrada inválida: o painel devolve 400, não 500."""


def aparelhos() -> list[dict[str, Any]]:
    """Perfis conhecidos, com o que os distingue.

    `godot_runtime` sai como texto de três estados — `null` no JSON — porque a
    diferença entre "não tem" e "não medido" é o que evita reprovar o que
    ninguém testou.
    """
    saida = []
    for chave, p in sorted(MANUAL_PROFILES.items()):
        saida.append({
            "id": chave,
            "device_id": p.device_id,
            "soc": p.soc,
            "ram_mb": p.ram_mb,
            "display": f"{p.display_width}x{p.display_height}",
            "utilizavel": "%dx%d" % p.usable_display(),
            "gles": p.gles_max,
            "godot_runtime": p.godot_runtime,
        })
    return saida


def _projeto(caminho: str) -> Path:
    p = Path(caminho).expanduser()
    if not p.is_dir():
        # Dizer **o caminho resolvido** e não só o digitado: caminho relativo é
        # relativo ao diretório de onde o painel foi aberto, e quem digitou
        # "app/jogos/volley" no navegador não tem como adivinhar qual foi.
        raise ErroDeAcao(f"pasta não encontrada: {caminho} (resolvido para {p.resolve()})")
    if not (p / "metadata.json").is_file():
        raise ErroDeAcao(f"{caminho} não tem metadata.json (não é pasta de app)")
    return p


def classificar(projeto: str, aparelho: str) -> dict[str, Any]:
    """Roda o classificador e devolve o veredito em forma de dados."""
    p = _projeto(projeto)
    perfil = MANUAL_PROFILES.get(aparelho)
    if perfil is None:
        raise ErroDeAcao(f"aparelho desconhecido: {aparelho}")

    forge_json = p / "forge.json"
    if forge_json.is_file():
        req = GameRequirements.from_forge_json(forge_json)
        origem = "forge.json"
    else:
        # Sem forge.json o classificador não sabe o que o app exige; assumir
        # zero reprovaria nada e o painel diria "compatível" para tudo.
        raise ErroDeAcao(f"{projeto} não tem forge.json: sem requisitos não há o que classificar")

    resultado = classify(req, perfil)
    return {
        "projeto": str(p),
        "aparelho": aparelho,
        "origem": origem,
        "veredito": resultado.verdict.label_pt,
        "codigo": int(resultado.verdict),
        "permitido": resultado.allowed,
        "problemas": [
            {"requisito": i.requirement, "detalhe": i.detail, "veredito": i.verdict.label_pt}
            for i in resultado.issues
        ],
        "avisos": resultado.warnings,
    }


def empacotar(projeto: str, saida: str, chave: str | None = None) -> dict[str, Any]:
    """Empacota um app em `.giochi` e, se houver chave, assina."""
    p = _projeto(projeto)
    destino = Path(saida).expanduser()
    destino.mkdir(parents=True, exist_ok=True)

    meta = json.loads((p / "metadata.json").read_text(encoding="utf-8"))
    slug = meta.get("slug") or p.name.replace("_", "-")
    alvo = destino / f"{slug}.giochi"

    try:
        resultado = build_package(p, alvo)
    except PackageError as exc:
        raise ErroDeAcao(str(exc)) from exc

    assinado = False
    if chave:
        caminho_chave = Path(chave).expanduser()
        if not caminho_chave.is_file():
            raise ErroDeAcao(f"chave não encontrada: {chave}")
        # `version` e obrigatoria e keyword-only: sem ela o manifesto sai sem a
        # versao e a loja nao compara com a instalada. O erro so apareceu quando
        # o painel chamou de verdade — o `forge build` ja a passava.
        sign_package(alvo, caminho_chave, version=str(meta.get("version", "0.0.0")))
        assinado = True

    return {
        "arquivo": str(resultado.package),
        "sha256": resultado.sha256,
        "bytes": resultado.size_bytes,
        "assinado": assinado,
    }


def pacotes(origem: str) -> dict[str, Any]:
    """Lista os `.giochi` de uma pasta, com o veredito de cada um."""
    pasta = Path(origem).expanduser()
    if not pasta.is_dir():
        raise ErroDeAcao(f"pasta não encontrada: {origem}")

    itens = []
    for pacote in varrer(pasta):
        itens.append({
            "arquivo": pacote.caminho.name,
            "id": pacote.id,
            "titulo": pacote.titulo,
            "bytes": pacote.tamanho,
            "sha256": pacote.sha256,
            "ok": pacote.ok,
            "motivo": pacote.motivo,
            "versao": str(pacote.meta.get("version", "")),
            "runtime": str(pacote.meta.get("runtime", "")),
        })
    return {"pasta": str(pasta), "pacotes": itens}


def destinos() -> dict[str, Any]:
    """Onde o console está visível neste computador."""
    encontrados = [
        {"caminho": str(d.caminho), "tipo": d.tipo} for d in destino_mod.todos()
    ]
    return {
        "destinos": encontrados,
        "resumo": destino_mod.resumo(),
    }


__all__ = [
    "ErroDeAcao",
    "aparelhos",
    "classificar",
    "destinos",
    "empacotar",
    "pacotes",
]
