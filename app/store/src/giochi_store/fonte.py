"""Fonte GitHub Releases: le uma lista de repositorios e resolve o build aarch64.

A entrada e' o `apps.json` do projeto **GithubLauncher**
(<https://github.com/SirDiabo/GithubLauncher>, MIT), que o mantenedor apontou:

    {"apps": [{"name": "...", "repository": "owner/repo",
               "folderName": "...", "appIconUrl": null}]}

Por que isso resolve o que o itch.io nao resolvia: no GitHub Releases cada
arquivo tem nome proprio, entao o build de aarch64 pode ser escolhido pela
convencao do nome -- e, depois de baixar, **conferido no cabecalho ELF**. Nome
de arquivo e' pista; `e_machine` e' prova.

Uma diferenca de rigor em relacao ao matcher do GithubLauncher, de proposito:
o deles aceita `armv7`/`armhf` como "ARM" (certo para um launcher generico, que
roda tambem em 32 bits). O aparelho do Giochi e' aarch64, entao binario de 32
bits **nao serve** e e' recusado aqui.
"""

from __future__ import annotations

import json
import re
import struct
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

API = "https://api.github.com"
EM_AARCH64 = 0xB7
USER_AGENT = "giochi-store"

# Marcadores que tiram o asset da disputa, qualquer que seja o alvo.
OUTRA_PLATAFORMA = (
    "windows", "win32", "win64", "win-x64", "win-x86", "-win.", "_win.",
    ".exe", ".msi", "macos", "osx", "darwin", ".dmg", ".pkg",
    "switch", "android", ".apk",
)

LINUX = ("linux", ".appimage", ".deb", ".rpm", "tar.gz", "tar.xz", ".tgz")
ARM64 = ("aarch64", "arm64", ".arm64", "arm-64")
ARM32 = ("armv7", "armv6", "armhf", "armel", "arm32", "arm-")
X86 = ("x86_64", "x86-64", "amd64", "x64", "i686", "i386", "i586", "x86_32")

# Formato: quanto menor, mais util para nos. Medido no teste real -- a primeira
# versao escolheu o `.deb` do TIC-80 (inutil: nao temos dpkg) so' porque vinha
# antes no alfabeto. Pacote de distro e' ultimo recurso, nunca preferencia.
FORMATO_EXTRAIVEL = (".zip", ".tar.gz", ".tar.xz", ".tgz", ".tar.bz2")
FORMATO_PACOTE = (".deb", ".rpm", ".apk", ".flatpak", ".snap")


class FonteError(ValueError):
    """Falha ao ler a lista ou falar com a API do GitHub."""


@dataclass
class AppEntry:
    """Uma entrada da lista. Campo desconhecido no JSON nao quebra a leitura."""

    nome: str
    repositorio: str
    pasta: str = ""
    icone: str | None = None
    bruto: dict = field(default_factory=dict, repr=False)

    @property
    def arquivo(self) -> str:
        return (self.pasta or self.repositorio.split("/")[-1]).replace("/", "-")


def normalizar_repositorio(texto: str) -> str:
    """Aceita 'owner/repo', URL https do GitHub e URL com .git no fim."""
    valor = str(texto).strip()
    m = re.search(r"github\.com[/:]+([\w.-]+)/([\w.-]+?)(?:\.git)?/?$", valor)
    if m:
        return f"{m.group(1)}/{m.group(2)}"
    if re.fullmatch(r"[\w.-]+/[\w.-]+", valor):
        return valor
    raise FonteError(f"repositorio invalido: {texto!r} (esperado owner/repo)")


def ler_apps_json(caminho: str | Path) -> list[AppEntry]:
    """Le o apps.json do GithubLauncher, ou uma lista simples de repositorios."""
    try:
        dados = json.loads(Path(caminho).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise FonteError(f"lista ilegivel: {exc}") from exc

    if isinstance(dados, dict):
        itens = dados.get("apps", [])
    elif isinstance(dados, list):
        itens = dados
    else:
        raise FonteError("lista: esperado objeto com 'apps' ou lista")

    if not isinstance(itens, list):
        raise FonteError("lista: 'apps' nao e' uma lista")

    entradas: list[AppEntry] = []
    for item in itens:
        if isinstance(item, str):
            repo = normalizar_repositorio(item)
            entradas.append(AppEntry(nome=repo.split("/")[-1], repositorio=repo))
            continue
        if not isinstance(item, dict):
            raise FonteError(f"lista: entrada invalida: {item!r}")
        if not item.get("repository"):
            raise FonteError("lista: entrada sem 'repository'")
        repo = normalizar_repositorio(item["repository"])
        entradas.append(AppEntry(
            nome=str(item.get("name") or repo.split("/")[-1]),
            repositorio=repo,
            pasta=str(item.get("folderName") or ""),
            icone=item.get("appIconUrl"),
            bruto=item,
        ))
    return entradas


def _marcador(nome: str, marcadores: tuple[str, ...]) -> str | None:
    for m in marcadores:
        if m in nome:
            return m
    return None


def escolher_asset(assets: list[dict]) -> tuple[dict | None, str]:
    """Escolhe o asset aarch64. Devolve (asset, motivo) -- motivo sempre.

    Nunca devolve asset cuja arquitetura nao possa ser provada pelo nome: e'
    melhor recusar com motivo do que baixar 40 MB de binario x86 para descobrir
    depois que nao roda.
    """
    candidatos: list[tuple[int, dict]] = []
    recusas: list[str] = []

    for asset in assets:
        nome = str(asset.get("name", "")).lower()
        if not nome:
            continue
        if not asset.get("browser_download_url"):
            continue

        ruim = _marcador(nome, OUTRA_PLATAFORMA)
        if ruim:
            recusas.append(f"{nome}: plataforma nao serve ({ruim})")
            continue
        if not _marcador(nome, LINUX):
            recusas.append(f"{nome}: sem marcador de Linux")
            continue
        # ARM64 e' testado ANTES do marcador de 32 bits porque "arm-" esta' em
        # ARM32 e casaria dentro de "arm-64" -- recusando o build certo.
        marca = _marcador(nome, ARM64)
        if not marca:
            if _marcador(nome, ARM32):
                recusas.append(f"{nome}: ARM de 32 bits")
                continue
            if _marcador(nome, X86):
                recusas.append(f"{nome}: x86")
                continue
            recusas.append(f"{nome}: nao diz arm64/aarch64")
            continue
        if _marcador(nome, X86) and "aarch64" not in nome:
            # "x64"/"x64_64" no mesmo nome de um "arm64" e' release combinado
            # (rarissimo); aarch64 explicito desempata, o resto recusa.
            recusas.append(f"{nome}: nome ambiguo (arm64 e x86)")
            continue

        # Ordem de preferencia (menor = melhor):
        #   0  aarch64 explicito num arquivo extraivel
        #   1  arm64 num arquivo extraivel
        #   2  aarch64 em binario solto
        #   3  pacote de distro (.deb/.rpm) -- ultimo recurso, nao temos dpkg
        extraivel = 0 if nome.endswith(FORMATO_EXTRAIVEL) else 1
        pacote = 1 if nome.endswith(FORMATO_PACOTE) else 0
        peso = (pacote * 2 + extraivel) * 2 + (0 if marca == "aarch64" else 1)
        candidatos.append((peso, asset))

    if not candidatos:
        return None, ("; ".join(recusas[:4]) if recusas else "release sem assets")

    candidatos.sort(key=lambda par: (par[0], str(par[1].get("name", ""))))
    escolhido = candidatos[0][1]
    motivo = str(escolhido.get("name"))
    if len(candidatos) > 1:
        motivo += f" (1 de {len(candidatos)} candidatos)"
    return escolhido, motivo


def verificar_elf(caminho: str | Path) -> tuple[bool, str]:
    """Le o cabecalho ELF e confirma AArch64. Nome de arquivo mente; isto nao."""
    try:
        with open(caminho, "rb") as fh:
            cabecalho = fh.read(64)
    except OSError as exc:
        return False, f"nao foi possivel ler: {exc}"

    if len(cabecalho) < 20 or cabecalho[:4] != b"\x7fELF":
        return False, "nao e' ELF (zip/tar/outro formato -- extraia primeiro)"
    if cabecalho[4] != 2:
        return False, "ELF de 32 bits"
    machine = struct.unpack_from("<H", cabecalho, 18)[0]
    if machine != EM_AARCH64:
        return False, f"e_machine 0x{machine:X} (esperado 0x{EM_AARCH64:X} = AArch64)"
    return True, "ELF64 AArch64"


def _requisitar(url: str, token: str | None, timeout: float) -> dict:
    req = urllib.request.Request(url, headers={
        "Accept": "application/vnd.github+json",
        "User-Agent": USER_AGENT,
    })
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resposta:
            return json.loads(resposta.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            raise FonteError("sem releases") from exc
        if exc.code in (401, 403):
            raise FonteError(f"API recusou ({exc.code}); use GITHUB_TOKEN") from exc
        raise FonteError(f"HTTP {exc.code}") from exc
    except (urllib.error.URLError, TimeoutError) as exc:
        raise FonteError(f"rede indisponivel ({exc})") from exc
    except json.JSONDecodeError as exc:
        raise FonteError("resposta nao e' JSON") from exc


def release_mais_recente(repositorio: str, *, token: str | None = None,
                         timeout: float = 30.0) -> dict:
    """GET /repos/{owner}/{repo}/releases/latest."""
    dados = _requisitar(f"{API}/repos/{repositorio}/releases/latest", token, timeout)
    if not isinstance(dados, dict) or "tag_name" not in dados:
        # A API do GitHub usa 401/403 de verdade (diferente da do itch.io, que
        # devolve 200 com "errors" -- AGENTS.md 3.55). Conferir o corpo custa
        # nada e cobre proxy que devolva pagina de erro com 200.
        raise FonteError("resposta sem tag_name (autenticacao ou repositorio invalido)")
    return dados


@dataclass
class Resolvido:
    entrada: AppEntry
    tag: str = ""
    asset: dict | None = None
    motivo: str = ""

    @property
    def ok(self) -> bool:
        return self.asset is not None

    @property
    def url(self) -> str:
        return str((self.asset or {}).get("browser_download_url", ""))

    @property
    def tamanho_mb(self) -> float:
        return float((self.asset or {}).get("size", 0)) / 1048576.0


def resolver(entradas: list[AppEntry], *, token: str | None = None,
             cliente=None) -> list[Resolvido]:
    """Resolve o asset de cada entrada. `cliente` permite testar sem rede."""
    out: list[Resolvido] = []
    for entrada in entradas:
        try:
            if cliente is None:
                release = release_mais_recente(entrada.repositorio, token=token)
            else:
                release = cliente(entrada.repositorio)
        except FonteError as exc:
            out.append(Resolvido(entrada=entrada,
                                 motivo=f"{entrada.repositorio}: {exc}"))
            continue
        asset, motivo = escolher_asset(release.get("assets", []))
        out.append(Resolvido(entrada=entrada,
                             tag=str(release.get("tag_name", "")),
                             asset=asset, motivo=motivo))
    return out


def baixar(resolvido: Resolvido, destino: Path, *, token: str | None = None,
           timeout: float = 120.0) -> tuple[Path, bool, str]:
    """Baixa o asset e confere o ELF quando ele for um binario solto.

    Devolve (caminho, elf_ok, detalhe). Pacote (zip/tar) **nao** e' ELF: nesse
    caso `elf_ok` e' False e o detalhe diz que falta extrair -- o veredito final
    fica para quem extrai, e o motivo aparece no relatorio em vez de virar um
    "ok" silencioso.
    """
    if not resolvido.ok:
        raise FonteError("nada a baixar: asset nao resolvido")
    destino.mkdir(parents=True, exist_ok=True)
    nome = str(resolvido.asset.get("name", "pacote.bin"))
    alvo = destino / nome

    req = urllib.request.Request(resolvido.url, headers={"User-Agent": USER_AGENT})
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resposta, \
                open(alvo, "wb") as saida:
            for bloco in iter(lambda: resposta.read(1 << 20), b""):
                saida.write(bloco)
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        alvo.unlink(missing_ok=True)
        raise FonteError(f"download falhou: {exc}") from exc

    elf_ok, detalhe = verificar_elf(alvo)
    return alvo, elf_ok, detalhe


def relatorio(resultados: list[Resolvido]) -> str:
    linhas: list[str] = []
    for r in resultados:
        marca = "ok  " if r.ok else "--  "
        tag = f"  {r.tag}" if r.tag else ""
        tamanho = f"  {r.tamanho_mb:5.1f} MB" if r.ok else ""
        linhas.append(f"  {marca}{r.entrada.nome:<26} {r.motivo}{tag}{tamanho}")
    ok = sum(1 for r in resultados if r.ok)
    linhas.append("")
    linhas.append(f"  {len(resultados)} app(s): {ok} com build aarch64, "
                  f"{len(resultados) - ok} sem")
    return "\n".join(linhas)
