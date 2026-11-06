"""Empacotamento .giochi (ADR-11) e assinatura (ADR-12)."""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tarfile
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

ALLOWED_RUNTIMES = {"godot", "retroarch", "proton"}
REQUIRED_FIELDS = ("title", "developer", "version", "arch", "runtime", "entrypoint")
MAX_METADATA_BYTES = 16 * 1024


class PackageError(ValueError):
    """Pacote invalido (schema, arquivos obrigatorios, assinatura)."""


@dataclass
class PackageResult:
    package: Path
    sha256: str
    size_bytes: int


def validate_metadata(metadata: dict) -> None:
    for field_name in REQUIRED_FIELDS:
        if not metadata.get(field_name):
            raise PackageError(f"metadata.json sem '{field_name}'")

    if metadata["arch"] != "aarch64":
        raise PackageError(f"arch '{metadata['arch']}' nao e aarch64 (ADR-13)")

    if metadata["runtime"] not in ALLOWED_RUNTIMES:
        raise PackageError(f"runtime '{metadata['runtime']}' desconhecido")

    entry = str(metadata["entrypoint"])
    if not entry.startswith("bin/") or ".." in entry or entry.startswith("/"):
        raise PackageError(f"entrypoint invalido: {entry}")


def build_package(
    source_dir: str | Path,
    output: str | Path,
    *,
    compress_metadata: bool = False,
    allow_proton: bool = True,
) -> PackageResult:
    """Empacota em tar puro na ordem exigida pela ADR-11."""
    src = Path(source_dir)
    out = Path(output)

    metadata_path = src / "metadata.json"
    if not metadata_path.is_file():
        raise PackageError("metadata.json ausente")
    if metadata_path.stat().st_size > MAX_METADATA_BYTES:
        raise PackageError("metadata.json grande demais")

    try:
        metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise PackageError(f"metadata.json invalido: {exc}") from exc

    validate_metadata(metadata)
    if not allow_proton and metadata["runtime"] == "proton":
        raise PackageError("runtime 'proton' recusado nesta build")

    cover = src / "capa.png"
    if not cover.is_file():
        raise PackageError("capa.png 320x320 obrigatoria")

    entrypoint = src / metadata["entrypoint"]
    if not entrypoint.is_file():
        raise PackageError(f"entrypoint ausente: {metadata['entrypoint']}")

    out.parent.mkdir(parents=True, exist_ok=True)
    members: list[tuple[Path, str]] = [(metadata_path, "metadata.json"),
                                       (cover, "capa.png")]
    thumb = src / "thumb.png"
    if thumb.is_file():
        members.append((thumb, "thumb.png"))

    bin_dir = src / "bin"
    for path in sorted(bin_dir.rglob("*")):
        if path.is_file():
            members.append((path, str(path.relative_to(src))))

    # TODO(ADR-11): zstd -1 opcional em metadata.json. Sem zstd o membro sai
    # como tar puro, que o Launcher aceita (compatibilidade preservada).
    if compress_metadata:
        raise PackageError("compressao de metadata requer zstd (nao implementado)")

    with tarfile.open(out, "w", format=tarfile.USTAR_FORMAT) as tar:
        for path, arcname in members:
            tar.add(path, arcname=arcname, recursive=False)

    digest = hashlib.sha256(out.read_bytes()).hexdigest()
    return PackageResult(package=out, sha256=digest, size_bytes=out.stat().st_size)


def sign_package(
    package: str | Path,
    private_key: str | Path,
    *,
    version: str,
    openssl: str = "openssl",
) -> tuple[Path, Path]:
    """Gera .sig (Ed25519) + manifesto JSON com SHA-256."""
    pkg = Path(package)
    key = Path(private_key)
    if not pkg.is_file():
        raise PackageError(f"pacote inexistente: {pkg}")
    if not key.is_file():
        raise PackageError(f"chave privada inexistente: {key}")

    sig_path = pkg.with_suffix(pkg.suffix + ".sig")
    manifest_path = pkg.with_suffix(pkg.suffix + ".manifest.json")

    command = [
        openssl, "pkeyutl", "-sign", "-inkey", str(key),
        "-rawin", "-in", str(pkg), "-out", str(sig_path),
    ]
    try:
        subprocess.run(command, check=True, capture_output=True)
    except FileNotFoundError as exc:
        raise PackageError("openssl nao encontrado no PATH") from exc
    except subprocess.CalledProcessError as exc:
        raise PackageError(
            f"openssl falhou: {exc.stderr.decode(errors='replace').strip()}"
        ) from exc

    manifest = {
        "schema_version": "1.0",
        "package": pkg.name,
        "version": version,
        "sha256": hashlib.sha256(pkg.read_bytes()).hexdigest(),
        "size_bytes": pkg.stat().st_size,
        "signature": sig_path.name,
    }
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return sig_path, manifest_path


def generate_keypair(out_dir: str | Path, openssl: str = "openssl") -> tuple[Path, Path]:
    """openssl genpkey -algorithm ed25519 + extracao da chave publica raw."""
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    priv = out / "giochi_ed25519.pem"
    pub = out / "pubkey.ed25519"

    subprocess.run(
        [openssl, "genpkey", "-algorithm", "ed25519", "-out", str(priv)],
        check=True,
        capture_output=True,
    )
    der = subprocess.run(
        [openssl, "pkey", "-in", str(priv), "-pubout", "-outform", "DER"],
        check=True,
        capture_output=True,
    ).stdout
    # A chave publica Ed25519 em DER termina com os 32 bytes crus.
    if len(der) < 32:
        raise PackageError("chave publica DER inesperada")
    pub.write_bytes(der[-32:])
    return priv, pub


def find_unpacked_metadata(root: str | Path) -> Iterable[Path]:
    """Pacotes extraidos (metadata.json) sob <root>; util para o modo pasta."""
    for path in sorted(Path(root).glob("*/metadata.json")):
        yield path


def copy_tree(src: str | Path, dst: str | Path) -> None:
    shutil.copytree(src, dst, dirs_exist_ok=True)
