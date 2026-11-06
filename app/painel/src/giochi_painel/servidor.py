"""Servidor HTTP local do painel.

## Por que `http.server` e não Flask/FastAPI

A mesma razão pela qual os apps de PC não têm dependência externa: a ferramenta
tem de rodar numa máquina limpa, e `pip install` atrás de proxy é o primeiro
suporte que ninguém quer dar. Aqui são ~200 linhas de stdlib.

## Segurança, porque isto serve uma ferramenta que assina pacotes

Três decisões, e nenhuma é decoração:

1. **Escuta só em 127.0.0.1.** Não há opção de `0.0.0.0`. Um servidor que aceita
   "empacote este caminho" só pode existir onde só o dono da máquina alcança.
2. **`Host` é verificado.** É a defesa contra DNS rebinding: sem isso, uma página
   maliciosa aberta no mesmo navegador pode apontar um domínio dela para
   127.0.0.1 e falar com o painel. Aceitar `Host` que não seja `127.0.0.1` ou
   `localhost` fecha esse caminho.
3. **Escrita só onde o usuário pediu.** O painel não varre o disco: ele trabalha
   nos caminhos que o formulário manda, e `_projeto` recusa pasta sem
   `metadata.json`.
"""

from __future__ import annotations

import json
import mimetypes
import threading
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from importlib import resources
from pathlib import PurePosixPath
from typing import Any
from urllib.parse import parse_qs, urlparse

from . import acoes

# Os estáticos vêm por `importlib.resources`, não por `Path(__file__)`.
# Dentro de um `.pyz` (o pacote de PC) não existe diretório ao lado do módulo:
# `__file__` aponta para dentro do zip e `Path.exists()` devolve False. O
# `resources` lê dos dois lugares, e sem isso o painel empacotado serviria 404
# na própria página — erro que só apareceria depois de distribuir.
ESTATICOS = resources.files(__package__).joinpath("estatico")

HOSTS_ACEITOS = ("127.0.0.1", "localhost", "[::1]")


class Handler(BaseHTTPRequestHandler):
    server_version = "GiochiPainel"
    # Sem log por requisição: o painel é usado por uma pessoa, e o ruído de
    # `GET /api/...` a cada clique cobre a mensagem que importa.
    def log_message(self, fmt: str, *args: Any) -> None:  # noqa: A003
        pass

    # ── utilidades ─────────────────────────────────────────
    def _host_ok(self) -> bool:
        host = (self.headers.get("Host") or "").split(":")[0].strip().lower()
        return host in HOSTS_ACEITOS

    def _json(self, codigo: int, corpo: dict[str, Any]) -> None:
        dados = json.dumps(corpo, ensure_ascii=False).encode("utf-8")
        self.send_response(codigo)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(dados)))
        # O painel nunca deve ser cacheado: o estado é o do disco agora.
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(dados)

    def _arquivo(self, relativo: str) -> None:
        # `..` não pode escapar do diretório estático — é o mesmo cuidado que o
        # validador de `entrypoint` toma com caminhos de app. Aqui a checagem é
        # sobre a string, porque `Traversable.resolve()` não existe no zip.
        partes = [p for p in PurePosixPath(relativo).parts if p not in ("", ".")]
        if not partes or any(p == ".." for p in partes):
            self._json(404, {"erro": "arquivo não encontrado"})
            return
        alvo = ESTATICOS.joinpath(*partes)
        if not alvo.is_file():
            self._json(404, {"erro": "arquivo não encontrado"})
            return
        corpo = alvo.read_bytes()
        tipo = mimetypes.guess_type(partes[-1])[0] or "application/octet-stream"
        if tipo.startswith("text/") or tipo in ("application/javascript", "application/json"):
            tipo += "; charset=utf-8"
        self.send_response(200)
        self.send_header("Content-Type", tipo)
        self.send_header("Content-Length", str(len(corpo)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(corpo)

    def _corpo_json(self) -> dict[str, Any]:
        tamanho = int(self.headers.get("Content-Length") or 0)
        if tamanho <= 0:
            return {}
        if tamanho > 64 * 1024:
            raise acoes.ErroDeAcao("corpo grande demais")
        try:
            return json.loads(self.rfile.read(tamanho).decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise acoes.ErroDeAcao(f"JSON inválido: {exc}") from exc

    # ── rotas ──────────────────────────────────────────────
    def do_GET(self) -> None:  # noqa: N802
        if not self._host_ok():
            self._json(403, {"erro": "Host não aceito"})
            return

        url = urlparse(self.path)
        query = parse_qs(url.query)
        try:
            if url.path in ("/", "/index.html"):
                self._arquivo("index.html")
            elif url.path == "/app.js":
                self._arquivo("app.js")
            elif url.path == "/estilo.css":
                self._arquivo("estilo.css")
            elif url.path == "/api/aparelhos":
                self._json(200, {"aparelhos": acoes.aparelhos()})
            elif url.path == "/api/destinos":
                self._json(200, acoes.destinos())
            elif url.path == "/api/pacotes":
                origem = (query.get("origem") or [""])[0]
                self._json(200, acoes.pacotes(origem))
            else:
                self._json(404, {"erro": f"rota desconhecida: {url.path}"})
        except acoes.ErroDeAcao as exc:
            self._json(400, {"erro": str(exc)})
        except Exception as exc:  # noqa: BLE001 — o painel não pode derrubar
            self._json(500, {"erro": f"{type(exc).__name__}: {exc}"})

    def do_POST(self) -> None:  # noqa: N802
        if not self._host_ok():
            self._json(403, {"erro": "Host não aceito"})
            return

        url = urlparse(self.path)
        try:
            dados = self._corpo_json()
            if url.path == "/api/classificar":
                self._json(200, acoes.classificar(
                    projeto=str(dados.get("projeto", "")),
                    aparelho=str(dados.get("aparelho", "")),
                ))
            elif url.path == "/api/empacotar":
                self._json(200, acoes.empacotar(
                    projeto=str(dados.get("projeto", "")),
                    saida=str(dados.get("saida", "")),
                    chave=(dados.get("chave") or None),
                ))
            else:
                self._json(404, {"erro": f"rota desconhecida: {url.path}"})
        except acoes.ErroDeAcao as exc:
            self._json(400, {"erro": str(exc)})
        except Exception as exc:  # noqa: BLE001
            self._json(500, {"erro": f"{type(exc).__name__}: {exc}"})


def criar(porta: int = 0, host: str = "127.0.0.1") -> ThreadingHTTPServer:
    """Cria o servidor sem servi-lo — o teste precisa da porta escolhida."""
    if host not in HOSTS_ACEITOS:
        raise acoes.ErroDeAcao(
            f"host não permitido: {host} (só {', '.join(HOSTS_ACEITOS)})"
        )
    return ThreadingHTTPServer((host, porta), Handler)


def servir(porta: int = 8710, abrir: bool = True) -> int:
    """Sobe o painel e bloqueia. Devolve 0 quando encerrado."""
    servidor = criar(porta)
    endereco, real = servidor.server_address[:2]
    url = f"http://{endereco}:{real}/"
    print(f"Giochi Painel em {url}")
    print("Ctrl+C para encerrar. Só esta máquina alcança (127.0.0.1).")

    if abrir:
        # Em thread: `webbrowser.open` pode bloquear esperando o navegador, e o
        # servidor precisa estar atendendo antes de a página pedir dados.
        threading.Thread(
            target=lambda: webbrowser.open(url), daemon=True
        ).start()

    try:
        servidor.serve_forever()
    except KeyboardInterrupt:
        print("\nencerrando")
    finally:
        servidor.server_close()
    return 0
