"""Production-quality AI background removal using neural network models (IS-Net, BiRefNet, u2net)."""

from __future__ import annotations
import os
import shutil
import tempfile
import time
from pathlib import Path
from typing import Any, Callable, Dict, Optional

from PIL import Image, ImageOps


class AIBackgroundRemover:
    def __init__(self, default_model: str = "isnet-general-use"):
        self.default_model = default_model
        self._sessions: Dict[str, Any] = {}

    def _get_session(self, model_name: str):
        if model_name not in self._sessions:
            try:
                import rembg
                # new_session loads the ONNX model into memory once and caches it
                session = rembg.new_session(model_name)
                self._sessions[model_name] = session
            except Exception as e:
                raise RuntimeError(
                    f"Failed to initialize AI model session '{model_name}': {e}. "
                    "Ensure onnxruntime and rembg are installed properly."
                )
        return self._sessions[model_name]

    def remove_background(
        self,
        input_path: str,
        output_path: str,
        model_name: Optional[str] = None,
        alpha_matting: bool = True,
        alpha_matting_foreground_threshold: int = 240,
        alpha_matting_background_threshold: int = 10,
        alpha_matting_erode_size: int = 10,
        post_process_mask: bool = True,
        progress_callback: Optional[Callable[[float, str], None]] = None,
    ) -> Dict[str, Any]:
        """
        Removes background from an image with production-quality AI inference.
        Preserves original image resolution and source EXIF orientation.
        Outputs transparent PNG or WebP based on output extension.
        """
        start_time = time.perf_counter()
        in_p = Path(input_path).resolve()
        out_p = Path(output_path).resolve()

        if not in_p.exists():
            raise FileNotFoundError(f"Source image not found: {in_p}")

        if progress_callback:
            progress_callback(0.1, "Opening and preparing image")

        # 1. Open image and handle EXIF orientation properly
        with Image.open(in_p) as src_img:
            img = ImageOps.exif_transpose(src_img)
            orig_width, orig_height = img.size
            orig_mode = img.mode

        if progress_callback:
            progress_callback(0.3, "Loading neural network model")

        chosen_model = model_name or self.default_model
        session = self._get_session(chosen_model)

        if progress_callback:
            progress_callback(0.5, "Running AI neural inference")

        import rembg

        # 2. Run neural segmentation with alpha matting for hair & fine detail preservation
        try:
            result_img = rembg.remove(
                img,
                session=session,
                alpha_matting=alpha_matting,
                alpha_matting_foreground_threshold=alpha_matting_foreground_threshold,
                alpha_matting_background_threshold=alpha_matting_background_threshold,
                alpha_matting_erode_size=alpha_matting_erode_size,
                post_process_mask=post_process_mask,
            )
        except Exception as e:
            # If alpha matting fails on specific edge cases, fallback to standard mask inference
            if alpha_matting:
                result_img = rembg.remove(img, session=session, post_process_mask=True)
            else:
                raise e

        if progress_callback:
            progress_callback(0.85, "Encoding transparent output")

        # 3. Ensure destination directory exists
        out_p.parent.mkdir(parents=True, exist_ok=True)

        # 4. Save to temporary output file first, then atomically replace
        temp_dir = tempfile.mkdtemp(prefix="orbit_nobg_")
        temp_file = Path(temp_dir) / out_p.name

        ext = out_p.suffix.lower()
        if ext == ".webp":
            result_img.save(temp_file, format="WEBP", lossless=True)
        else:
            # Default to transparent PNG
            result_img.save(temp_file, format="PNG", optimize=True)

        # Atomic move/replace
        shutil.move(str(temp_file), str(out_p))
        try:
            shutil.rmtree(temp_dir)
        except Exception:
            pass

        if progress_callback:
            progress_callback(1.0, "Complete")

        elapsed_ms = (time.perf_counter() - start_time) * 1000.0

        return {
            "input_file": str(in_p),
            "output_file": str(out_p),
            "model_used": chosen_model,
            "dimensions": [orig_width, orig_height],
            "processing_time_ms": round(elapsed_ms, 2),
        }


# Global lazy instance
_global_remover: Optional[AIBackgroundRemover] = None


def get_ai_remover() -> AIBackgroundRemover:
    global _global_remover
    if _global_remover is None:
        _global_remover = AIBackgroundRemover()
    return _global_remover
