"""Image conversions and transformations using Pillow."""

from pathlib import Path
from PIL import Image, ImageOps


def convert_image(input_path: str, output_path: str, format_name: str, quality: int = 90) -> str:
    in_p = Path(input_path).resolve()
    out_p = Path(output_path).resolve()
    out_p.parent.mkdir(parents=True, exist_ok=True)

    with Image.open(in_p) as img:
        transposed = ImageOps.exif_transpose(img)
        save_kwargs = {}
        fmt = format_name.upper()
        if fmt in ("JPG", "JPEG"):
            fmt = "JPEG"
            if transposed.mode in ("RGBA", "LA", "P"):
                transposed = transposed.convert("RGB")
            save_kwargs["quality"] = quality
        elif fmt == "WEBP":
            save_kwargs["quality"] = quality

        transposed.save(out_p, format=fmt, **save_kwargs)

    return str(out_p)
