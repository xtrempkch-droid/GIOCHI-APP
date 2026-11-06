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
            raw=data,
        )

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


# Fallback sem USB (secao 4.5.3): perfis manuais conservadores.
MANUAL_PROFILES: dict[str, DeviceProfile] = {
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
            "storage": {"system_mb": 150, "user_free_mb": 4096},
            "os": {"name": "Giochi Konsol OS", "version": "1.2", "kernel": "6.12"},
            "graphics_api": {"gles3": False, "gles2": True, "vulkan": False},
            "input": {"touchscreen": False, "gamepad": True},
            "display": {"width": 640, "height": 480, "refresh_hz": 60},
            "engine": {"godot_min": "3.5", "godot_max": "4.3", "renderer": "compatibility"},
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
            "storage": {"system_mb": 150, "user_free_mb": 8192},
            "os": {"name": "Giochi Konsol OS", "version": "1.2", "kernel": "6.12"},
            "graphics_api": {"gles3": False, "gles2": True, "vulkan": False},
            "input": {"touchscreen": False, "gamepad": True},
            "display": {"width": 640, "height": 480, "refresh_hz": 60},
            "engine": {"godot_min": "3.5", "godot_max": "4.3", "renderer": "compatibility"},
            "network": {"wifi_usb": True, "bluetooth_usb": True, "integrated": False},
            "proton": {"supported": False, "reason": "vulkan_absent"},
        }
    ),
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
            "storage": {"system_mb": 150, "user_free_mb": 8192},
            "os": {"name": "Giochi Konsol OS", "version": "1.2", "kernel": "6.12"},
            "graphics_api": {"gles3": True, "gles2": True, "vulkan": False},
            "input": {"touchscreen": False, "gamepad": True},
            "display": {"width": 640, "height": 480, "refresh_hz": 60},
            "engine": {"godot_min": "3.5", "godot_max": "4.3", "renderer": "compatibility"},
            "network": {"wifi_usb": True, "bluetooth_usb": True, "integrated": False},
            "proton": {"supported": False, "reason": "vulkan_immature"},
        }
    ),
}
