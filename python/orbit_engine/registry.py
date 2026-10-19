"""Unified capability registry matching Rust capability expectations."""

from __future__ import annotations
from typing import Any, Dict, List

from .dependencies import get_full_dependency_status


def get_engine_capabilities() -> Dict[str, Any]:
    deps = get_full_dependency_status()
    rembg_installed = deps["python_packages"]["rembg"]["installed"]
    onnx_installed = deps["python_packages"]["onnxruntime"]["installed"]
    has_ai_bg = rembg_installed and onnx_installed

    capabilities = {
        "engine": "OrbitEngine-Python",
        "version": "0.1.0",
        "quick_actions": {
            "remove_background": {
                "available": has_ai_bg,
                "engine": "NeuralNetwork-ONNX",
                "models": ["isnet-general-use", "birefnet-general", "u2net"],
                "default_model": "isnet-general-use",
                "supported_inputs": [
                    "jpg", "jpeg", "png", "webp", "bmp", "tiff", "tif", "avif"
                ],
                "supported_outputs": ["png", "webp"],
                "features": [
                    "alpha_matting",
                    "fine_hair_preservation",
                    "exif_orientation_preservation",
                    "edge_refinement",
                ],
                "options": {
                    "model": {"type": "string", "default": "isnet-general-use"},
                    "alpha_matting": {"type": "bool", "default": True},
                    "foreground_threshold": {"type": "int", "default": 240, "range": [0, 255]},
                    "background_threshold": {"type": "int", "default": 10, "range": [0, 255]},
                    "erode_size": {"type": "int", "default": 10, "range": [0, 50]},
                },
            },
            "ai_upscale": {
                "available": False,  # Phase 3
                "engine": "Real-ESRGAN",
                "supported_scales": [2, 4],
            },
        },
        "conversions": {
            "images": {
                "available": deps["python_packages"]["pillow"]["installed"],
                "formats": ["png", "jpg", "jpeg", "webp", "bmp", "tiff", "ico"],
            },
            "pdf": {
                "available": False,  # Phase 5
            },
            "ocr": {
                "available": deps["external_tools"]["tesseract"]["installed"],  # Phase 5
            },
        },
        "dependencies": deps,
    }

    return capabilities
