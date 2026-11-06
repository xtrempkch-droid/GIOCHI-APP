"""Leitura e validacao de device_profile.json (secao 4.5.2)."""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "1.0"


class ProfileError(ValueError):
    """device_profile.json invalido ou incompleto."""


def _require(obj: dict[str, Any], path: str) -> Any:
    node: Any = obj
    for part in path.split("."):
        if not isinstance(node, dict) or part not in node:
            raise ProfileError(f"device_profile.json sem '{path}'")
        node = node[part]
    return node


@dataclass(frozen=True)
class DeviceProfile:
    device_id: str
    soc: str
    arch: str
    cpu_cores: int
    cpu_model: str
    cpu_max_mhz: int
    gpu_model: str
    gpu_driver: str
    gles_max: str
    ram_mb: int
    vram_mb: int
    user_free_mb: int
    os_version: str
    kernel: str
    gles3: bool
    gles2: bool
    vulkan: bool
    touchscreen: bool
    gamepad: bool
    display_width: int
    display_height: int
    display_refresh_hz: int
    godot_min: str
    godot_max: str
    renderer: str
    wifi_usb: bool
    bluetooth_usb: bool
    integrated_radio: bool
    proton_supported: bool
    proton_reason: str = ""
    #: O **runtime do Godot** esta na imagem deste aparelho?
    #:
    #: Três estados de propósito, e a distinção é medida, não suposta:
    #:
    #: * ``True``  — verificado que está;
    #: * ``False`` — verificado que **não** está (a imagem do X55 e do X35H não
    #:   tem Godot: medido em 06/11/2026, ver ``docs/godot.md``);
    #: * ``None``  — não medido (perfis genéricos, que descrevem hardware e não
    #:   uma imagem que exista).
    #:
    #: Isto é ortogonal a ``godot_min``/``godot_max``: aqueles dizem o que o
    #: **hardware** suporta, este diz o que a **imagem** tem. Confundir os dois
    #: foi o que fez o classificador chamar de compatível um jogo que não abre.
    #:
    #: A lógica de três estados é a mesma de ``SemFio::tem_wifi`` no launcher:
    #: ``None`` não é "não tem", é "não sei".
    godot_runtime: bool | None = None
    raw: dict[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "DeviceProfile":
        if not isinstance(data, dict):
            raise ProfileError("perfil nao e um objeto JSON")
        version = data.get("schema_version")
        if version is not None and str(version).split(".")[0] != "1":
            raise ProfileError(f"schema_version desconhecida: {version}")

        graphics = _require(data, "graphics_api")
        display = _require(data, "display")
        engine = _require(data, "engine")
        network = data.get("network", {})
        proton = data.get("proton", {})

        return cls(
            device_id=str(_require(data, "device_id")),
            soc=str(_require(data, "soc")),
            arch=str(_require(data, "cpu.arch")),
            cpu_cores=int(_require(data, "cpu.cores")),
            cpu_model=str(_require(data, "cpu.model")),
            cpu_max_mhz=int(_require(data, "cpu.max_mhz")),
            gpu_model=str(_require(data, "gpu.model")),
            gpu_driver=str(_require(data, "gpu.driver")),
            gles_max=str(_require(data, "gpu.gles_max")),
            ram_mb=int(_require(data, "ram_mb")),
            vram_mb=int(_require(data, "vram_mb")),
            user_free_mb=int(_require(data, "storage.user_free_mb")),
            os_version=str(_require(data, "os.version")),
            kernel=str(_require(data, "os.kernel")),
            gles3=bool(graphics.get("gles3", False)),
            gles2=bool(graphics.get("gles2", False)),
            vulkan=bool(graphics.get("vulkan", False)),
            touchscreen=bool(_require(data, "input.touchscreen")),
            gamepad=bool(_require(data, "input.gamepad")),
            display_width=int(display["width"]),
            display_height=int(display["height"]),
            display_refresh_hz=int(display.get("refresh_hz", 60)),
            godot_min=str(engine["godot_min"]),
            godot_max=str(engine["godot_max"]),
            renderer=str(engine.get("renderer", "compatibility")),
            wifi_usb=bool(network.get("wifi_usb", False)),
            bluetooth_usb=bool(network.get("bluetooth_usb", False)),
            integrated_radio=bool(network.get("integrated", False)),
            proton_supported=bool(proton.get("supported", False)),
            proton_reason=str(proton.get("reason", "")),
            godot_runtime=engine.get("godot_runtime"),
            raw=data,
        )

    def usable_display(self) -> tuple[int, int]:
        """Área de desenho disponível para um app: **maior lado × menor lado**.

        O painel do X55 é 720×1280 — retrato. Mas nenhum app deste projeto
        desenha em retrato: o espaço de projeto é paisagem (1280×720) e o
        sistema gira o desenho (`fbcon/rotate=3`), o que o launcher e os jogos
        aplicam e está medido.

        Comparar `forge.json` com o painel cru reprovava o caso mais comum:
        um jogo que declara 1280×720 num aparelho de 720×1280. Ele roda — foi
        exatamente o que o arcade-volleyball fez no X55.

        A regra de maior×menor não precisa saber a rotação: qualquer app pode
        ser desenhado com o maior lado na horizontal. Num painel já paisagem
        (640×480) ela devolve o próprio painel.
        """
        maior = max(self.display_width, self.display_height)
        menor = min(self.display_width, self.display_height)
        return maior, menor

    @classmethod
    def from_json_text(cls, text: str) -> "DeviceProfile":
        try:
            data = json.loads(text)
        except json.JSONDecodeError as exc:
            raise ProfileError(f"JSON invalido: {exc}") from exc
        return cls.from_dict(data)

    @classmethod
    def from_file(cls, path: str | Path) -> "DeviceProfile":
        try:
            return cls.from_json_text(Path(path).read_text(encoding="utf-8"))
        except OSError as exc:
            raise ProfileError(f"nao foi possivel ler {path}: {exc}") from exc


# Perfis manuais: fallback quando o console não está por USB para responder.
#
# ## De onde vem cada número
#
# Nada aqui é estimativa. Três fontes, e cada campo diz a sua:
#
# 1. `GIOCHI-SYSTEM/device/<aparelho>/device.conf` e o `device.conf` do
#    antecessor `GIOCHI-KONSOL-OS` — SoC, GPU, painel, rotação, rádio.
# 2. **Medição no aparelho**, sessões de 03–06/11/2026: RAM real pelo
#    `/proc/meminfo`, painel pelo modo do DRM, rotação pelo `fbcon/rotate`.
# 3. Capacidade gráfica do SoC pelo `features.txt` do Mesa — RK3326 (lima) fica
#    em GL 2.1 / GLES 2.0; RK3566 (panfrost) chega a GL 3.1 / GLES 3.1. Nenhum
#    dos dois tem Vulkan.
#
# ## O que estava errado antes (06/11/2026)
#
# Havia **três** perfis e o do RK3566 declarava painel 640×480. Só que o painel
# não é do SoC: o X55 tem **720×1280** e o X35H tem 640×480, os dois com RK3566.
# Classificar os dois como o mesmo aparelho fazia o classificador reprovar um
# jogo 1280×720 no X55, onde ele roda — e aprovar o mesmo jogo no X35H, onde ele
# sai com faixa preta. O painel é fato do **aparelho**, não do chip.
#
# A RAM também estava arredondada para 2048/1024. O medido é **1968 MB** no X55
# e **964 MB** no X35H: o firmware reserva memória para a GPU, e um jogo que
# declara "preciso de 2000 MB" passaria no número redondo e morreria no real.
#
# ## `godot_runtime`
#
# `False` nos aparelhos medidos: a imagem não traz o runtime do Godot, e nenhum
# template oficial aarch64 tem driver KMSDRM (`docs/godot.md`). `None` nos
# perfis genéricos, que descrevem **hardware** e não uma imagem que exista.

MANUAL_PROFILES: dict[str, DeviceProfile] = {
    # ── Aparelhos reais, medidos ────────────────────────────
    #
    # X55: o único do projeto com bring-up completo (áudio, Wi-Fi, BT, 16 botões
    # + 4 eixos). Painel retrato, então os apps desenham em 1280×720 girado.
    "x55": DeviceProfile.from_dict(
        {
            "schema_version": SCHEMA_VERSION,
            "device_id": "powkiddy-x55",
            "soc": "rk3566",
            "cpu": {"arch": "aarch64", "cores": 4, "model": "Cortex-A55", "max_mhz": 1800},
            "gpu": {"model": "Mali-G52", "driver": "panfrost", "gles_max": "3.1",
                    "gl_desktop_max": "3.1"},
            # 1968 MB medido no /proc/meminfo, não os 2048 MB do SoC: o
            # firmware reserva para a GPU.
            "ram_mb": 1968,
            "vram_mb": 128,
            # rootfs.ext4 = 1,7 GiB medido. O cartão tem 116,5 GiB **sem
            # partição** — não é espaço utilizável até o System particionar,
            # então `user_free_mb` fica 0 em vez de prometer o que não existe.
            "storage": {"system_mb": 1740, "user_free_mb": 0},
            "os": {"name": "Giochi OS", "version": "1.2", "kernel": "6.12"},
            "graphics_api": {"gles3": True, "gles2": True, "vulkan": False},
            "input": {"touchscreen": False, "gamepad": True},
            "display": {"width": 720, "height": 1280, "refresh_hz": 60},
            # `godot_max` 4.7: medido que o 4.7.2 roda o projeto 4.3 sem uma
            # linha alterada (docs/godot.md). O `godot_runtime: False` é o que
            # impede o classificador de chamar isso de "compatível".
            "engine": {"godot_min": "3.5", "godot_max": "4.7",
                       "renderer": "compatibility", "godot_runtime": False},
            "network": {"wifi_usb": True, "bluetooth_usb": True, "integrated": True},
            "proton": {"supported": False, "reason": "vulkan_absent"},
        }
    ),
    # X35H: mesmo SoC do X55 e painel 4:3 — a prova de que a arquitetura não
    # depende do aparelho. Sem Wi-Fi nem BT (`GIOCHI_WIFI=none`), o que a tela de
    # rede do launcher passou a dizer como capacidade ausente.
    "x35h": DeviceProfile.from_dict(
        {
            "schema_version": SCHEMA_VERSION,
            "device_id": "powkiddy-x35h",
            "soc": "rk3566",
            "cpu": {"arch": "aarch64", "cores": 4, "model": "Cortex-A55", "max_mhz": 1800},
            "gpu": {"model": "Mali-G52", "driver": "panfrost", "gles_max": "3.1",
                    "gl_desktop_max": "3.1"},
            # 964 MB medido — 4,7 % abaixo do 1024 MB nominal.
            "ram_mb": 964,
            "vram_mb": 64,
            "storage": {"system_mb": 1740, "user_free_mb": 0},
            "os": {"name": "Giochi OS", "version": "1.2", "kernel": "6.12"},
            "graphics_api": {"gles3": True, "gles2": True, "vulkan": False},
            "input": {"touchscreen": False, "gamepad": True},
            "display": {"width": 640, "height": 480, "refresh_hz": 60},
            "engine": {"godot_min": "3.5", "godot_max": "4.7",
                       "renderer": "compatibility", "godot_runtime": False},
            "network": {"wifi_usb": True, "bluetooth_usb": True, "integrated": False},
            "proton": {"supported": False, "reason": "vulkan_absent"},
        }
    ),
    # ── R35S: medido no aparelho via TTL, 06/11/2026 ────────
    #
    # O R35S não tem rádio e o `dwc2` nasce como *host*: não há SSH nem usb0. O
    # único canal é a console serial (`ttyS2` @ 1500000), e foi por ela que estes
    # números saíram: `scripts/console-r35s.sh`.
    #
    # O que muda em relação ao X35H (mesmo painel, SoC diferente):
    #
    # - **Sem `KEY_POWER`.** O kernel declara 19 códigos, e 116 não está entre
    #   eles: `gpio-keys-vol` (114/115) e `r36s_Gamepad` (17). O X55 tem
    #   `rk805 pwrkey`; aqui o botão de energia não é dispositivo de entrada.
    # - **Sem RTC e sem rede**: o relógio diz 2017 para sempre. Isso é decisão de
    #   licença, não de perfil — ver `docs/loja.md` §4.1.
    # - Sem bateria e sem sensor térmico expostos nesta imagem.
    "r35s": DeviceProfile.from_dict(
        {
            "schema_version": SCHEMA_VERSION,
            "device_id": "giochi-system-r35s",
            "soc": "rk3326",
            "cpu": {"arch": "aarch64", "cores": 4, "model": "Cortex-A35", "max_mhz": 1500},
            "gpu": {"model": "Mali-G31", "driver": "panfrost", "gles_max": "3.1",
                    "gl_desktop_max": "3.1"},
            # 987.596 kB no /proc/meminfo = 964 MB, igual ao X35H.
            "ram_mb": 964,
            "vram_mb": 32,
            "storage": {"system_mb": 1740, "user_free_mb": 0},
            "os": {"name": "Giochi OS", "version": "1.2", "kernel": "6.12"},
            "graphics_api": {"gles3": True, "gles2": True, "vulkan": False},
            # Sem touchscreen e sem rádio; gamepad de 17 botões + 2 de volume.
            "input": {"touchscreen": False, "gamepad": True},
            # `fbcon/rotate` = 0 e DRM `card0` com modo 640x480: paisagem, sem
            # letterbox e sem rotação — medido, não deduzido do formato.
            "display": {"width": 640, "height": 480, "refresh_hz": 60},
            "engine": {"godot_min": "3.5", "godot_max": "4.7",
                       "renderer": "compatibility", "godot_runtime": False},
            # Sem rádio integrado (não existe no DTB) e sem porta USB host livre
            # no `dwc2`, que nasce como host mas é o canal de carga.
            "network": {"wifi_usb": False, "bluetooth_usb": False, "integrated": False},
            "proton": {"supported": False, "reason": "vulkan_absent"},
        }
    ),
    # X35S: mesmo chassi do X35H. Nunca medido em hardware
    # (`GIOCHI_TESTED=no`) e o rádio é desconhecido — os campos de rede dizem
    # "dongle USB, se houver", que é o que o device.conf declara.
    "x35s": DeviceProfile.from_dict(
        {
            "schema_version": SCHEMA_VERSION,
            "device_id": "powkiddy-x35s",
            "soc": "rk3566",
            "cpu": {"arch": "aarch64", "cores": 4, "model": "Cortex-A55", "max_mhz": 1800},
            "gpu": {"model": "Mali-G52", "driver": "panfrost", "gles_max": "3.1",
                    "gl_desktop_max": "3.1"},
            "ram_mb": 964,
            "vram_mb": 64,
            "storage": {"system_mb": 1740, "user_free_mb": 0},
            "os": {"name": "Giochi OS", "version": "1.2", "kernel": "6.12"},
            "graphics_api": {"gles3": True, "gles2": True, "vulkan": False},
            "input": {"touchscreen": False, "gamepad": True},
            "display": {"width": 640, "height": 480, "refresh_hz": 60},
            "engine": {"godot_min": "3.5", "godot_max": "4.7",
                       "renderer": "compatibility", "godot_runtime": False},
            "network": {"wifi_usb": True, "bluetooth_usb": True, "integrated": False},
            "proton": {"supported": False, "reason": "vulkan_absent"},
        }
    ),
    # ── Perfis genéricos de SoC ─────────────────────────────
    #
    # Não são aparelhos: são o **piso** de cada chip, para classificar antes de
    # existir o hardware. `godot_runtime` fica `None` porque ninguém mediu a
    # imagem — e `None` não é "não tem".
    "rk3326-1gb": DeviceProfile.from_dict(
        {
            "schema_version": SCHEMA_VERSION,
            "device_id": "manual-rk3326-1gb",
            "soc": "rk3326",
            "cpu": {"arch": "aarch64", "cores": 4, "model": "Cortex-A35", "max_mhz": 1500},
            "gpu": {"model": "Mali-G31", "driver": "lima", "gles_max": "2.0",
                    "gl_desktop_max": "2.1"},
            "ram_mb": 1024,
            "vram_mb": 64,
            "storage": {"system_mb": 1740, "user_free_mb": 0},
            "os": {"name": "Giochi OS", "version": "1.2", "kernel": "6.12"},
            "graphics_api": {"gles3": False, "gles2": True, "vulkan": False},
            "input": {"touchscreen": False, "gamepad": True},
            "display": {"width": 640, "height": 480, "refresh_hz": 60},
            "engine": {"godot_min": "3.5", "godot_max": "4.7", "renderer": "compatibility"},
            "network": {"wifi_usb": True, "bluetooth_usb": True, "integrated": False},
            "proton": {"supported": False, "reason": "vulkan_absent"},
        }
    ),
    "rk3326-2gb": DeviceProfile.from_dict(
        {
            "schema_version": SCHEMA_VERSION,
            "device_id": "manual-rk3326-2gb",
            "soc": "rk3326",
            "cpu": {"arch": "aarch64", "cores": 4, "model": "Cortex-A35", "max_mhz": 1500},
            "gpu": {"model": "Mali-G31", "driver": "lima", "gles_max": "2.0",
                    "gl_desktop_max": "2.1"},
            "ram_mb": 2048,
            "vram_mb": 128,
            "storage": {"system_mb": 1740, "user_free_mb": 0},
            "os": {"name": "Giochi OS", "version": "1.2", "kernel": "6.12"},
            "graphics_api": {"gles3": False, "gles2": True, "vulkan": False},
            "input": {"touchscreen": False, "gamepad": True},
            "display": {"width": 640, "height": 480, "refresh_hz": 60},
            "engine": {"godot_min": "3.5", "godot_max": "4.7", "renderer": "compatibility"},
            "network": {"wifi_usb": True, "bluetooth_usb": True, "integrated": False},
            "proton": {"supported": False, "reason": "vulkan_absent"},
        }
    ),
    # RK3566 genérico: painel 640×480 como o `rk3566-generic` do antecessor.
    # Se o alvo for o X55, use `--device x55` — o painel é do aparelho.
    "rk3566-2gb": DeviceProfile.from_dict(
        {
            "schema_version": SCHEMA_VERSION,
            "device_id": "manual-rk3566-2gb",
            "soc": "rk3566",
            "cpu": {"arch": "aarch64", "cores": 4, "model": "Cortex-A55", "max_mhz": 1800},
            "gpu": {"model": "Mali-G52", "driver": "panfrost", "gles_max": "3.1",
                    "gl_desktop_max": "3.1"},
            "ram_mb": 2048,
            "vram_mb": 128,
            "storage": {"system_mb": 1740, "user_free_mb": 0},
            "os": {"name": "Giochi OS", "version": "1.2", "kernel": "6.12"},
            "graphics_api": {"gles3": True, "gles2": True, "vulkan": False},
            "input": {"touchscreen": False, "gamepad": True},
            "display": {"width": 640, "height": 480, "refresh_hz": 60},
            "engine": {"godot_min": "3.5", "godot_max": "4.7", "renderer": "compatibility"},
            "network": {"wifi_usb": True, "bluetooth_usb": True, "integrated": False},
            "proton": {"supported": False, "reason": "vulkan_immature"},
        }
    ),
}
