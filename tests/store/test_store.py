"""Testes do store do PC: sideload honesto e espaco em `/user`.

## O que estes testes protegem

1. **sideload tem de se declarar.** Pacote sem `.sig` de 64 bytes nao pode
   aparecer com o mesmo selo de um pacote da loja — e' o pedido 3 da issue #2,
   e o caso concreto e' o PortMaster (GPL dentro, `.zip` sem assinatura).
2. **sideload nao se instala sozinho.** Sem `--aceitar-sideload`, o pacote e'
   pulado com o motivo; copiar em silencio e' o defeito, nao o comportamento.
3. **espaco e' lido no destino, nao em `/`.** Desde 26/11/2026 `/user` e' a p3
   (`GIOCHI_USER`), outro device; a pergunta certa e' a da particao do caminho
   (issue #8, pedido 3).
4. **manifesto de terceiro invalido e' recusado com motivo**, nunca ignorado.

Os pacotes sao montados aqui com `tarfile` (stdlib), sem o forge: assim o teste
do store roda sozinho e mede o **leitor**, que e' o que ele usa.
"""

from __future__ import annotations

import io
import json
import tarfile
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

from giochi_store import cli, destino, sideload
from giochi_store.catalogo import (
    ORIGEM_LOJA,
    ORIGEM_SIDELOAD,
    catalogo,
    tem_assinatura,
    validar,
    varrer,
)


def _metadata(titulo: str = "App de Teste", **extra) -> dict:
    meta = {
        "title": titulo,
        "developer": "Teste",
        "version": "1.0.0",
        "arch": "aarch64",
        "runtime": "native",
        "entrypoint": "bin/app",
    }
    meta.update(extra)
    return meta


def _pacote(pasta: Path, nome: str = "app-teste",
            assinatura: bytes | None = None, **extra) -> Path:
    """Um `.giochi` minimo, com `metadata.json` como PRIMEIRO membro (ADR-11)."""
    alvo = pasta / f"{nome}.giochi"
    dados = json.dumps(_metadata(**extra)).encode("utf-8")
    with tarfile.open(alvo, "w") as tar:
        info = tarfile.TarInfo("metadata.json")
        info.size = len(dados)
        tar.addfile(info, io.BytesIO(dados))
        binario = b"#!/bin/sh\necho oi\n"
        info = tarfile.TarInfo("bin/app")
        info.size = len(binario)
        tar.addfile(info, io.BytesIO(binario))
    if assinatura is not None:
        Path(f"{alvo}.sig").write_bytes(assinatura)
    return alvo


class TestAssinatura(unittest.TestCase):
    def test_sem_sig_e_sideload(self):
        with tempfile.TemporaryDirectory() as tmp:
            caminho = _pacote(Path(tmp))
            self.assertFalse(tem_assinatura(caminho))
            self.assertEqual(validar(caminho).origem, ORIGEM_SIDELOAD)

    def test_sig_de_64_bytes_e_loja(self):
        with tempfile.TemporaryDirectory() as tmp:
            caminho = _pacote(Path(tmp), assinatura=b"\x00" * 64)
            self.assertTrue(tem_assinatura(caminho))
            self.assertEqual(validar(caminho).origem, ORIGEM_LOJA)

    def test_sig_em_base64_nao_conta_como_assinado(self):
        # O formato que ja' divergiu: base64 de 64 bytes da' 89 caracteres.
        # Aceitar por extensao faria um `.sig` de texto passar por assinatura.
        with tempfile.TemporaryDirectory() as tmp:
            caminho = _pacote(Path(tmp), assinatura=b"A" * 89)
            self.assertFalse(tem_assinatura(caminho))

    def test_catalogo_marca_origem_por_item(self):
        with tempfile.TemporaryDirectory() as tmp:
            pasta = Path(tmp)
            _pacote(pasta, "com-sig", assinatura=b"\x00" * 64, title="Da Loja")
            _pacote(pasta, "sem-sig", title="De Fora")
            doc = catalogo(varrer(pasta))
            origens = {i["id"]: i["origem"] for i in doc["itens"]}
            self.assertEqual(origens, {"com-sig": "loja", "sem-sig": "sideload"})
            self.assertTrue(all(i["assinado"] == (i["origem"] == "loja")
                                for i in doc["itens"]))


class TestManifestoSideload(unittest.TestCase):
    def _item(self, **extra) -> dict:
        item = {
            "id": "portmaster",
            "titulo": "PortMaster",
            "developer": "PortsMaster (comunidade)",
            "url": "https://github.com/PortsMaster/PortMaster-New/releases/x.zip",
            "sha256": "a" * 64,
            "tamanho_bytes": 25800000,
            "destino": "ports",
            "licenca": "GPL-2.0 / LGPL-2.1 dentro",
            "aviso": "codigo de terceiro sem assinatura; roda em sandbox",
        }
        item.update(extra)
        return item

    def test_ausente_nao_e_erro(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(sideload.ler(Path(tmp) / "sideload.json"), [])

    def test_item_valido_sai_marcado(self):
        with tempfile.TemporaryDirectory() as tmp:
            arquivo = Path(tmp) / "sideload.json"
            arquivo.write_text(json.dumps({
                "schema_version": "1.0",
                "itens": [self._item(), self._item(id="aaa", titulo="A")],
            }), encoding="utf-8")
            itens = sideload.ler(arquivo)
            self.assertEqual([i["id"] for i in itens], ["aaa", "portmaster"])
            self.assertTrue(all(i["origem"] == "sideload" for i in itens))
            self.assertTrue(all(i["assinado"] is False for i in itens))

    def test_http_e_recusado(self):
        with tempfile.TemporaryDirectory() as tmp:
            arquivo = Path(tmp) / "sideload.json"
            arquivo.write_text(json.dumps({
                "schema_version": "1.0",
                "itens": [self._item(url="http://exemplo/x.zip")],
            }), encoding="utf-8")
            with self.assertRaises(sideload.SideloadError) as ctx:
                sideload.ler(arquivo)
            self.assertIn("https", str(ctx.exception))

    def test_sem_hash_e_recusado(self):
        with tempfile.TemporaryDirectory() as tmp:
            arquivo = Path(tmp) / "sideload.json"
            arquivo.write_text(json.dumps({
                "schema_version": "1.0",
                "itens": [self._item(sha256="a" * 40)],
            }), encoding="utf-8")
            with self.assertRaises(sideload.SideloadError):
                sideload.ler(arquivo)

    def test_campo_desconhecido_e_recusado(self):
        # `sha` no lugar de `sha256` seria ignorado em silencio: o item passaria
        # sem hash, que e' o que o manifesto existe para impedir.
        with tempfile.TemporaryDirectory() as tmp:
            arquivo = Path(tmp) / "sideload.json"
            item = self._item()
            del item["sha256"]
            item["sha"] = "a" * 64
            arquivo.write_text(json.dumps({
                "schema_version": "1.0", "itens": [item],
            }), encoding="utf-8")
            with self.assertRaises(sideload.SideloadError) as ctx:
                sideload.ler(arquivo)
            self.assertIn("sha", str(ctx.exception))

    def test_aviso_vazio_e_recusado(self):
        # Sem `aviso` a interface nao teria o que dizer; o item apareceria com o
        # mesmo selo dos assinados.
        with tempfile.TemporaryDirectory() as tmp:
            arquivo = Path(tmp) / "sideload.json"
            arquivo.write_text(json.dumps({
                "schema_version": "1.0",
                "itens": [self._item(aviso="   ")],
            }), encoding="utf-8")
            with self.assertRaises(sideload.SideloadError):
                sideload.ler(arquivo)

    def test_id_repetido_e_recusado(self):
        with tempfile.TemporaryDirectory() as tmp:
            arquivo = Path(tmp) / "sideload.json"
            arquivo.write_text(json.dumps({
                "schema_version": "1.0",
                "itens": [self._item(), self._item()],
            }), encoding="utf-8")
            with self.assertRaises(sideload.SideloadError):
                sideload.ler(arquivo)


class TestEspaco(unittest.TestCase):
    def test_le_a_particao_do_caminho(self):
        with tempfile.TemporaryDirectory() as tmp:
            livre, total = destino.espaco_livre(Path(tmp))
            self.assertGreater(total, 0)
            self.assertGreaterEqual(livre, 0)
            self.assertLessEqual(livre, total)

    def test_caminho_que_ainda_nao_existe_responde_pelo_ancestral(self):
        # `--dry-run` nao cria a pasta de destino, e `disk_usage` exige que o
        # caminho exista. O ancestral responde pela mesma particao.
        with tempfile.TemporaryDirectory() as tmp:
            livre, total = destino.espaco_livre(Path(tmp) / "nao" / "existe")
            self.assertGreater(total, 0)
            self.assertLessEqual(livre, total)

    def test_bytes_legivel(self):
        self.assertEqual(destino.bytes_legivel(512), "512 B")
        self.assertEqual(destino.bytes_legivel(1536), "1.5 KiB")
        self.assertEqual(destino.bytes_legivel(40 * 1024 * 1024), "40.0 MiB")


class TestInstalar(unittest.TestCase):
    def _saida(self, funcao, *args, **kwargs):
        buffer = io.StringIO()
        with redirect_stdout(buffer):
            resultado = funcao(*args, **kwargs)
        return resultado, buffer.getvalue()

    def test_sem_aceitar_sideload_o_pacote_e_pulado(self):
        with tempfile.TemporaryDirectory() as tmp:
            origem, jogos = Path(tmp) / "src", Path(tmp) / "user" / "games"
            origem.mkdir()
            _pacote(origem)
            pacotes = varrer(origem)
            (copiados, falhas), saida = self._saida(
                cli.instalar, pacotes, jogos, False)
            self.assertEqual((copiados, falhas), (0, 1))
            self.assertEqual(list(jogos.glob("*.giochi")), [])
            self.assertIn("SIDELOAD", saida)
            self.assertIn("--aceitar-sideload", saida)

    def test_com_aceitar_sideload_copia_e_confere_o_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            origem, jogos = Path(tmp) / "src", Path(tmp) / "user" / "games"
            origem.mkdir()
            _pacote(origem)
            pacotes = varrer(origem)
            (copiados, falhas), saida = self._saida(
                cli.instalar, pacotes, jogos, False, aceitar_sideload=True)
            self.assertEqual((copiados, falhas), (1, 0))
            self.assertEqual(len(list(jogos.glob("*.giochi"))), 1)
            self.assertIn("particao de dados", saida)

    def test_espaco_insuficiente_e_recusado_antes_de_copiar(self):
        with tempfile.TemporaryDirectory() as tmp:
            origem, jogos = Path(tmp) / "src", Path(tmp) / "user" / "games"
            origem.mkdir()
            # assinado: o pacote entraria no destino, e o que barra e' o espaco
            _pacote(origem, assinatura=b"\x00" * 64)
            pacotes = varrer(origem)
            with mock.patch.object(cli, "espaco_livre", return_value=(1, 100)):
                with self.assertRaises(cli.SemEspaco) as ctx:
                    cli.instalar(pacotes, jogos, True)
            self.assertIn("nao cabe", str(ctx.exception))
            self.assertEqual(list(jogos.glob("*.giochi")), [])


if __name__ == "__main__":
    unittest.main()
