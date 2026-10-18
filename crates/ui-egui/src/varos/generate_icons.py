#!/usr/bin/env python3
"""Original temporary Varos V. Run from any directory; Pillow + macOS iconutil required.
Existing filenames intentionally stay compatible with upstream build consumers.
"""
from pathlib import Path
from math import sqrt
import subprocess
import tempfile
import struct
import zlib
import io
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[4]
DIR = ROOT / 'assets/app-icon'
SVG = '''<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">
  <rect width="512" height="512" rx="108" fill="#141313"/>
  <path d="M108.201 211.799 L147.799 172.201 L256 280.402 L256 359.598 Z" fill="#eeeae5"/>
  <path d="M256 280.402 L364.201 172.201 L403.799 211.799 L256 359.598 Z" fill="#0c8ce9"/>
</svg>
'''


def tile(size, mac=False):
    # Four samples per output pixel; straight 45-degree strokes with butt caps.
    scale = 4
    canvas = Image.new('RGBA', (size * scale, size * scale))
    draw = ImageDraw.Draw(canvas)
    extent = size * scale * (824 / 1024 if mac else 1)
    offset = (size * scale - extent) / 2
    def xy(x, y):
        return offset + x / 512 * extent, offset + y / 512 * extent
    draw.rounded_rectangle((*xy(0, 0), *xy(512, 512)), radius=108 / 512 * extent, fill='#141313')
    half = 28 / sqrt(2)
    for points, color in [
        ([(128-half, 192+half), (128+half, 192-half), (256, 320-2*half), (256, 320+2*half)], '#eeeae5'),
        ([(256, 320-2*half), (384-half, 192-half), (384+half, 192+half), (256, 320+2*half)], '#0c8ce9'),
    ]:
        draw.polygon([xy(*point) for point in points], fill=color)
    return canvas.resize((size, size), Image.Resampling.LANCZOS)


def write_icns_fallback():
    # PNG-backed ICNS chunks, including both 16pt/32pt @1x and @2x entries.
    # Container bytes are big-endian; each chunk size includes its eight-byte header.
    chunks = []
    for kind, size in [(b'icp4', 16), (b'ic11', 32), (b'icp5', 32), (b'ic12', 64),
                       (b'ic07', 128), (b'ic13', 256), (b'ic08', 256), (b'ic14', 512),
                       (b'ic09', 512), (b'ic10', 1024)]:
        stream = io.BytesIO()
        tile(size, mac=True).save(stream, format='PNG')
        png = stream.getvalue()
        chunks.append(kind + struct.pack('>I', len(png)+8) + png)
    payload = b''.join(chunks)
    (DIR / 'vectorcraft.icns').write_bytes(b'icns' + struct.pack('>I', len(payload)+8) + payload)


for name in ['vectorcraft.svg', 'vectorcraft-small.svg', 'hicolor/scalable/apps/ai.storyteller.vectorcraft.svg']:
    (DIR / name).write_text(SVG)
for size in [16, 24, 32, 48, 64, 128, 256, 512]:
    tile(size).save(DIR / f'hicolor/{size}x{size}/apps/ai.storyteller.vectorcraft.png')
tile(1024).save(DIR / 'vectorcraft-1024.png')
tile(512, mac=True).save(DIR / 'vectorcraft-macos-512.png')
tile(1024).save(DIR / 'vectorcraft.ico', sizes=[(s, s) for s in [16, 20, 24, 32, 40, 48, 64, 128, 256]])
# Temporary files must stay inside this worktree.
with tempfile.TemporaryDirectory(dir=DIR) as tmp:
    iconset = Path(tmp) / 'varos.iconset'
    iconset.mkdir()
    for size in [16, 32, 128, 256, 512]:
        tile(size, mac=True).save(iconset / f'icon_{size}x{size}.png')
        tile(size*2, mac=True).save(iconset / f'icon_{size}x{size}@2x.png')
    result = subprocess.run(['iconutil', '-c', 'icns', '-o', str(DIR / 'vectorcraft.icns'), str(iconset)])
    if result.returncode:
        # Some managed macOS hosts reject valid iconsets; write all ten PNG-backed entries.
        write_icns_fallback()
print('Regenerated SVG, PNG, ICO and ICNS placeholders.')

# A neutral DMG backdrop with the same original mark. No font dependency or old artwork.
DMG = ROOT / 'packaging/macos/dmg'
DMG_SVG = '<svg xmlns="http://www.w3.org/2000/svg" width="660" height="400" viewBox="0 0 660 400">\n' + '<rect width="660" height="400" fill="#efe9dc"/>\n' + '<rect width="232" height="400" fill="#141313"/>\n' + '<g transform="translate(16 100) scale(0.390625)">\n' + SVG.split('>\n', 1)[1].rsplit('</svg>', 1)[0] + '</g>\n</svg>\n'
(DMG / 'background.svg').write_text(DMG_SVG)
# A minimal TIFF writer keeps the per-page DPI exact on older Pillow releases.
# TIFF 6.0 RGB strips, compression 8 (zlib), no third-party profile or metadata.
pages = []
for scale in [1, 2]:
    page = Image.new('RGB', (660*scale, 400*scale), '#efe9dc')
    ImageDraw.Draw(page).rectangle((0, 0, 232*scale-1, 400*scale), fill='#141313')
    mark = tile(200*scale)
    page.paste(mark, (16*scale, 100*scale), mark)
    pages.append((page, 72*scale))
out = bytearray(b'II' + struct.pack('<HI', 42, 8))
previous_next = None
for page, dpi in pages:
    if len(out) % 2:
        out.append(0)
    start = len(out)
    if previous_next is not None:
        struct.pack_into('<I', out, previous_next, start)
    tags = 13
    extra = start + 2 + tags*12 + 4
    pixels = zlib.compress(page.tobytes())
    entries = [(256, 4, 1, page.width), (257, 4, 1, page.height),
               (258, 3, 3, extra), (259, 3, 1, 8), (262, 3, 1, 2),
               (273, 4, 1, extra+14), (277, 3, 1, 3), (278, 4, 1, page.height),
               (279, 4, 1, len(pixels)), (282, 5, 1, extra+6),
               (283, 5, 1, extra+6), (284, 3, 1, 1), (296, 3, 1, 2)]
    out.extend(struct.pack('<H', tags))
    for entry in entries:
        out.extend(struct.pack('<HHII', *entry))
    previous_next = len(out)
    out.extend(struct.pack('<I', 0))
    out.extend(struct.pack('<HHHII', 8, 8, 8, dpi, 1))
    out.extend(pixels)
(DMG / 'background.tiff').write_bytes(out)
print('Regenerated neutral DMG background at 1x/72 dpi and 2x/144 dpi.')
