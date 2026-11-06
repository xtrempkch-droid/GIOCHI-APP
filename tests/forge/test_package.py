import hashlib
import io
import json
import tarfile
import tempfile
import unittest
from pathlib import Path

from giochi_forge.package import (
    PackageError,
    build_package,
    validate_metadata,
)


def make_project(root: Path, **overrides) -> Path:
    metadata = {
        "title": "Meu Jogo",
        "developer": "Studio X",
        "version": "1.0.0",
        "arch": "aarch64",
        "runtime": "godot",
        "graphics_api": "gles3",
        "graphics_api_fallback": "gles2",
        "buttons_map": "default",
        "entrypoint": "bin/jogo.arm64",
    }
    metadata.update(overrides)
    (root / "bin").mkdir(parents=True, exist_ok=True)
    (root / "metadata.json").write_text(json.dumps(metadata), encoding="utf-8")
    (root / "capa.png").write_bytes(b"\x89PNG\r\n\x1a\nfake")
    (root / "bin" / "jogo.arm64").write_bytes(b"\x7fELFfake")
    return root


class PackageTest(unittest.TestCase):
    def test_build_order_and_members(self):
        with tempfile.TemporaryDirectory() as tmp:
            project = make_project(Path(tmp) / "proj")
            out = Path(tmp) / "jogo.giochi"
            result = build_package(project, out)

            self.assertTrue(out.is_file())
            self.assertEqual(result.sha256,
                             hashlib.sha256(out.read_bytes()).hexdigest())

            with tarfile.open(out) as tar:
                names = tar.getnames()
            self.assertEqual(names[0], "metadata.json")
            self.assertEqual(names[1], "capa.png")
            self.assertIn("bin/jogo.arm64", names)

    def test_thumb_optional_and_ordered(self):
        with tempfile.TemporaryDirectory() as tmp:
            project = make_project(Path(tmp) / "proj")
            (project / "thumb.png").write_bytes(b"\x89PNGthumb")
            out = Path(tmp) / "jogo.giochi"
            build_package(project, out)
            with tarfile.open(out) as tar:
                names = tar.getnames()
            self.assertEqual(names[2], "thumb.png")

    def test_missing_cover_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            project = make_project(Path(tmp) / "proj")
            (project / "capa.png").unlink()
            with self.assertRaises(PackageError):
                build_package(project, Path(tmp) / "x.giochi")

    def test_arch_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            project = make_project(Path(tmp) / "proj", arch="x86_64")
            with self.assertRaises(PackageError):
                build_package(project, Path(tmp) / "x.giochi")

    def test_entrypoint_escape_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            project = make_project(Path(tmp) / "proj",
                                   entrypoint="bin/../../etc/passwd")
            with self.assertRaises(PackageError):
                build_package(project, Path(tmp) / "x.giochi")

    def test_proton_allowed_in_schema_but_refusable(self):
        with tempfile.TemporaryDirectory() as tmp:
            project = make_project(Path(tmp) / "proj", runtime="proton")
            out = Path(tmp) / "p.giochi"
            build_package(project, out)
            self.assertTrue(out.is_file())
            with self.assertRaises(PackageError):
                build_package(project, out, allow_proton=False)

    def test_validate_requires_fields(self):
        with self.assertRaises(PackageError):
            validate_metadata({"title": "x"})


if __name__ == "__main__":
    unittest.main()
