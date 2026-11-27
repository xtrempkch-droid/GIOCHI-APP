"""Linha de comando da Loja (lado PC)."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
from pathlib import Path

from . import fonte
from . import sideload as sideload_mod
from .catalogo import catalogo, novidades, sha256_de, varrer
from .destino import bytes_legivel, descobrir, espaco_livre, games_de, resumo, todos


class SemEspaco(Exception):
    """A particao de destino nao tem espaco para o que se quer copiar."""


def escrever(destino: Path, nome: str, doc: dict) -> Path:
    destino.mkdir(parents=True, exist_ok=True)
    alvo = destino / nome
    alvo.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n",
                    encoding="utf-8")
    return alvo


class SemEscrita(Exception):
    """O destino existe mas não aceita escrita deste usuário."""


def _verificar_escrita(games: Path) -> None:
    """Testa a escrita de verdade ANTES de copiar 40 MB.

    A partição `/user` do cartão é ext4 e o udisks monta como ROOT; a do MTP é
    do gvfs e pode vir somente-leitura. Nos dois casos a primeira tentativa
    estourava `PermissionError` com traceback — que não diz o que fazer.
    Escrever um arquivo vazio responde a mesma pergunta em um syscall.
    """
    try:
        games.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        raise SemEscrita(f"nao consegui criar '{games}': {exc.strerror}") from exc

    teste = games / ".giochi-store-teste"
    try:
        teste.write_bytes(b"")
    except OSError as exc:
        raise SemEscrita(
            f"'{games}' nao aceita escrita ({exc.strerror})."
            "\n  cartao montado pelo udisks e' do root:"
            f"\n      sudo chown -R $USER '{games.parent}'"
            "\n  ou use o MTP (o gvfs monta com o seu usuario)") from exc
    finally:
        try:
            teste.unlink(missing_ok=True)
        except OSError:
            pass


def instalar(pacotes, games: Path, dry_run: bool,
             aceitar_sideload: bool = False) -> tuple[int, int]:
    """Copia os pacotes validos para `games`, conferindo o sha256 no DESTINO.

    Conferir na origem nao serve: copiar e confiar no `shutil.copy` nao pega
    cartao que grava lixo — o mesmo motivo do tools/verify-card.py (§3.42).

    ## Sideload tem de ser uma decisao visivel

    Pacote sem `.sig` de 64 bytes e' **sideload**: nao ha' o que verificar no
    aparelho, e o conteudo pode ser qualquer coisa. Antes disto ele era copiado
    como se fosse da loja — o mesmo selo, sem a mesma garantia. Agora ele e'
    pulado com o motivo, e so' entra com `--aceitar-sideload`, que e' a
    confirmacao explicita de quem copia. O PortMaster e' o caso concreto
    (issue #2, pedido 3).

    ## O espaco e' conferido antes, na particao do destino

    Copiar 40 MB para uma p3 cheia enche o cartao no meio do arquivo e deixa um
    `.giochi` truncado com o mesmo nome — pior que nao copiar. Perguntar antes
    custa um `statfs`.
    """
    if not dry_run:
        _verificar_escrita(games)

    # O que realmente vai ser copiado: valido e, se for sideload, aceito.
    a_copiar = [
        p for p in pacotes
        if p.ok and (p.assinado or aceitar_sideload)
    ]
    necessario = sum(p.tamanho for p in a_copiar)

    livre, total = espaco_livre(games)
    print(f"espaco em {games}: {bytes_legivel(livre)} livres "
          f"de {bytes_legivel(total)} (particao de dados, nao o rootfs)")
    if necessario > livre:
        raise SemEspaco(
            f"nao cabe: {bytes_legivel(necessario)} a copiar, "
            f"{bytes_legivel(livre)} livres em {games}")

    copiados = falhas = 0

    if not dry_run:
        games.mkdir(parents=True, exist_ok=True)

    for p in pacotes:
        if not p.ok:
            falhas += 1
            print(f"  pulado   {p.caminho.name:32s} {p.motivo}")
            continue
        if not p.assinado and not aceitar_sideload:
            falhas += 1
            print(f"  SIDELOAD {p.caminho.name:32s} sem .sig — "
                  "nao instala sem --aceitar-sideload")
            continue
        if dry_run:
            print(f"  copiaria {p.caminho.name:32s} {p.tamanho // 1024} KB")
            copiados += 1
            continue

        alvo = games / p.caminho.name
        # copyfile, NAO copy2: o copy2 tenta replicar metadados (utime) e o
        # MTP responde `OSError: [Errno 95] Operation not supported` — o
        # conteudo ja' foi escrito e o comando estoura no fim, deixando o
        # pacote no cartao E um traceback na cara do usuario. Medido em
        # 2026-11-01 com o console ligado pelo cabo.
        # O sha256 conferido logo abaixo e' o que realmente importa: se o
        # conteudo chegou inteiro, os atributos sao irrelevantes.
        try:
            shutil.copyfile(p.caminho, alvo)
        except OSError as exc:
            falhas += 1
            print(f"  ERRO     {p.caminho.name:32s} {exc.strerror or exc}")
            continue
        if sha256_de(alvo) != p.sha256:
            alvo.unlink(missing_ok=True)
            falhas += 1
            print(f"  ERRO     {p.caminho.name:32s} copia nao bate o sha256")
            continue
        copiados += 1
        print(f"  copiado  {p.caminho.name:32s} {p.tamanho // 1024} KB")

    return copiados, falhas


def cmd_catalogo(args) -> int:
    origem = Path(args.src).expanduser()
    if not origem.is_dir():
        print(f"giochi-store: '{origem}' nao e' um diretorio", file=sys.stderr)
        return 2

    pacotes = varrer(origem)
    manifesto = (Path(args.sideload).expanduser() if args.sideload
                 else sideload_mod.caminho_padrao(origem))
    try:
        terceiros = sideload_mod.ler(manifesto)
    except sideload_mod.SideloadError as exc:
        print(f"giochi-store: {exc}", file=sys.stderr)
        return 2

    cat = catalogo(pacotes, terceiros)
    novo = novidades(pacotes)

    sem_sig = [p for p in pacotes if p.ok and not p.assinado]
    if args.dry_run:
        print(f"(dry-run) {len(pacotes)} pacote(s) em {origem}")
        for p in pacotes:
            marca = "ok    " if p.ok and p.assinado else (
                "sidel." if p.ok else "recusa")
            print(f"  {marca} {p.caminho.name:30s} {p.motivo or p.titulo}")
        for s in terceiros:
            print(f"  terc.  {s['id']:30s} {s['titulo']}")
        return 0

    destino = Path(args.out).expanduser()
    c = escrever(destino, "store.json", cat)
    n = escrever(destino, "news.json", novo)
    print(f"{len(cat['itens'])} de {len(pacotes)} pacote(s) no catalogo")
    if sem_sig:
        # O numero tem de aparecer: um `.giochi` sem `.sig` no catalogo e' o
        # caminho pelo qual a interface mostraria sideload como conteudo.
        print(f"  atencao: {len(sem_sig)} pacote(s) sem .sig — "
              "nao passam pelo `instalar` sem --aceitar-sideload")
    if terceiros:
        print(f"  {len(terceiros)} artefato(s) de terceiro em 'sideload' "
              "(marcados como nao assinados)")
    print(f"  {c}")
    print(f"  {n}")

    for p in pacotes:
        if not p.ok:
            print(f"  recusado {p.caminho.name:30s} {p.motivo}")
    return 0


def cmd_instalar(args) -> int:
    pacotes = varrer(Path(args.src).expanduser())
    if not pacotes:
        print(f"giochi-store: nenhum .giochi em '{args.src}'", file=sys.stderr)
        return 2

    # Sem --games, descobre o destino: MTP (console no cabo) ou cartao no
    # leitor. Pedir o caminho na mao significava copiar para o lugar errado
    # sem erro nenhum quando o usuario errava o /media/<user>/<label>.
    if args.games:
        games = Path(args.games).expanduser()
        print(f"destino: {games}")
    else:
        destinos = todos()
        if not destinos:
            print(f"giochi-store: {resumo()}", file=sys.stderr)
            return 2
        alvo = descobrir(preferir=args.preferir)
        if alvo is None:
            print(f"giochi-store: nenhum destino do tipo '{args.preferir}'."
                  f"\n{resumo()}", file=sys.stderr)
            return 2
        if len(destinos) > 1:
            print("destinos encontrados:")
            for d in destinos:
                marca = "->" if d.caminho == alvo.caminho else "  "
                print(f" {marca} {d}")
            print(f"   usando o primeiro; use --preferir cartao|mtp para escolher")
        games = games_de(alvo)
        print(f"destino: {games}")

    try:
        copiados, falhas = instalar(pacotes, games, args.dry_run,
                                    aceitar_sideload=args.aceitar_sideload)
    except SemEscrita as exc:
        print(f"giochi-store: {exc}", file=sys.stderr)
        return 3
    except SemEspaco as exc:
        print(f"giochi-store: {exc}", file=sys.stderr)
        return 3

    if args.dry_run:
        print(f"{copiados} seria(m) copiado(s), {falhas} recusado(s)")
    else:
        print(f"{copiados} instalado(s), {falhas} recusado(s)")
    return 0 if falhas == 0 else 1


def cmd_espaco(args) -> int:
    """Quanto cabe em `/user` — a pergunta que `df /` responde errado.

    Desde 26/11/2026 `/user` e' a p3 (`GIOCHI_USER`), um device diferente do
    rootfs. Este comando segue o **destino** (cartao, MTP ou `--games`) e
    imprime o numero da particao certa. E' o pedido 3 da issue #8, do lado do
    PC, onde quem instala precisa saber se o app cabe antes de copiar.
    """
    if args.games:
        games = Path(args.games).expanduser()
    else:
        alvo = descobrir(preferir=args.preferir)
        if alvo is None:
            print(f"giochi-store: {resumo()}", file=sys.stderr)
            return 2
        games = games_de(alvo)
        print(f"destino: {alvo}")

    livre, total = espaco_livre(games)
    usado = total - livre
    pct = (usado * 100 / total) if total else 0
    print(f"{games}")
    print(f"  livre: {bytes_legivel(livre)}")
    print(f"  usado: {bytes_legivel(usado)} de {bytes_legivel(total)} ({pct:.0f}%)")
    print("  (particao de dados em /user, nao o rootfs)")
    return 0


def cmd_destinos(args) -> int:
    """Mostra onde o console esta' visivel e qual seria usado."""
    achados = todos()
    if not achados:
        print(resumo(), file=sys.stderr)
        return 1
    alvo = descobrir(preferir=args.preferir)
    for d in achados:
        marca = "*" if alvo and d.caminho == alvo.caminho else " "
        print(f" {marca} {d}  (games: {games_de(d)})")
    return 0


def cmd_obter(args) -> int:
    """Le a lista (apps.json do GithubLauncher) e resolve o build aarch64.

    Isto e' o que faltava entre "baixei um projeto do GitHub" e "tenho um
    .giochi": o GitHub Releases diz o nome de cada arquivo, entao o build de
    aarch64 pode ser escolhido -- e conferido no ELF depois do download.
    """
    try:
        entradas = fonte.ler_apps_json(Path(args.apps_json).expanduser())
    except fonte.FonteError as exc:
        print(f"giochi-store: {exc}", file=sys.stderr)
        return 2
    if not entradas:
        print("giochi-store: lista sem apps", file=sys.stderr)
        return 2

    token = args.token or os.environ.get("GITHUB_TOKEN")
    resultados = fonte.resolver(entradas, token=token)
    print(f"lista: {args.apps_json}  ({len(entradas)} app(s))")
    print(fonte.relatorio(resultados))

    falhas = 0
    if args.baixar:
        destino = Path(args.out or ".").expanduser()
        baixados = 0
        for r in resultados:
            if not r.ok:
                continue
            try:
                caminho, elf_ok, detalhe = fonte.baixar(r, destino, token=token)
            except fonte.FonteError as exc:
                print(f"  erro    {r.entrada.nome:26s} {exc}", file=sys.stderr)
                falhas += 1
                continue
            estado = "ELF ok " if elf_ok else "conferir"
            print(f"  {estado} {caminho.name:26s} {detalhe}")
            baixados += 1
        print(f"  {baixados} arquivo(s) em {destino}")

    return 0 if all(r.ok for r in resultados) and not falhas else 1


def cmd_listar(args) -> int:
    pacotes = varrer(Path(args.src).expanduser())
    if args.json:
        print(json.dumps(catalogo(pacotes), indent=2, ensure_ascii=False))
        return 0
    if not pacotes:
        print(f"nenhum .giochi em '{args.src}'", file=sys.stderr)
        return 0
    for p in pacotes:
        if p.ok:
            # `origem` e' o que impede um pacote sem `.sig` de aparecer com o
            # mesmo selo de um pacote da loja (issue #2, pedido 3).
            print(f"  {p.titulo:28s} {str(p.meta.get('version', '')):8s} "
                  f"{str(p.meta.get('runtime', '')):10s} "
                  f"{p.tamanho // 1024:6d} KB  "
                  f"{'loja' if p.assinado else 'SIDELOAD (sem .sig)'}")
        else:
            print(f"  {p.caminho.name:28s} RECUSADO: {p.motivo}")
    return 0


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
    ap = argparse.ArgumentParser(
        prog="giochi-store",
        description="Catalogo e instalacao da Loja (lado PC). "
                    "O console nao empacota nem assina nada.")
    sub = ap.add_subparsers(dest="cmd", required=True)

    c = sub.add_parser("catalogo", help="gera store.json + news.json")
    c.add_argument("--src", required=True, help="pasta com os .giochi")
    c.add_argument("--out", required=True,
                   help="destino (ex.: /media/eu/GIOCHI_USER/.cache)")
    c.add_argument("--sideload", default=None,
                   help="manifesto de artefatos de terceiro "
                        "(padrao: sideload.json ao lado dos pacotes)")
    c.add_argument("--dry-run", action="store_true")
    c.set_defaults(func=cmd_catalogo)

    i = sub.add_parser("instalar", help="copia para GIOCHI_USER/games")
    i.add_argument("--src", required=True)
    i.add_argument("--games", default=None,
                   help="caminho explicito; sem ele, descobre cartao ou MTP")
    i.add_argument("--preferir", choices=("cartao", "mtp"), default=None,
                   help="com dois destinos, qual usar")
    i.add_argument("--aceitar-sideload", action="store_true",
                   help="copia tambem pacote sem .sig (sideload explicito)")
    i.add_argument("--dry-run", action="store_true")
    i.set_defaults(func=cmd_instalar)

    e = sub.add_parser("espaco",
                       help="espaco livre em /user (a particao de dados)")
    e.add_argument("--games", default=None,
                   help="caminho explicito; sem ele, descobre cartao ou MTP")
    e.add_argument("--preferir", choices=("cartao", "mtp"), default=None)
    e.set_defaults(func=cmd_espaco)

    d = sub.add_parser("destinos", help="onde o console esta' visivel")
    d.add_argument("--preferir", choices=("cartao", "mtp"), default=None)
    d.set_defaults(func=cmd_destinos)

    o = sub.add_parser(
        "obter", help="resolve o build aarch64 de uma lista de repositorios")
    o.add_argument("--apps-json", required=True,
                   help="lista no formato do GithubLauncher, ou lista de owner/repo")
    o.add_argument("--out", help="onde gravar os arquivos baixados (com --baixar)")
    o.add_argument("--baixar", action="store_true",
                   help="baixa de verdade (sem isto, so' mostra o que escolheria)")
    o.add_argument("--token", help="token do GitHub (ou GITHUB_TOKEN)")
    o.set_defaults(func=cmd_obter)

    l = sub.add_parser("listar", help="mostra o catalogo em texto")
    l.add_argument("--src", required=True)
    l.add_argument("--json", action="store_true")
    l.set_defaults(func=cmd_listar)

    args = ap.parse_args(argv)
    return args.func(args)
