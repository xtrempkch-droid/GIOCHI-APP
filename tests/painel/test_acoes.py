"""Testes das operações do painel, sem abrir socket.

O que se testa aqui é **o que acontece**, não o roteamento: classificar,
empacotar, listar. O `servidor.py` tem os seus próprios testes, e a separação
existe para que uma falha diga de que lado está o defeito.
"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from giochi_forge.package import generate_keypair
from giochi_painel import acoes

RAIZ = Path(__file__).resolve().parents[2]

# Fixtures, não apps publicados.
#
# Estes testes usavam dois apps do repositório. Isso acoplava o teste do painel
# ao catálogo: apagar um app jogava 12 testes para o vermelho sem nenhum defeito
# no painel. A fixture nativa
# (`forge.json` + metadata + capa + binário) e a fixture Godot (`project.godot`)
# exercitam os mesmos caminhos de código sem depender do que está publicado.
NATIVO = RAIZ / "tests" / "fixtures" / "app-nativo"
GODOT = RAIZ / "tests" / "fixtures" / "projeto-godot"


class TestAparelhos(unittest.TestCase):
    def test_lista_os_perfis_reais(self):
        perfis = acoes.aparelhos()
        ids = {p["id"] for p in perfis}
        # Os aparelhos medidos em hardware precisam estar na lista: são eles que
        # dão sentido ao painel. Um perfil inventado aqui reprovaria app bom.
        self.assertIn("x55", ids)
        self.assertIn("x35h", ids)

    def test_mostra_o_display_utilizavel_e_nao_so_o_painel(self):
        # O X55 tem painel 720x1280 e roda paisagem 1280x720. Comparar um jogo
        # 1280x720 com o painel cru o reprovaria — foi um bug real do forge.
        por_id = {p["id"]: p for p in acoes.aparelhos()}
        x55 = por_id["x55"]
        self.assertEqual(x55["display"], "720x1280")
        self.assertEqual(x55["utilizavel"], "1280x720")

    def test_godot_runtime_distingue_ausente_de_nao_medido(self):
        # `False` = medido e não tem; `None` = não medido. Colapsar os dois em
        # "falso" reprovaria app num aparelho que ninguém testou.
        valores = {p["godot_runtime"] for p in acoes.aparelhos()}
        self.assertIn(False, valores)


class TestClassificar(unittest.TestCase):
    def test_volley_e_compativel_com_o_x55(self):
        r = acoes.classificar(str(NATIVO), "x55")
        self.assertTrue(r["permitido"], r["problemas"])
        self.assertEqual(r["aparelho"], "x55")
        self.assertEqual(r["origem"], "forge.json")

    def test_spacewar_e_recusado_onde_nao_ha_runtime_godot(self):
        # O jogo é Godot e o X55 não tem Godot: dizer "compatível" seria a
        # mentira mais cara do projeto, porque o usuário descobriria no aparelho.
        r = acoes.classificar(str(GODOT), "x55")
        self.assertFalse(r["permitido"])
        self.assertTrue(r["problemas"])

    def test_aparelho_desconhecido_e_erro_de_entrada(self):
        with self.assertRaises(acoes.ErroDeAcao):
            acoes.classificar(str(NATIVO), "nao-existe")

    def test_pasta_sem_metadata_nao_e_app(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(acoes.ErroDeAcao):
                acoes.classificar(tmp, "x55")

    def test_pasta_inexistente_e_erro_claro(self):
        with self.assertRaises(acoes.ErroDeAcao) as ctx:
            acoes.classificar("/nao/existe/em/lugar/nenhum", "x55")
        self.assertIn("não encontrada", str(ctx.exception))

    def test_sem_forge_json_nao_classifica_em_silencio(self):
        # Sem requisitos declarados o classificador não tem o que comparar.
        # Devolver "compatível" seria absurdo; o painel precisa dizer isso.
        with tempfile.TemporaryDirectory() as tmp:
            pasta = Path(tmp) / "app"
            pasta.mkdir()
            (pasta / "metadata.json").write_text(
                json.dumps({"title": "X", "version": "1.0.0"}), encoding="utf-8"
            )
            with self.assertRaises(acoes.ErroDeAcao) as ctx:
                acoes.classificar(str(pasta), "x55")
            self.assertIn("forge.json", str(ctx.exception))


class TestEmpacotar(unittest.TestCase):
    def test_empacota_sem_chave(self):
        with tempfile.TemporaryDirectory() as tmp:
            r = acoes.empacotar(str(NATIVO), tmp)
            self.assertTrue(Path(r["arquivo"]).is_file())
            self.assertEqual(r["bytes"], Path(r["arquivo"]).stat().st_size)
            self.assertFalse(r["assinado"])

    def test_empacota_e_assina(self):
        with tempfile.TemporaryDirectory() as tmp:
            saida = Path(tmp) / "out"
            priv, _ = generate_keypair(Path(tmp) / "chaves")
            r = acoes.empacotar(str(NATIVO), str(saida), str(priv))
            self.assertTrue(r["assinado"])
            pacote = Path(r["arquivo"])
            self.assertTrue(Path(r["arquivo"] + ".sig").is_file())
            self.assertTrue(Path(r["arquivo"] + ".manifest.json").is_file())
            self.assertTrue(pacote.is_file())

    def test_manifesto_leva_a_versao_e_aponta_o_arquivo_sig(self):
        # Regressão de 06/11/2026: o painel chamava `sign_package` sem `version`
        # e recebia TypeError — o pacote saía sem manifesto. E o manifesto tem
        # de apontar o `.sig`, no formato herdado do antecessor; assinatura em
        # base64 dentro do manifesto era um segundo formato, incompatível.
        with tempfile.TemporaryDirectory() as tmp:
            saida = Path(tmp) / "out"
            priv, _ = generate_keypair(Path(tmp) / "chaves")
            r = acoes.empacotar(str(NATIVO), str(saida), str(priv))
            manifesto = json.loads(
                Path(r["arquivo"] + ".manifest.json").read_text(encoding="utf-8")
            )
            self.assertEqual(manifesto["version"], "1.0.0")
            self.assertEqual(manifesto["signature"], "app-nativo.giochi.sig")
            self.assertEqual(manifesto["sha256"], r["sha256"])

    def test_a_assinatura_tem_64_bytes_crus(self):
        # 64 é o tamanho do Ed25519. Gravar base64 em texto dá 89 bytes e o
        # `openssl pkeyutl -verify -rawin -sigfile` recusa.
        #
        # Não se testa "não contém \n": uma assinatura **binária** contém o byte
        # 0x0A por acaso em ~22% das vezes (1 - (255/256)**64), e o teste
        # falharia sem haver defeito. Teste probabilístico é bug de teste — o
        # tamanho é a prova, e é determinístico.
        with tempfile.TemporaryDirectory() as tmp:
            saida = Path(tmp) / "out"
            priv, _ = generate_keypair(Path(tmp) / "chaves")
            r = acoes.empacotar(str(NATIVO), str(saida), str(priv))
            sig = Path(r["arquivo"] + ".sig").read_bytes()
            self.assertEqual(len(sig), 64, "base64 em texto daria 89 bytes")

    def test_chave_inexistente_e_erro_de_entrada_nao_de_servidor(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(acoes.ErroDeAcao) as ctx:
                acoes.empacotar(str(NATIVO), tmp, "/nao/existe.pem")
            self.assertIn("chave", str(ctx.exception).lower())

    def test_pacote_e_reproduzivel(self):
        # Mesmo conteúdo, mesmo sha256 — é o que permite a assinatura
        # identificar o conteúdo em vez de identificar o momento do build.
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            r1 = acoes.empacotar(str(NATIVO), a)
            r2 = acoes.empacotar(str(NATIVO), b)
            self.assertEqual(r1["sha256"], r2["sha256"])


class TestPacotes(unittest.TestCase):
    def test_lista_o_que_esta_na_pasta(self):
        with tempfile.TemporaryDirectory() as tmp:
            acoes.empacotar(str(NATIVO), tmp)
            r = acoes.pacotes(tmp)
            self.assertEqual(len(r["pacotes"]), 1)
            item = r["pacotes"][0]
            self.assertEqual(item["id"], "app-nativo")
            self.assertEqual(item["versao"], "1.0.0")

    def test_pasta_inexistente_e_erro_de_entrada(self):
        with self.assertRaises(acoes.ErroDeAcao):
            acoes.pacotes("/nao/existe")

    def test_pacote_assinado_aparece_como_ok(self):
        with tempfile.TemporaryDirectory() as tmp:
            priv, _ = generate_keypair(Path(tmp) / "chaves")
            saida = Path(tmp) / "out"
            acoes.empacotar(str(NATIVO), str(saida), str(priv))
            r = acoes.pacotes(str(saida))
            self.assertTrue(r["pacotes"][0]["ok"], r["pacotes"][0]["motivo"])


class TestDestinos(unittest.TestCase):
    def test_devolve_lista_e_resumo(self):
        r = acoes.destinos()
        self.assertIsInstance(r["destinos"], list)
        self.assertIn("resumo", r)


if __name__ == "__main__":
    unittest.main()
