"""Perfis dos aparelhos reais e as duas correções de classificação (06/11/2026).

Separado de `test_classify.py` de propósito: aquele veio do antecessor `GIOCHI-KONSOL-OS`
e continua sendo a referência do que ele já garantia. Este é nosso, e fixa duas
coisas que a medição em hardware mostrou estarem erradas.

1. **O painel é do aparelho, não do SoC.** Havia um perfil para o RK3566 com
   640×480, enquanto o X55 tem 720×1280. E a comparação usava o painel cru, então
   um jogo 1280×720 — que é *o formato de todo app deste projeto* — era reprovado
   num aparelho de 720×1280 onde ele roda. Medido: o arcade-volleyball desenha
   1280×720 girado no X55.

2. **O runtime do Godot não está na imagem.** Nenhum build oficial do Godot tem
   driver KMSDRM e o rootfs não tem X11 nem Wayland (`docs/godot.md`). O
   classificador dizia "compatível" e o jogo não abria.

A RAM também passou a ser a medida (1968/964 MB) em vez do arredondado do SoC
(2048/1024): o firmware reserva memória para a GPU, e um jogo que declara
"preciso de 2000 MB" passaria no número redondo e morreria no real.
"""

import unittest

from giochi_forge.classify import GameRequirements, Verdict, classify
from giochi_forge.profile import MANUAL_PROFILES


class PerfisReaisTest(unittest.TestCase):
    def test_os_tres_aparelhos_estao_declarados(self):
        for nome in ("x55", "x35h", "x35s"):
            self.assertIn(nome, MANUAL_PROFILES, f"falta o perfil {nome}")

    def test_x55_e_retrato_720x1280(self):
        x55 = MANUAL_PROFILES["x55"]
        self.assertEqual((x55.display_width, x55.display_height), (720, 1280))

    def test_x35h_e_640x480(self):
        x35h = MANUAL_PROFILES["x35h"]
        self.assertEqual((x35h.display_width, x35h.display_height), (640, 480))

    def test_ram_e_a_medida_e_nao_o_nominal(self):
        # 1968 no X55 e 964 no X35H: medido no /proc/meminfo. Se alguém
        # "corrigir" para 2048/1024, o classificador volta a aprovar jogo que
        # nao cabe.
        self.assertEqual(MANUAL_PROFILES["x55"].ram_mb, 1968)
        self.assertEqual(MANUAL_PROFILES["x35h"].ram_mb, 964)

    def test_x55_nao_tem_runtime_godot_na_imagem(self):
        self.assertIs(MANUAL_PROFILES["x55"].godot_runtime, False)

    def test_perfil_generico_nao_declara_o_que_nao_foi_medido(self):
        # `None` é "não medido", que não é "não tem". Perfil genérico não pode
        # reprovar o que ninguém testou.
        for nome in ("rk3326-1gb", "rk3566-2gb"):
            self.assertIsNone(MANUAL_PROFILES[nome].godot_runtime, nome)


class AreaUtilizavelTest(unittest.TestCase):
    def test_painel_retrato_e_usado_como_paisagem(self):
        self.assertEqual(MANUAL_PROFILES["x55"].usable_display(), (1280, 720))

    def test_painel_paisagem_nao_muda(self):
        self.assertEqual(MANUAL_PROFILES["x35h"].usable_display(), (640, 480))


class ClassificacaoCorrigidaTest(unittest.TestCase):
    def test_jogo_1280x720_cabe_num_aparelho_720x1280(self):
        # O caso que o código antigo reprovava. Não é hipótese: é o formato do
        # espaço de projeto do launcher e dos jogos.
        req = GameRequirements(
            runtime="native", graphics_api_min="gles2",
            display_min_width=1280, display_min_height=720,
        )
        result = classify(req, MANUAL_PROFILES["x55"])
        self.assertIs(result.verdict, Verdict.COMPATIBLE, result.summary())
        self.assertFalse(
            any(i.requirement == "display" for i in result.issues),
            "tela 1280x720 não pode ser problema num painel 720x1280 girado",
        )

    def test_jogo_1280x720_e_reprovado_num_painel_640x480(self):
        # E o outro lado: no X35H, que é 4:3 de verdade, não cabe.
        req = GameRequirements(
            runtime="native", graphics_api_min="gles2",
            display_min_width=1280, display_min_height=720,
        )
        result = classify(req, MANUAL_PROFILES["x35h"])
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)
        self.assertTrue(any(i.requirement == "display" for i in result.issues))

    def test_jogo_godot_e_incompativel_no_x55(self):
        req = GameRequirements(runtime="godot", godot_version="4.3",
                               graphics_api_min="gles3")
        result = classify(req, MANUAL_PROFILES["x55"])
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)
        runtime = [i for i in result.issues if i.requirement == "runtime"]
        self.assertTrue(runtime, result.summary())
        self.assertIn("KMSDRM", runtime[0].detail)

    def test_jogo_nativo_nao_e_afetado_pelo_godot_ausente(self):
        req = GameRequirements(runtime="native", graphics_api_min="gles2")
        result = classify(req, MANUAL_PROFILES["x55"])
        self.assertIs(result.verdict, Verdict.COMPATIBLE, result.summary())

    def test_godot_em_perfil_generico_nao_e_reprovado_por_runtime(self):
        # Ninguém mediu a imagem do perfil genérico. Não saber não é reprovar.
        req = GameRequirements(runtime="godot", godot_version="3.5",
                               graphics_api_min="gles2")
        result = classify(req, MANUAL_PROFILES["rk3326-1gb"])
        self.assertFalse(
            any(i.requirement == "runtime" for i in result.issues), result.summary()
        )


class FerramentaSemGraficosTest(unittest.TestCase):
    """`graphics_api_min: "none"`: o app não desenha nada.

    Era uma pendência anotada no README do antecessor e nunca resolvida: o
    classificador assumia `gles3` quando a chave faltava, então a própria
    `forge` — uma CLI headless — era reprovada num RK3326, que não tem GLES3.
    """

    def test_none_nao_exige_gles_nenhum(self):
        req = GameRequirements(runtime="native", graphics_api_min="none")
        result = classify(req, MANUAL_PROFILES["rk3326-1gb"])  # só GLES 2.0
        self.assertIs(result.verdict, Verdict.COMPATIBLE, result.summary())

    def test_valor_desconhecido_e_reprovado_e_nao_ignorado(self):
        # Antes, um valor que não casasse com nenhum ramo da cadeia passava em
        # silêncio. Era o que fazia `test_none_nao_exige_gles_nenhum` passar com
        # e sem a correção — teste que não discrimina não prova nada.
        req = GameRequirements(runtime="native", graphics_api_min="gles4")
        result = classify(req, MANUAL_PROFILES["x55"])
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)
        self.assertTrue(any("desconhecido" in i.detail for i in result.issues))

    def test_omitir_a_chave_continua_conservador(self):
        # O default continua gles3 de propósito: um app gráfico que esquece de
        # declarar não pode passar como se não desenhasse.
        req = GameRequirements(runtime="native")
        self.assertEqual(req.graphics_api_min, "gles3")
        result = classify(req, MANUAL_PROFILES["rk3326-1gb"])
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)

    def test_a_propria_forge_cabe_nos_aparelhos(self):
        # `app/forge/forge.json` é headless: tem de ser compatível até no
        # aparelho mais fraco.
        import json
        from pathlib import Path

        raiz = Path(__file__).resolve().parents[2]
        forge_json = raiz / "app" / "forge" / "forge.json"
        req = GameRequirements.from_dict(json.loads(forge_json.read_text(encoding="utf-8")))
        for nome in ("x55", "x35h", "rk3326-1gb"):
            result = classify(req, MANUAL_PROFILES[nome])
            self.assertIs(result.verdict, Verdict.COMPATIBLE, f"{nome}: {result.summary()}")


class StoreRecusaGodotNoX55Test(unittest.TestCase):
    """O caminho de ponta a ponta: a loja não pode gerar pacote que não abre.

    `store.run` é o que a loja usa para varrer uma pasta de projetos baixados.
    Se ele gerar o `.giochi` de um projeto Godot para o X55, o usuário compra e
    instala um app que cai em `Headless` e não desenha nada.
    """

    def _projeto(self, raiz, runtime, forge_json):
        import json
        import struct
        import zlib
        from pathlib import Path

        proj = Path(raiz) / "proj"
        (proj / "bin").mkdir(parents=True)
        (proj / "bin" / "jogo").write_bytes(b"\x7fELF")
        (proj / "metadata.json").write_text(json.dumps({
            "title": "Teste", "developer": "X", "version": "1.0.0",
            "arch": "aarch64", "runtime": runtime, "entrypoint": "bin/jogo",
        }), encoding="utf-8")
        (proj / "forge.json").write_text(json.dumps(forge_json), encoding="utf-8")
        # capa PNG 320x320 de uma cor: construída aqui para o teste não depender
        # de arquivo binário no repositório.
        w = h = 320
        cru = (b"\x00" + bytes([15, 15, 15]) * w) * h

        def bloco(tipo, dados):
            return (struct.pack(">I", len(dados)) + tipo + dados
                    + struct.pack(">I", zlib.crc32(tipo + dados) & 0xFFFFFFFF))

        (proj / "capa.png").write_bytes(
            b"\x89PNG\r\n\x1a\n"
            + bloco(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + bloco(b"IDAT", zlib.compress(cru))
            + bloco(b"IEND", b"")
        )
        return Path(raiz)

    def test_godot_nao_gera_pacote_para_o_x55(self):
        import tempfile

        from giochi_forge import store

        with tempfile.TemporaryDirectory() as tmp:
            src = self._projeto(tmp, "godot",
                                {"runtime": "godot", "godot_version": "4.3",
                                 "graphics_api_min": "gles3"})
            report = store.run(src, f"{tmp}/out", profile=MANUAL_PROFILES["x55"])
            self.assertEqual(report.count(store.Status.ERROR), 1)
            self.assertEqual(report.count(store.Status.BUILT), 0)

    def test_o_motivo_nao_repete_o_veredito(self):
        # Já saiu "Incompatível: Incompatível" na loja. A mensagem é o que o
        # usuário lê; repetir a palavra não é informação.
        import tempfile

        from giochi_forge import store

        with tempfile.TemporaryDirectory() as tmp:
            src = self._projeto(tmp, "godot",
                                {"runtime": "godot", "godot_version": "4.3",
                                 "graphics_api_min": "gles3"})
            report = store.run(src, f"{tmp}/out", profile=MANUAL_PROFILES["x55"])
            detalhe = report.candidates[0].detail
            self.assertEqual(detalhe.count("Incompatível"), 1, detalhe)
            self.assertIn("runtime:", detalhe)

    def test_nativo_gera_pacote_para_o_x55(self):
        import tempfile

        from giochi_forge import store

        with tempfile.TemporaryDirectory() as tmp:
            src = self._projeto(tmp, "native",
                                {"runtime": "native", "graphics_api_min": "gles2"})
            report = store.run(src, f"{tmp}/out", profile=MANUAL_PROFILES["x55"])
            self.assertEqual(report.count(store.Status.BUILT), 1,
                             store.render(report))


if __name__ == "__main__":
    unittest.main()
