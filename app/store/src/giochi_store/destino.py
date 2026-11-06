"""Descobrir onde está a partição `/user` do console — cartão ou MTP.

POR QUE ISSO EXISTE
-------------------
Há **dois** caminhos para copiar um `.giochi` para o console, e o usuário não
deveria precisar lembrar qual:

  * **cartão no leitor** → o gerenciador de arquivos monta em
    `/run/media/<usuário>/GIOCHI_USER` (ext4, dono root);
  * **MTP pelo cabo USB** → o gvfs monta em
    `/run/user/<uid>/gvfs/mtp:host=...` e o conteúdo é o mesmo `/user`, com
    `games/`, `music/`, `ports/` e `.giochi/`.

Antes disso, cada ferramenta pedia o caminho na mão, e caminho errado significa
copiar 40 MB para o lugar errado sem erro nenhum. A descoberta é por
**assinatura de conteúdo** (`games/` ou `.giochi/`), nunca pelo nome do ponto de
montagem: o nome do MTP muda com o bus, o device e o vendor string.

Limitação honesta do MTP: ele expõe arquivos, não permissões. `settings.json` e
`ssh/authorized_keys` precisam de modo `0600` e de diretório `0700`, e o MTP não
os aplica — por isso o Modo Desenvolvedor é preparado pelo **cartão**
(`tools/giochi-dev-mode.sh`), não por MTP.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

MARCA_JOGOS = "games"
MARCA_PERFIL = ".giochi"


@dataclass
class Destino:
    caminho: Path
    tipo: str  # "cartao" | "mtp" | "manual"

    def __str__(self) -> str:
        return f"{self.tipo}: {self.caminho}"


def _tem_assinatura(path: Path) -> bool:
    """A assinatura da partição `/user`: `games/` ou `.giochi/`.

    Criados pelo `S10storage` (seed da árvore) e pelo `giochi-init`
    (`device_profile.json`). Qualquer um dos dois basta — um cartão recém
    gravado já tem os dois.
    """
    try:
        if (path / MARCA_JOGOS).is_dir():
            return True
        if (path / MARCA_PERFIL).is_dir():
            return True
    except OSError:
        return False
    return False


def _candidatos() -> list[tuple[Path, str]]:
    lista: list[tuple[Path, str]] = []

    # 1. MTP pelo gvfs: o mais volátil, então vem primeiro (se o aparelho está
    #    plugado, é o que o usuário quer).
    #
    #    ATENÇÃO ao nível: o gvfs monta o VOLUME em
    #    /run/user/<uid>/gvfs/mtp:host=.../, e o /user da partição aparece como
    #    uma pasta dentro dele (no uMTP-Responder o volume se chama "Giochi").
    #    Olhar só a raiz do mount nunca acha a assinatura `games/`: medido em
    #    2026-11-01, com o console ligado e o MTP montado, `destinos` imprimia
    #    "nenhum destino encontrado" — o aparelho estava visível e a ferramenta
    #    dizia que não.
    for base in Path("/run/user").glob("*/gvfs"):
        for item in sorted(base.glob("mtp*")):
            if _tem_assinatura(item):
                lista.append((item, "mtp"))
                continue
            for dentro in sorted(item.glob("*")):
                if dentro.is_dir() and _tem_assinatura(dentro):
                    lista.append((dentro, "mtp"))

    # 2. Cartão montado pelo udisks.
    for base in (Path("/run/media"), Path("/media")):
        if not base.is_dir():
            continue
        for usuario in sorted(base.iterdir()):
            if not usuario.is_dir():
                continue
            if _tem_assinatura(usuario):
                # Alguns gerenciadores montam a partição direto em /media/x.
                lista.append((usuario, "cartao"))
                continue
            for sub in sorted(usuario.glob("*")):
                if sub.is_dir():
                    lista.append((sub, "cartao"))

    return lista


def descobrir(preferir: str | None = None) -> Destino | None:
    """Primeiro destino com assinatura válida. `preferir` força um tipo."""
    vistos: set[Path] = set()

    for caminho, tipo in _candidatos():
        if caminho in vistos:
            continue
        vistos.add(caminho)
        if preferir and tipo != preferir:
            continue
        if _tem_assinatura(caminho):
            return Destino(caminho, tipo)
    return None


def todos() -> list[Destino]:
    """Todos os destinos válidos, para o usuário escolher quando houver dois."""
    achados: list[Destino] = []
    vistos: set[Path] = set()

    for caminho, tipo in _candidatos():
        if caminho in vistos or not _tem_assinatura(caminho):
            continue
        vistos.add(caminho)
        achados.append(Destino(caminho, tipo))
    return achados


def games_de(destino: Destino) -> Path:
    return destino.caminho / MARCA_JOGOS


def resumo() -> str:
    """Texto para mensagem de erro: o que existe e o que fazer."""
    achados = todos()

    if not achados:
        return (
            "nenhum destino encontrado.\n"
            "  - cartão: coloque no leitor (o gerenciador monta sozinho);\n"
            "  - MTP:   ligue o console com o cabo USB de DADOS e aceite o\n"
            "            volume 'Giochi' que aparecer.")
    return "\n".join(f"  - {d}" for d in achados)


__all__ = ["Destino", "descobrir", "todos", "games_de", "resumo"]
