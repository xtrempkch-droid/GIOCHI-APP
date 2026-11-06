"""Loja local: le uma pasta de projetos baixados e produz os `.giochi`.

O caso de uso e o do console: voce baixa um projeto do GitHub (zip ou clone),
joga a pasta em um diretorio, e este modulo descobre o que aquilo e, diz o que
falta e empacota o que ja da para empacotar.

**Por que isto roda no PC e nao no aparelho:** o rootfs do console nao tem
toolchain (nenhum `BR2_PACKAGE_GCC/MAKE/CMAKE`) e tem 1-2 GB de RAM; e o
`giochi-pack.sh` exige GNU tar (`--format=ustar`, `--owner/--group`, `-r`), que
o BusyBox nao tem (AGENTS.md 3.12). O aparelho *instala* o `.giochi`; quem
*produz* e este lado.

A deteccao e explicita de proposito: cada pasta recebe um `kind`, e so o kind
`pacote` e empacotavel hoje. Os outros dois casos (projeto Godot e arvore de
fonte) **nao** sao falha silenciosa -- viram uma linha de relatorio dizendo o
que falta, que e a informacao util na hora de decidir o que portar.
"""

from __future__ import annotations

import json
import shutil
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path

from .classify import classify
from .package import PackageError, build_package, sign_package, validate_metadata

# Arquivos que definem a "forma" de um projeto, na ordem em que sao testados.
GODOT_MARKERS = ("project.godot", "export_presets.cfg")
SOURCE_MARKERS = ("CMakeLists.txt", "Makefile", "meson.build", "configure")
PACKAGE_MARKERS = ("metadata.json", "capa.png", "bin")


class Kind(str, Enum):
    """O que a pasta parece ser."""

    PACKAGE = "pacote"          # metadata.json + capa.png + bin/ -> empacotavel
    PACKAGE_INCOMPLETE = "pacote-incompleto"  # tem metadata.json, falta o resto
    GODOT = "projeto-godot"     # project.godot: precisa export (ADR-03)
    SOURCE = "fonte"            # arvore de compilacao: precisa cross-compile
    UNKNOWN = "desconhecido"

    @property
    def label_pt(self) -> str:
        return {
            Kind.PACKAGE: "pacote",
            Kind.PACKAGE_INCOMPLETE: "pacote incompleto",
            Kind.GODOT: "projeto Godot",
            Kind.SOURCE: "fonte",
            Kind.UNKNOWN: "desconhecido",
        }[self]


class Status(str, Enum):
    BUILT = "gerado"
    SKIPPED = "pulado"
    ERROR = "erro"

    @property
    def label_pt(self) -> str:
        return {Status.BUILT: "gerado", Status.SKIPPED: "pulado",
                Status.ERROR: "erro"}[self]


@dataclass
class Candidate:
    """Uma pasta examinada e o que se descobriu dela."""

    path: Path
    kind: Kind
    status: Status
    detail: str
    package: Path | None = None
    sha256: str | None = None
    size_bytes: int = 0
    title: str = ""

    @property
    def name(self) -> str:
        return self.title or self.path.name


@dataclass
class Report:
    candidates: list[Candidate] = field(default_factory=list)
    out_dir: Path | None = None
    dry_run: bool = False

    def count(self, status: Status) -> int:
        return sum(1 for c in self.candidates if c.status is status)

    @property
    def has_errors(self) -> bool:
        return self.count(Status.ERROR) > 0


def _has(path: Path, names: tuple[str, ...]) -> bool:
    return any((path / name).exists() for name in names)


def detect_kind(path: Path) -> Kind:
    """Classifica a pasta so pelos arquivos que ela tem."""
    if (path / "metadata.json").is_file():
        faltando = [n for n in PACKAGE_MARKERS if not (path / n).exists()]
        return Kind.PACKAGE if not faltando else Kind.PACKAGE_INCOMPLETE
    if _has(path, GODOT_MARKERS):
        return Kind.GODOT
    if _has(path, SOURCE_MARKERS):
        return Kind.SOURCE
    return Kind.UNKNOWN


def _describe_gap(path: Path, kind: Kind) -> str:
    """Diz o que falta, em vez de so recusar."""
    if kind is Kind.PACKAGE_INCOMPLETE:
        faltando = [n for n in PACKAGE_MARKERS if not (path / n).exists()]
        return "falta " + ", ".join(faltando)
    if kind is Kind.GODOT:
        # O bloqueio NAO e de exportacao, e sim de plataforma — e a versao
        # anterior desta mensagem estava errada em dois pontos, medida contra o
        # aparelho em 06/11/2026:
        #
        # 1. o rootfs NAO e musl: e glibc 2.38. O binario do arcade-volleyball,
        #    compilado aqui contra o glibc do aparelho, exige GLIBC_2.38 e roda.
        # 2. nao adianta "recompilar o template contra musl": nenhum build
        #    oficial do Godot tem driver KMSDRM, e o rootfs nao tem X11 nem
        #    Wayland. Exportar produz um binario que cai em Headless e nao
        #    desenha nada. Ver docs/godot.md.
        return ("projeto Godot nao roda neste aparelho: nenhum build oficial do "
                "Godot tem KMSDRM e o rootfs nao tem X11/Wayland (docs/godot.md) "
                "-- o caminho e port nativo (runtime \"native\")")
    if kind is Kind.SOURCE:
        # Idem: era "aarch64/musl". O alvo e aarch64/glibc 2.38, e o
        # cross-compile existe (scripts/compilar-jogo-*-arm64.sh).
        return ("precisa cross-compile para aarch64/glibc 2.38; ver "
                "scripts/compilar-jogo-arm64.sh (Rust) e "
                "scripts/compilar-jogo-cpp-arm64.sh (C++)")
    if kind is Kind.UNKNOWN:
        return "sem metadata.json, project.godot nem build system reconhecido"
    return ""


def _inspect_package(path: Path, profile) -> str:
    """Valida o metadata e, se houver forge.json, classifica contra o aparelho.

    Devolve o titulo quando o pacote pode ser gerado; levanta PackageError
    quando nao pode.
    """
    try:
        metadata = json.loads((path / "metadata.json").read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise PackageError(f"metadata.json ilegivel: {exc}") from exc

    validate_metadata(metadata)

    if profile is not None and (path / "forge.json").is_file():
        from .classify import GameRequirements

        req = GameRequirements.from_forge_json(path / "forge.json")
        result = classify(req, profile)
        if not result.allowed:
            # A mensagem sai dos **dados**, não do texto já formatado.
            # Prefixar `label_pt` a `summary()` produzia "Incompatível:
            # Incompatível", e a tentativa de limpar com `strip(" -[]")`
            # arrancava o `[` de um lado e deixava o `]` do outro. O veredito
            # já está no começo; o que interessa é o motivo.
            motivos = "; ".join(
                f"{i.requirement}: {i.detail}" for i in result.issues
            )
            raise PackageError(f"{result.verdict.label_pt}: {motivos}")
    return str(metadata.get("title", path.name))


def scan(src: str | Path) -> list[Path]:
    """Subdiretorios de primeiro nivel, em ordem estavel.

    Nao desce mais fundo de proposito: um zip do GitHub extrai como
    `<repo>-main/`, e descer arbitrariamente faria a loja achar projeto dentro
    de `examples/` de outro projeto.
    """
    root = Path(src)
    if not root.is_dir():
        return []
    return sorted(p for p in root.iterdir() if p.is_dir() and not p.name.startswith("."))


def run(
    src: str | Path,
    out: str | Path,
    *,
    key: str | Path | None = None,
    profile=None,
    dry_run: bool = False,
) -> Report:
    """Examina `src` e empacota em `out` tudo o que ja e um pacote valido."""
    report = Report(out_dir=Path(out), dry_run=dry_run)

    for path in scan(src):
        kind = detect_kind(path)
        if kind is not Kind.PACKAGE:
            report.candidates.append(
                Candidate(path=path, kind=kind, status=Status.SKIPPED,
                          detail=_describe_gap(path, kind))
            )
            continue

        try:
            title = _inspect_package(path, profile)
        except PackageError as exc:
            report.candidates.append(
                Candidate(path=path, kind=kind, status=Status.ERROR,
                          detail=str(exc), title=path.name)
            )
            continue

        if dry_run:
            report.candidates.append(
                Candidate(path=path, kind=kind, status=Status.BUILT,
                          detail="(dry-run)", title=title)
            )
            continue

        out_dir = Path(out)
        out_dir.mkdir(parents=True, exist_ok=True)
        destino = out_dir / f"{path.name}.giochi"
        try:
            result = build_package(path, destino)
        except (PackageError, OSError) as exc:
            report.candidates.append(
                Candidate(path=path, kind=kind, status=Status.ERROR,
                          detail=f"empacotamento falhou: {exc}", title=title)
            )
            continue

        if key:
            try:
                sign_package(result.package, key, version=_version_of(path))
            except (PackageError, OSError) as exc:
                # Pacote sem assinatura nao passa no giochi-verify do console:
                # deixa-lo no diretorio e convite a gravar engano.
                destino.unlink(missing_ok=True)
                report.candidates.append(
                    Candidate(path=path, kind=kind, status=Status.ERROR,
                              detail=f"assinatura falhou: {exc} "
                                     f"(pacote descartado)", title=title)
                )
                continue

        report.candidates.append(
            Candidate(path=path, kind=kind, status=Status.BUILT,
                      detail=result.package.name, title=title,
                      package=result.package, sha256=result.sha256,
                      size_bytes=result.size_bytes)
        )

    return report


def _version_of(path: Path) -> str:
    try:
        data = json.loads((path / "metadata.json").read_text(encoding="utf-8"))
        return str(data.get("version", "0"))
    except (OSError, json.JSONDecodeError):
        return "0"


def render(report: Report) -> str:
    """Relatorio em texto, uma linha por pasta."""
    linhas: list[str] = []
    for c in report.candidates:
        extra = ""
        if c.status is Status.BUILT and c.size_bytes:
            extra = f"  {c.size_bytes / 1024:.0f} KB"
        linhas.append(
            f"  {c.status.label_pt:<7} {c.name:<32} "
            f"{c.kind.label_pt:<16} {c.detail}{extra}"
        )

    total = len(report.candidates)
    gerados = report.count(Status.BUILT)
    pulados = report.count(Status.SKIPPED)
    erros = report.count(Status.ERROR)
    linhas.append("")
    if total == 0:
        linhas.append("  nenhuma pasta encontrada")
    else:
        linhas.append(f"  {total} pasta(s): {gerados} gerado(s), "
                      f"{pulados} pulado(s), {erros} com erro")
    if report.out_dir and gerados and not report.dry_run:
        linhas.append(f"  pacotes em {report.out_dir}")
    return "\n".join(linhas)


def install(report: Report, games_dir: str | Path) -> int:
    """Copia os `.giochi` gerados para um diretorio de jogos.

    Serve para o caso "criei no PC, quero deixar pronto para levar por USB":
    `games_dir` aponta para o ponto de montagem da particao gravavel.
    """
    destino = Path(games_dir)
    if not destino.is_dir():
        raise PackageError(f"diretorio de destino inexistente: {destino}")
    copiados = 0
    for c in report.candidates:
        if c.status is not Status.BUILT or c.package is None:
            continue
        shutil.copy2(c.package, destino / c.package.name)
        copiados += 1
    return copiados
