"""Dependency inspection and hardware capability detection."""

from __future__ import annotations
import importlib
import os
import shutil
import subprocess
import sys
from typing import Any, Dict, List


def check_module_version(module_name: str) -> Dict[str, Any]:
    try:
        mod = importlib.import_module(module_name)
        ver = getattr(mod, "__version__", "installed")
        path = getattr(mod, "__file__", "builtin")
        return {"installed": True, "version": str(ver), "location": str(path)}
    except ImportError as e:
        return {"installed": False, "version": None, "error": str(e)}


def check_binary(bin_name: str, version_arg: str = "-version") -> Dict[str, Any]:
    path = shutil.which(bin_name)
    if not path:
        return {"installed": False, "version": None, "location": None}

    version_str = "available"
    try:
        flags = 0
        if sys.platform == "win32":
            flags = subprocess.CREATE_NO_WINDOW
        res = subprocess.run(
            [path, version_arg],
            capture_output=True,
            text=True,
            timeout=3,
            creationflags=flags,
        )
        first_line = (res.stdout or res.stderr or "").splitlines()
        if first_line:
            version_str = first_line[0].strip()
    except Exception:
        pass

    return {"installed": True, "version": version_str, "location": path}


def get_onnx_providers() -> List[str]:
    try:
        import onnxruntime as ort
        return ort.get_available_providers()
    except Exception:
        return []


def get_full_dependency_status() -> Dict[str, Any]:
    python_packages = {
        "rembg": check_module_version("rembg"),
        "onnxruntime": check_module_version("onnxruntime"),
        "pillow": check_module_version("PIL"),
        "scipy": check_module_version("scipy"),
        "pymatting": check_module_version("pymatting"),
        "numpy": check_module_version("numpy"),
    }

    external_tools = {
        "ffmpeg": check_binary("ffmpeg", "-version"),
        "ffprobe": check_binary("ffprobe", "-version"),
        "tesseract": check_binary("tesseract", "--version"),
        "libreoffice": check_binary("soffice", "--version"),
        "blender": check_binary("blender", "--version"),
    }

    providers = get_onnx_providers()
    has_gpu = any("CUDA" in p or "Dml" in p or "DirectML" in p for p in providers)

    return {
        "python_runtime": {
            "version": sys.version,
            "executable": sys.executable,
            "platform": sys.platform,
        },
        "python_packages": python_packages,
        "external_tools": external_tools,
        "hardware": {
            "onnx_providers": providers,
            "gpu_acceleration": has_gpu,
            "recommended_provider": providers[0] if providers else "CPUExecutionProvider",
        },
    }
