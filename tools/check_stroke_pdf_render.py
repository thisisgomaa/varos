#!/usr/bin/env python3
"""Independent PDFium pixel regression for baked translucent v5 stroke bands.
Run with python3 tools/check_stroke_pdf_render.py (requires pypdfium2 + Pillow).
Expected samples are authored geometry/paint regions, not hashes of writer output.
"""
from pathlib import Path
import pypdfium2 as pdfium

ROOT = Path(__file__).resolve().parents[1] / "varos/crates/varos-core/tests/fixtures/v5"
FILL = (153, 194, 235)
BAND = (81, 94, 106)
WHITE = (255, 255, 255)
CASES = {
    "align_Inside": [((140, 80), FILL), ((180, 80), FILL), ((140, 42), BAND), ((236, 80), BAND)],
    "align_Outside": [((140, 80), FILL), ((140, 42), FILL), ((140, 36), (113, 113, 113)), ((246, 80), (113, 113, 113))],
    "holes": [((140, 80), WHITE), ((180, 80), BAND), ((220, 140), FILL), ((140, 42), BAND)],
}
for name, samples in CASES.items():
    with pdfium.PdfDocument(ROOT / f"{name}.pdf") as pdf:
        page = pdf[0]
        bitmap = page.render(scale=2)
        image = bitmap.to_pil().convert("RGB")
        for point, expected in samples:
            actual = image.getpixel(point)
            assert all(abs(a-b) <= 3 for a, b in zip(actual, expected)), (name, point, actual, expected)
        bitmap.close()
        page.close()
print(f"PASS: {len(CASES)} rendered PDFs, {sum(map(len, CASES.values()))} independent pixel assertions")
