"""Classificacao Compatível / Parcial / Incompatível (secao 4.5.3).

Regra: classificacao final = pior caso. Todas as exigencias nao atendidas
sao listadas (nunca apenas a primeira).
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from enum import IntEnum
from pathlib import Path
from typing import Any

from .profile import DeviceProfile, ProfileError

# Folga considerada "parcial" quando o requisito chega perto do limite.
NEAR_LIMIT_RATIO = 0.80


class Verdict(IntEnum):
    COMPATIBLE = 0
    PARTIAL = 1
    INCOMPATIBLE = 2

    @property
    def label_pt(self) -> str:
        return {
            Verdict.COMPATIBLE: "Compatível",
            Verdict.PARTIAL: "Parcial",
            Verdict.INCOMPATIBLE: "Incompatível",
        }[self]


@dataclass
class Issue:
    requirement: str
    detail: str
    verdict: Verdict


@dataclass
class Classification:
    verdict: Verdict
    issues: list[Issue] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

    @property
    def allowed(self) -> bool:
        return self.verdict is not Verdict.INCOMPATIBLE

    def summary(self) -> str:
        lines = [f"{self.verdict.label_pt}"]
        for issue in self.issues:
            lines.append(f"  - [{issue.verdict.label_pt}] {issue.requirement}: {issue.detail}")
        return "\n".join(lines)


@dataclass
class GameRequirements:
    arch: str = "aarch64"
    runtime: str = "godot"
    graphics_api_min: str = "gles3"
    graphics_api_fallback: str = "gles2"
    godot_version: str | None = None
    ram_min_mb: int = 0
    vram_min_mb: int = 0
    display_min_width: int = 0
    display_min_height: int = 0
    touchscreen_required: bool = False
    source: str = "forge.json"

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "GameRequirements":
        return cls(
            arch=str(data.get("arch", "aarch64")),
            runtime=str(data.get("runtime", "godot")),
            graphics_api_min=str(data.get("graphics_api_min", "gles3")),
            graphics_api_fallback=str(data.get("graphics_api_fallback", "gles2")),
            godot_version=data.get("godot_version"),
            ram_min_mb=int(data.get("ram_min_mb", 0)),
            vram_min_mb=int(data.get("vram_min_mb", 0)),
            display_min_width=int(data.get("display_min_width", 0)),
            display_min_height=int(data.get("display_min_height", 0)),
            touchscreen_required=bool(data.get("touchscreen_required", False)),
            source="forge.json",
        )

    @classmethod
    def from_forge_json(cls, path: str | Path) -> "GameRequirements":
        try:
            data = json.loads(Path(path).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            raise ProfileError(f"forge.json invalido: {exc}") from exc
        return cls.from_dict(data)

    @classmethod
    def infer_from_project_godot(cls, path: str | Path) -> "GameRequirements":
        """Inferencia conservadora a partir de project.godot."""
        try:
            text = Path(path).read_text(encoding="utf-8")
        except OSError as exc:
            raise ProfileError(f"project.godot ilegivel: {exc}") from exc

        req = cls(source="project.godot")
        features: list[str] = []
        match = re.search(r"config/features\s*=\s*\[([^\]]*)\]", text)
        if match:
            features = re.findall(r'"([^"]+)"', match.group(1))

        for feature in features:
            if re.fullmatch(r"\d+\.\d+", feature):
                req.godot_version = feature
            elif feature in {"GLES3", "GLES2", "Vulkan"}:
                req.graphics_api_min = feature.lower()

        renderer = re.search(r"rendering/renderer/rendering_method\s*=\s*\"([^\"]+)\"", text)
        if renderer:
            method = renderer.group(1)
            if method == "mobile":
                req.graphics_api_min = "gles3"
            elif method == "gl_compatibility":
                req.graphics_api_min = "gles3" if "GLES3" in features else "gles2"
            elif method == "forward_plus":
                # forward_plus exige Vulkan: incompativel por definicao.
                req.graphics_api_min = "vulkan"

        if re.search(r"^\[input_devices/pointing\]", text, re.MULTILINE):
            req.touchscreen_required = False

        return req


def _version_tuple(text: str) -> tuple[int, ...]:
    parts = re.findall(r"\d+", text)
    return tuple(int(p) for p in parts)


def version_in_range(version: str, minimum: str, maximum: str) -> bool:
    v = _version_tuple(version)
    lo = _version_tuple(minimum)
    hi = _version_tuple(maximum)
    if not v or not lo or not hi:
        return False
    return lo <= v <= hi


def classify(req: GameRequirements, profile: DeviceProfile) -> Classification:
    issues: list[Issue] = []
    warnings: list[str] = []

    if req.arch != profile.arch:
        issues.append(Issue(
            "arch",
            f"pacote {req.arch} != dispositivo {profile.arch}",
            Verdict.INCOMPATIBLE,
        ))

    if req.runtime == "proton" and not profile.proton_supported:
        reason = profile.proton_reason or "unsupported"
        issues.append(Issue(
            "runtime",
            f"proton nao suportado ({reason})",
            Verdict.INCOMPATIBLE,
        ))

    # O runtime do Godot **nao esta na imagem** dos aparelhos medidos.
    #
    # Isto e ortogonal a `godot_min`/`godot_max`: aqueles dizem o que o hardware
    # suporta, este diz o que a imagem tem. Confundir os dois fazia o
    # classificador chamar de compativel um jogo que nao abre — nenhum build
    # oficial do Godot tem driver KMSDRM, e o rootfs nao tem X11 nem Wayland
    # (medido em 06/11/2026, ver docs/godot.md).
    #
    # `None` nao e "nao tem": e "nao medido", e nao gera nem problema nem
    # aviso. Perfil generico nao deve reprovar o que ninguem testou.
    if req.runtime == "godot" and profile.godot_runtime is False:
        issues.append(Issue(
            "runtime",
            "a imagem deste aparelho nao traz o runtime do Godot "
            "(nenhum build oficial tem KMSDRM); ver docs/godot.md — "
            "o caminho e port nativo (runtime \"native\")",
            Verdict.INCOMPATIBLE,
        ))

    api = req.graphics_api_min.lower()
    # A cadeia abaixo tem de ser **fechada**. Antes ela era uma sequencia de
    # `elif api == ... and not profile...`, e um valor que nao casasse com
    # nenhum ramo — `"none"`, `"gles4"`, um erro de digitacao — passava sem
    # problema nenhum, em silencio.
    #
    # Foi assim que um teste meu passou dos dois lados: ele afirmava que `none`
    # e compativel, e continuava verde mesmo sem o ramo `none`, porque a
    # classificacao caia no vazio. Teste que passa com e sem a correcao nao
    # prova nada; o ramo `else` e o que faz o teste discriminar.
    if api == "none":
        # O app nao desenha nada (ferramenta headless). Sem esta saida, o
        # default `gles3` reprovava a propria `forge` num RK3326, que nao tem
        # GLES3 — medido.
        pass
    elif api == "vulkan":
        issues.append(Issue(
            "graphics_api",
            "exige Vulkan; dispositivo oferece OpenGL ES 3.0/2.0 (ADR-06)",
            Verdict.INCOMPATIBLE,
        ))
    elif api == "gles3":
        if not profile.gles3:
            issues.append(Issue(
                "graphics_api",
                "exige GLES3; dispositivo nao declara gles3",
                Verdict.INCOMPATIBLE,
            ))
        elif req.graphics_api_fallback == "gles2":
            warnings.append("GLES3 com fallback GLES2: qualidade reduzida possivel")
    elif api == "gles2":
        if not profile.gles2:
            issues.append(Issue(
                "graphics_api",
                "exige GLES2; dispositivo nao declara gles2",
                Verdict.INCOMPATIBLE,
            ))
    else:
        issues.append(Issue(
            "graphics_api",
            f"valor desconhecido: {req.graphics_api_min!r} "
            "(aceitos: none, gles2, gles3, vulkan)",
            Verdict.INCOMPATIBLE,
        ))

    if req.godot_version:
        if req.runtime == "godot" and not version_in_range(
            req.godot_version, profile.godot_min, profile.godot_max
        ):
            issues.append(Issue(
                "engine",
                f"Godot {req.godot_version} fora de [{profile.godot_min}, "
                f"{profile.godot_max}]",
                Verdict.INCOMPATIBLE,
            ))

    if req.ram_min_mb > 0:
        ratio = req.ram_min_mb / profile.ram_mb if profile.ram_mb else 1.0
        if req.ram_min_mb > profile.ram_mb:
            issues.append(Issue(
                "ram",
                f"requer {req.ram_min_mb} MB; dispositivo tem {profile.ram_mb} MB",
                Verdict.INCOMPATIBLE,
            ))
        elif ratio > NEAR_LIMIT_RATIO:
            issues.append(Issue(
                "ram",
                f"requer {req.ram_min_mb} MB de {profile.ram_mb} MB "
                f"({ratio * 100:.0f}%); risco de swap",
                Verdict.PARTIAL,
            ))

    if req.vram_min_mb > 0:
        ratio = req.vram_min_mb / profile.vram_mb if profile.vram_mb else 1.0
        if req.vram_min_mb > profile.vram_mb:
            issues.append(Issue(
                "vram",
                f"requer {req.vram_min_mb} MB; dispositivo tem {profile.vram_mb} MB",
                Verdict.INCOMPATIBLE,
            ))
        elif ratio > NEAR_LIMIT_RATIO:
            issues.append(Issue(
                "vram",
                f"requer {req.vram_min_mb} MB de {profile.vram_mb} MB",
                Verdict.PARTIAL,
            ))

    # Comparacao com a area **utilizavel**, nao com o painel cru.
    #
    # O painel do X55 e 720x1280 (retrato) e o jogo desenha 1280x720 girando.
    # Comparar com o painel cru reprovava o caso mais comum: um jogo 1280x720
    # "nao caber" num aparelho de 720x1280 onde ele roda — foi o que o
    # arcade-volleyball fez, medido.
    util_largura, util_altura = profile.usable_display()
    if req.display_min_width > util_largura or req.display_min_height > util_altura:
        issues.append(Issue(
            "display",
            f"requer {req.display_min_width}x{req.display_min_height}; "
            f"dispositivo {profile.display_width}x{profile.display_height}"
            f" (utilizavel {util_largura}x{util_altura})",
            Verdict.INCOMPATIBLE,
        ))

    if req.touchscreen_required and not profile.touchscreen:
        issues.append(Issue(
            "touchscreen",
            "exige tela de toque (fora de escopo v1.2)",
            Verdict.INCOMPATIBLE,
        ))

    verdict = max((i.verdict for i in issues), default=Verdict.COMPATIBLE)
    return Classification(verdict=verdict, issues=issues, warnings=warnings)
