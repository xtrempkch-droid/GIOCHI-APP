"""CLI do giochi-forge (S13).

    python3 -m giochi_forge profile  --out meudevice.json
    python3 -m giochi_forge check    --profile device_profile.json --project .
    python3 -m giochi_forge build    --project . --out jogo.giochi
    python3 -m giochi_forge keys     --out keys/
    python3 -m giochi_forge store    --src baixados/ --out pacotes/ --key chave.pem
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from . import __version__
from .classify import (
    GameRequirements,
    Verdict,
    classify,
)
from . import store as store_mod
from .package import PackageError, build_package, generate_keypair, sign_package
from .profile import MANUAL_PROFILES, DeviceProfile, ProfileError

EXIT_COMPATIBLE = 0
EXIT_PARTIAL = 1
EXIT_INCOMPATIBLE = 2
EXIT_ERROR = 3


def _load_profile(args: argparse.Namespace) -> DeviceProfile:
    if args.profile:
        return DeviceProfile.from_file(args.profile)
    if args.device:
        try:
            return MANUAL_PROFILES[args.device]
        except KeyError as exc:
            raise ProfileError(
                f"perfil manual desconhecido: {args.device} "
                f"(opcoes: {', '.join(sorted(MANUAL_PROFILES))})"
            ) from exc
    raise ProfileError("informe --profile PATH ou --device NOME")


def _load_requirements(project: Path) -> GameRequirements:
    forge_json = project / "forge.json"
    if forge_json.is_file():
        return GameRequirements.from_forge_json(forge_json)
    project_godot = project / "project.godot"
    if project_godot.is_file():
        return GameRequirements.infer_from_project_godot(project_godot)
    raise ProfileError(
        f"{project}: sem forge.json nem project.godot (use --advanced para ignorar)"
    )


def cmd_profile(args: argparse.Namespace) -> int:
    profile = MANUAL_PROFILES[args.device]
    data = json.dumps(profile.raw, indent=2, ensure_ascii=False) + "\n"
    if args.out:
        Path(args.out).write_text(data, encoding="utf-8")
        print(f"perfil {args.device} escrito em {args.out}")
    else:
        print(data, end="")
    return EXIT_COMPATIBLE


def cmd_check(args: argparse.Namespace) -> int:
    profile = _load_profile(args)
    project = Path(args.project)

    if args.advanced:
        print("modo avancado: filtro desativado (sob sua responsabilidade)")
        return EXIT_COMPATIBLE

    req = _load_requirements(project)
    result = classify(req, profile)

    print(f"dispositivo: {profile.device_id} ({profile.soc}, "
          f"{profile.ram_mb} MB, {profile.gpu_model})")
    print(f"requisitos : {req.source}")
    print(result.summary())
    for warning in result.warnings:
        print(f"  ! {warning}")

    if result.verdict is Verdict.INCOMPATIBLE:
        return EXIT_INCOMPATIBLE
    if result.verdict is Verdict.PARTIAL:
        return EXIT_PARTIAL
    return EXIT_COMPATIBLE


def cmd_build(args: argparse.Namespace) -> int:
    project = Path(args.project)
    try:
        metadata = json.loads((project / "metadata.json").read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise PackageError(f"metadata.json ilegivel: {exc}") from exc

    result = build_package(project, args.out,
                           allow_proton=not args.no_proton)
    print(f"pacote: {result.package} ({result.size_bytes} bytes)")
    print(f"sha256: {result.sha256}")

    if args.key:
        sig, manifest = sign_package(result.package, args.key,
                                     version=str(metadata.get("version", "0")))
        print(f"assinatura: {sig}")
        print(f"manifesto : {manifest}")
    return EXIT_COMPATIBLE


def cmd_store(args: argparse.Namespace) -> int:
    """Le uma pasta de projetos baixados e produz `.giochi` para os validos.

    Roda no PC: o rootfs do console nao tem toolchain e o tar do BusyBox nao
    aceita as opcoes que o formato exige (AGENTS.md 3.12).
    """
    profile = None
    if args.device:
        try:
            profile = MANUAL_PROFILES[args.device]
        except KeyError as exc:
            raise ProfileError(f"aparelho desconhecido: {args.device}") from exc

    src = Path(args.src)
    if not src.is_dir():
        print(f"store: pasta de origem inexistente: {src}", file=sys.stderr)
        return EXIT_ERROR

    report = store_mod.run(src, args.out, key=args.key, profile=profile,
                           dry_run=args.dry_run)
    if args.device and profile is not None:
        print(f"dispositivo: {profile.device_id} ({profile.ram_mb} MB)")
    print(store_mod.render(report))

    if args.install:
        try:
            copiados = store_mod.install(report, args.install)
        except PackageError as exc:
            print(f"store: {exc}", file=sys.stderr)
            return EXIT_ERROR
        print(f"  {copiados} pacote(s) copiado(s) para {args.install}")

    return EXIT_ERROR if report.has_errors else EXIT_COMPATIBLE


def cmd_keys(args: argparse.Namespace) -> int:
    priv, pub = generate_keypair(args.out)
    print(f"chave privada: {priv}")
    print(f"chave publica: {pub} (instalar em /etc/giochi/pubkey.ed25519)")
    return EXIT_COMPATIBLE


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="giochi-forge",
        description="Giochi Konsol OS v1.2 — deteccao, classificacao e empacotamento",
    )
    parser.add_argument("--version", action="version", version=f"giochi-forge {__version__}")
    sub = parser.add_subparsers(dest="command", required=True)

    p_profile = sub.add_parser("profile", help="emite um device_profile.json manual")
    p_profile.add_argument("--device", default="rk3566-2gb", choices=sorted(MANUAL_PROFILES))
    p_profile.add_argument("--out")
    p_profile.set_defaults(func=cmd_profile)

    p_check = sub.add_parser("check", help="classifica compatibilidade")
    p_check.add_argument("--profile")
    p_check.add_argument("--device")
    p_check.add_argument("--project", default=".")
    p_check.add_argument("--advanced", action="store_true",
                         help="ignora o filtro (com aviso)")
    p_check.set_defaults(func=cmd_check)

    p_build = sub.add_parser("build", help="empacota (e opcionalmente assina) .giochi")
    p_build.add_argument("--project", default=".")
    p_build.add_argument("--out", required=True)
    p_build.add_argument("--key", help="chave privada Ed25519 (PEM)")
    p_build.add_argument("--no-proton", action="store_true",
                         help="recusa runtime=proton")
    p_build.set_defaults(func=cmd_build)

    p_store = sub.add_parser(
        "store", help="le uma pasta de projetos baixados e produz .giochi")
    p_store.add_argument("--src", required=True,
                         help="pasta com os projetos (um subdiretorio por jogo)")
    p_store.add_argument("--out", required=True, help="onde gravar os .giochi")
    p_store.add_argument("--key", help="chave privada Ed25519 (PEM)")
    p_store.add_argument("--device", choices=sorted(MANUAL_PROFILES),
                         help="classifica contra este aparelho")
    p_store.add_argument("--install", metavar="DIR",
                         help="copia os .giochi gerados para DIR (ex.: /media/user/games)")
    p_store.add_argument("--dry-run", action="store_true",
                         help="so relata, nao escreve pacote")
    p_store.set_defaults(func=cmd_store)

    p_keys = sub.add_parser("keys", help="gera par Ed25519 de assinatura")
    p_keys.add_argument("--out", required=True, help="diretorio de saida")
    p_keys.set_defaults(func=cmd_keys)

    return parser


def _saida_utf8() -> None:
    """Forca UTF-8 em stdout/stderr.

    ## Por que isto e necessario, e nao "detalhe de Windows"

    No Windows, quando a saida e um **pipe** (redirecionamento, `| tee`, o CI),
    o Python usa o encoding do locale — `cp1252` num Windows pt-BR/en-US — e nao
    UTF-8. O resultado medido:

        $ PYTHONIOENCODING=cp1252 giochi-forge check --device x55 --project ...
        Compat?vel          <- o "i" acentuado virou um byte cp1252

    Isso quebrou o job de Windows do `pc-apps.yml`: o passo comparava a saida com
    "Compatível" em UTF-8 e nao achava. Mas o defeito e do produto antes de ser
    do CI — o usuario no Windows leria `Compat?vel`.

    `reconfigure` existe desde o Python 3.7; o piso declarado e 3.9.
    `errors="replace"` em vez de deixar estourar: um caracter estranho num nome
    de jogo nao pode derrubar a ferramenta.
    """
    for fluxo in (sys.stdout, sys.stderr):
        if hasattr(fluxo, "reconfigure"):
            try:
                fluxo.reconfigure(encoding="utf-8", errors="replace")
            except (ValueError, OSError):  # stdout fechado, por exemplo
                pass


def main(argv: list[str] | None = None) -> int:
    _saida_utf8()
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return int(args.func(args))
    except (ProfileError, PackageError) as exc:
        print(f"giochi-forge: {exc}", file=sys.stderr)
        return EXIT_ERROR


if __name__ == "__main__":
    raise SystemExit(main())
