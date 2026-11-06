"""Testes da Loja local (giochi-forge store).

O que estes testes protegem, em ordem de importancia:

1. a deteccao de `kind` nao pode passar a aceitar pasta que nao e pacote;
2. um pacote invalido (arch errado, sem capa, sem bin/) tem de virar erro ou
   "pulado" com motivo -- nunca "gerado";
3. `arch` diferente de aarch64 e erro, nao aviso (ADR-13);
4. o `.giochi` gerado tem a ordem que a ADR-11 exige;
5. dry-run nao escreve nada.
"""

import io
import json
import os
import struct
import tarfile
import tempfile
import unittest
import zlib
from pathlib import Path

from giochi_forge import store
from giochi_forge.profile import MANUAL_PROFILES


def _png(w: int = 8, h: int = 8) -> bytes:
    """PNG minimo valido, sem depender de Pillow."""
    raw = b"".join(b"\x00" + bytes([10, 20, 30]) * w for _ in range(h))

    def chunk(tag: bytes, data: bytes) -> bytes:
        body = tag + data
        return (struct.pack(">I", len(data)) + body
                + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF))

    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw))
            + chunk(b"IEND", b""))


def _metadata(**over) -> dict:
    data = {"title": "Jogo", "developer": "Dev", "version": "1.0.0",
            "arch": "aarch64", "runtime": "godot", "entrypoint": "bin/run"}
    data.update(over)
    return data


class Fixture(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.src = self.root / "baixados"
        self.out = self.root / "pacotes"
        self.src.mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def make_package(self, name: str, **over) -> Path:
        path = self.src / name
        (path / "bin").mkdir(parents=True)
        (path / "metadata.json").write_text(json.dumps(_metadata(**over)),
                                           encoding="utf-8")
        (path / "capa.png").write_bytes(_png())
        entry = path / "bin" / "run"
        entry.write_text("#!/bin/sh\n", encoding="utf-8")
        entry.chmod(0o755)
        return path

    def make_godot(self, name: str) -> Path:
        path = self.src / name
        path.mkdir()
        (path / "project.godot").write_text('config_version=5\n', encoding="utf-8")
        return path

    def make_source(self, name: str) -> Path:
        path = self.src / name
        path.mkdir()
        (path / "CMakeLists.txt").write_text("project(x)\n", encoding="utf-8")
        return path

    def make_junk(self, name: str) -> Path:
        path = self.src / name
        path.mkdir()
        (path / "leiame.txt").write_text("nada aqui\n", encoding="utf-8")
        return path


class DetectKindTest(Fixture):
    def test_pacote_completo(self):
        self.assertIs(store.detect_kind(self.make_package("bom")), store.Kind.PACKAGE)

    def test_metadata_sem_capa_e_incompleto(self):
        path = self.make_package("sem-capa")
        (path / "capa.png").unlink()
        self.assertIs(store.detect_kind(path), store.Kind.PACKAGE_INCOMPLETE)

    def test_metadata_sem_bin_e_incompleto(self):
        path = self.make_package("sem-bin")
        (path / "bin" / "run").unlink()
        (path / "bin").rmdir()
        self.assertIs(store.detect_kind(path), store.Kind.PACKAGE_INCOMPLETE)

    def test_project_godot(self):
        self.assertIs(store.detect_kind(self.make_godot("g")), store.Kind.GODOT)

    def test_fonte(self):
        self.assertIs(store.detect_kind(self.make_source("f")), store.Kind.SOURCE)

    def test_desconhecido(self):
        self.assertIs(store.detect_kind(self.make_junk("x")), store.Kind.UNKNOWN)

    def test_scan_ignora_diretorio_oculto(self):
        self.make_package("bom")
        (self.src / ".git").mkdir()
        nomes = [p.name for p in store.scan(self.src)]
        self.assertEqual(nomes, ["bom"])

    def test_scan_nao_desce_dois_niveis(self):
        # Um zip do GitHub extrai como <repo>-main/; nao descer evita achar
        # "projeto" dentro de examples/ de outro projeto.
        profundo = self.src / "repo" / "examples" / "demo"
        profundo.mkdir(parents=True)
        (profundo / "project.godot").write_text("config_version=5\n", encoding="utf-8")
        self.assertEqual([p.name for p in store.scan(self.src)], ["repo"])


class RunTest(Fixture):
    def test_gera_pacote_e_relata(self):
        self.make_package("bom")
        report = store.run(self.src, self.out)
        self.assertEqual(report.count(store.Status.BUILT), 1)
        self.assertFalse(report.has_errors)
        gerado = self.out / "bom.giochi"
        self.assertTrue(gerado.is_file())

    def test_ordem_do_tar_e_a_da_adr11(self):
        self.make_package("bom")
        store.run(self.src, self.out)
        with tarfile.open(self.out / "bom.giochi") as tar:
            nomes = [m.name for m in tar.getmembers()]
        self.assertEqual(nomes[0], "metadata.json")
        self.assertEqual(nomes[1], "capa.png")
        self.assertTrue(nomes[2].startswith("bin/"))

    def test_arch_errada_e_erro_nao_aviso(self):
        self.make_package("x86", arch="x86_64")
        report = store.run(self.src, self.out)
        self.assertEqual(report.count(store.Status.ERROR), 1)
        self.assertEqual(report.count(store.Status.BUILT), 0)
        self.assertIn("aarch64", report.candidates[0].detail)

    def test_runtime_desconhecido_e_erro(self):
        self.make_package("r", runtime="unity")
        report = store.run(self.src, self.out)
        self.assertEqual(report.count(store.Status.ERROR), 1)

    def test_godot_e_pulado_com_motivo(self):
        self.make_godot("g")
        report = store.run(self.src, self.out)
        self.assertEqual(report.count(store.Status.SKIPPED), 1)
        # A asserção era `assertIn("musl", ...)`, porque o motivo registrado era
        # "o rootfs é musl". Medido em 06/11/2026: o rootfs é **glibc 2.38** (o
        # binário do arcade-volleyball exige GLIBC_2.38 e roda), e o bloqueio
        # real do Godot é outro — nenhum build oficial tem KMSDRM e o rootfs não
        # tem X11 nem Wayland (`docs/godot.md`).
        #
        # O teste continua valendo: o que ele garante é que o projeto Godot é
        # pulado **com motivo escrito**. O motivo é que mudou, porque a medição
        # derrubou o anterior.
        detalhe = report.candidates[0].detail
        self.assertIn("KMSDRM", detalhe)
        self.assertNotIn("musl", detalhe, "o rootfs não é musl: é glibc 2.38")

    def test_fonte_e_pulada_com_motivo(self):
        self.make_source("f")
        report = store.run(self.src, self.out)
        self.assertEqual(report.count(store.Status.SKIPPED), 1)
        self.assertIn("cross-compile", report.candidates[0].detail)

    def test_pacote_incompleto_diz_o_que_falta(self):
        path = self.make_package("incompleto")
        (path / "capa.png").unlink()
        report = store.run(self.src, self.out)
        self.assertEqual(report.count(store.Status.SKIPPED), 1)
        self.assertIn("capa.png", report.candidates[0].detail)

    def test_dry_run_nao_escreve(self):
        self.make_package("bom")
        report = store.run(self.src, self.out, dry_run=True)
        self.assertEqual(report.count(store.Status.BUILT), 1)
        self.assertFalse(self.out.exists() and any(self.out.iterdir()))

    def test_src_inexistente_devolve_relatorio_vazio(self):
        report = store.run(self.root / "nao-existe", self.out)
        self.assertEqual(report.candidates, [])
        self.assertIn("nenhuma pasta", store.render(report))

    def test_chave_ausente_nao_deixa_pacote_orfao(self):
        self.make_package("bom")
        report = store.run(self.src, self.out, key=self.root / "nao.pem")
        self.assertEqual(report.count(store.Status.ERROR), 1)
        self.assertIn("assinatura", report.candidates[0].detail)
        self.assertFalse((self.out / "bom.giochi").exists())

    def test_classifica_contra_o_aparelho_quando_ha_forge_json(self):
        path = self.make_package("pesado")
        (path / "forge.json").write_text(
            json.dumps({"runtime": "godot", "godot_version": "4.3",
                        "ram_min_mb": 8192}), encoding="utf-8")
        # Sem perfil: empacota. Com perfil de 2 GB: barra.
        self.assertEqual(store.run(self.src, self.out).count(store.Status.BUILT), 1)
        report = store.run(self.src, self.out,
                           profile=MANUAL_PROFILES["rk3566-2gb"])
        self.assertEqual(report.count(store.Status.ERROR), 1)


class RenderTest(Fixture):
    def test_resumo_conta_por_status(self):
        self.make_package("bom")
        self.make_godot("g")
        report = store.run(self.src, self.out, dry_run=True)
        texto = store.render(report)
        self.assertIn("1 gerado(s)", texto)
        self.assertIn("1 pulado(s)", texto)


if __name__ == "__main__":
    unittest.main()
