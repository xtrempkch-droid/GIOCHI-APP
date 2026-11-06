"""Testes do servidor HTTP do painel.

Sobem um servidor de verdade em porta efêmera (`criar(0)`) e falam com ele por
`urllib`. Testar o roteamento com `Handler` chamado à mão provaria que os
métodos existem, não que uma requisição HTTP funciona — e o que pode quebrar
aqui é justamente o caminho: cabeçalho `Host`, `..` nos estáticos, JSON malformado.
"""

from __future__ import annotations

import json
import threading
from pathlib import Path
import unittest
import urllib.error
import urllib.request

from giochi_painel import servidor

# Caminho absoluto de propósito: o painel resolve relativo ao cwd do processo, e
# o cwd do teste é `app/painel/src`. Usar relativo aqui testaria a localização
# do arquivo de teste, não o classificador.
RAIZ = Path(__file__).resolve().parents[2]
NATIVO = str(RAIZ / "tests" / "fixtures" / "app-nativo")
GODOT = str(RAIZ / "tests" / "fixtures" / "projeto-godot")


class ServidorDeTeste(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.httpd = servidor.criar(0)
        cls.porta = cls.httpd.server_address[1]
        cls.thread = threading.Thread(target=cls.httpd.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.httpd.shutdown()
        cls.httpd.server_close()
        cls.thread.join(timeout=5)

    def url(self, caminho: str) -> str:
        return f"http://127.0.0.1:{self.porta}{caminho}"

    def get(self, caminho: str, cabecalhos: dict[str, str] | None = None):
        pedido = urllib.request.Request(self.url(caminho), headers=cabecalhos or {})
        return urllib.request.urlopen(pedido, timeout=10)

    def post(self, caminho: str, corpo: dict | bytes):
        dados = corpo if isinstance(corpo, bytes) else json.dumps(corpo).encode("utf-8")
        pedido = urllib.request.Request(
            self.url(caminho),
            data=dados,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        return urllib.request.urlopen(pedido, timeout=30)


class TestPagina(ServidorDeTeste):
    def test_raiz_serve_a_pagina(self):
        with self.get("/") as r:
            corpo = r.read().decode("utf-8")
        self.assertEqual(r.status, 200)
        self.assertIn("Giochi", corpo)

    def test_estaticos_sao_servidos(self):
        for nome in ("app.js", "estilo.css"):
            with self.get("/" + nome) as r:
                self.assertEqual(r.status, 200, nome)
                self.assertGreater(len(r.read()), 0, nome)

    def test_traversal_nao_escapa_do_diretorio(self):
        # `/../../etc/passwd` é normalizado pelo cliente HTTP; o que importa é
        # o caminho codificado, que chega ao servidor como está.
        for caminho in ("/%2e%2e/%2e%2e/etc/passwd", "/..%2f..%2fetc%2fpasswd"):
            with self.assertRaises(urllib.error.HTTPError) as ctx:
                self.get(caminho)
            self.assertEqual(ctx.exception.code, 404, caminho)
            ctx.exception.close()

    def test_arquivo_desconhecido_da_404(self):
        with self.assertRaises(urllib.error.HTTPError) as ctx:
            self.get("/nao-existe.js")
        self.assertEqual(ctx.exception.code, 404)
        ctx.exception.close()


class TestHostHeader(ServidorDeTeste):
    def test_host_estrangeiro_e_recusado(self):
        # Servidor local sem esta checagem é alvo de DNS rebinding: uma página
        # qualquer resolve um domínio para 127.0.0.1 e fala com o painel.
        with self.assertRaises(urllib.error.HTTPError) as ctx:
            self.get("/", {"Host": "atacante.example"})
        self.assertEqual(ctx.exception.code, 403)
        ctx.exception.close()

    def test_localhost_e_aceito(self):
        with self.get("/", {"Host": f"localhost:{self.porta}"}) as r:
            self.assertEqual(r.status, 200)


class TestApi(ServidorDeTeste):
    def test_aparelhos(self):
        with self.get("/api/aparelhos") as r:
            dados = json.load(r)
        ids = {a["id"] for a in dados["aparelhos"]}
        self.assertIn("x55", ids)

    def test_classificar_compativel(self):
        with self.post(
            "/api/classificar",
            {"projeto": NATIVO, "aparelho": "x55"},
        ) as r:
            dados = json.load(r)
        self.assertTrue(dados["permitido"], dados.get("problemas"))

    def test_classificar_incompativel_devolve_200_nao_erro(self):
        # "Incompatível" é resposta, não falha: 200 com o veredito. Reservar
        # erro HTTP para isso faria a UI tratar veredito como pane.
        with self.post(
            "/api/classificar", {"projeto": GODOT, "aparelho": "x55"}
        ) as r:
            self.assertEqual(r.status, 200)
            dados = json.load(r)
        self.assertFalse(dados["permitido"])

    def test_projeto_inexistente_devolve_400(self):
        with self.assertRaises(urllib.error.HTTPError) as ctx:
            self.post("/api/classificar", {"projeto": "/nao/existe", "aparelho": "x55"})
        self.assertEqual(ctx.exception.code, 400)
        ctx.exception.close()

    def test_campo_faltando_devolve_400(self):
        with self.assertRaises(urllib.error.HTTPError) as ctx:
            self.post("/api/classificar", {"projeto": NATIVO})
        self.assertEqual(ctx.exception.code, 400)
        ctx.exception.close()

    def test_json_malformado_devolve_400_nao_500(self):
        with self.assertRaises(urllib.error.HTTPError) as ctx:
            self.post("/api/classificar", b"{isso nao e json")
        self.assertEqual(ctx.exception.code, 400)
        ctx.exception.close()

    def test_rota_desconhecida_devolve_404(self):
        with self.assertRaises(urllib.error.HTTPError) as ctx:
            self.post("/api/nao-existe", {})
        self.assertEqual(ctx.exception.code, 404)
        ctx.exception.close()


if __name__ == "__main__":
    unittest.main()
