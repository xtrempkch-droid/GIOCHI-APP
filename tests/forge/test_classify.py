import unittest

from giochi_forge.classify import (
    GameRequirements,
    Verdict,
    classify,
    version_in_range,
)
from giochi_forge.profile import MANUAL_PROFILES


class ClassifyTest(unittest.TestCase):
    def setUp(self):
        self.rk3326 = MANUAL_PROFILES["rk3326-1gb"]
        self.rk3566 = MANUAL_PROFILES["rk3566-2gb"]

    def test_compatible(self):
        # Godot 3.5 no RK3326: o SoC tem lima, que entrega GLES 2.0 e nao 3.x
        # (medido no features.txt do Mesa). Um jogo 3.5 pede gles2, nao gles3.
        req = GameRequirements(godot_version="3.5", ram_min_mb=512,
                               graphics_api_min="gles2")
        result = classify(req, self.rk3326)
        self.assertIs(result.verdict, Verdict.COMPATIBLE)
        self.assertEqual(result.issues, [])

    def test_gles3_nao_roda_no_lima(self):
        # Correcao de dado (2026-10-31): o perfil do RK3326 dizia
        # gles3=true/panfrost, o que fazia a Loja recomendar jogo que nao roda.
        # O Mali-G31 com lima fica em GLES 2.0.
        req = GameRequirements(godot_version="4.3", graphics_api_min="gles3")
        result = classify(req, self.rk3326)
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)
        self.assertTrue(any("gles" in i.detail.lower() for i in result.issues))

    def test_vulkan_is_incompatible(self):
        req = GameRequirements(graphics_api_min="Vulkan")
        result = classify(req, self.rk3566)
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)
        self.assertTrue(any("Vulkan" in i.detail for i in result.issues))

    def test_arch_mismatch(self):
        req = GameRequirements(arch="x86_64")
        result = classify(req, self.rk3326)
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)

    def test_proton_without_support(self):
        req = GameRequirements(runtime="proton")
        result = classify(req, self.rk3566)
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)
        self.assertTrue(any(i.requirement == "runtime" for i in result.issues))

    def test_ram_near_limit_is_partial(self):
        req = GameRequirements(ram_min_mb=900, graphics_api_min="gles2")
        result = classify(req, self.rk3326)
        self.assertIs(result.verdict, Verdict.PARTIAL)

    def test_ram_over_limit_is_incompatible(self):
        req = GameRequirements(ram_min_mb=2048)
        result = classify(req, self.rk3326)
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)

    def test_worst_case_wins_and_lists_all(self):
        req = GameRequirements(
            arch="x86_64",
            graphics_api_min="vulkan",
            ram_min_mb=900,
            touchscreen_required=True,
        )
        result = classify(req, self.rk3326)
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)
        requirements = {issue.requirement for issue in result.issues}
        self.assertIn("ram", requirements)
        self.assertIn("arch", requirements)
        self.assertIn("graphics_api", requirements)
        self.assertIn("touchscreen", requirements)

    def test_display_too_small(self):
        req = GameRequirements(display_min_width=1280, display_min_height=720)
        result = classify(req, self.rk3326)
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)

    def test_godot_out_of_range(self):
        req = GameRequirements(godot_version="4.5")
        result = classify(req, self.rk3326)
        self.assertIs(result.verdict, Verdict.INCOMPATIBLE)

    def test_version_in_range(self):
        self.assertTrue(version_in_range("3.5", "3.5", "4.3"))
        self.assertTrue(version_in_range("4.3", "3.5", "4.3"))
        self.assertFalse(version_in_range("4.4", "3.5", "4.3"))
        self.assertFalse(version_in_range("3.4", "3.5", "4.3"))


if __name__ == "__main__":
    unittest.main()
