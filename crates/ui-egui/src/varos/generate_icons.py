#!/usr/bin/env python3
"""Render the owner-supplied Varos icon at existing build-consumer paths.

Read-only source defaults to the installed Varos app; pass another ICNS as argv[1].
All writes, including temporary files, stay inside this worktree. Requires Pillow.
"""
from pathlib import Path
import base64
import io
import shutil
import struct
import sys
import zlib
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[4]
DIR = ROOT / 'assets/app-icon'
SOURCE = Path(sys.argv[1]) if len(sys.argv) > 1 else Path('/Applications/Varos.app/Contents/Resources/Varos.icns')
DEST = DIR / 'vectorcraft.icns'
if SOURCE.resolve() != DEST.resolve():
    shutil.copyfile(SOURCE, DEST)
with Image.open(DEST) as icon:
    ORIGINAL = icon.convert('RGBA')


def tile(size, mac=False):
    # Preserve the original icon's transparent canvas/inset, including on macOS.
    return ORIGINAL.resize((size, size), Image.Resampling.LANCZOS)


stream = io.BytesIO()
ORIGINAL.save(stream, format='PNG')
data = base64.b64encode(stream.getvalue()).decode('ascii')
# No invented vector tracing: SVG consumers get the original raster in a self-contained wrapper.
SVG = ('<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
       'width="512" height="512" viewBox="0 0 512 512">\n'
       f'<image width="512" height="512" xlink:href="data:image/png;base64,{data}"/>\n</svg>\n')
for name in ['vectorcraft.svg', 'vectorcraft-small.svg', 'hicolor/scalable/apps/ai.storyteller.vectorcraft.svg']:
    (DIR / name).write_text(SVG)
for size in [16, 24, 32, 48, 64, 128, 256, 512]:
    tile(size).save(DIR / f'hicolor/{size}x{size}/apps/ai.storyteller.vectorcraft.png')
tile(1024).save(DIR / 'vectorcraft-1024.png')
tile(512, mac=True).save(DIR / 'vectorcraft-macos-512.png')
tile(1024).save(DIR / 'vectorcraft.ico', sizes=[(s, s) for s in [16, 20, 24, 32, 40, 48, 64, 128, 256]])
print('Copied original ICNS; rendered SVG wrappers, PNG sizes and Windows ICO.')

# A neutral DMG backdrop with the original Varos mark. No font dependency or old artwork.
DMG = ROOT / 'packaging/macos/dmg'
DMG_SVG = '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="660" height="400" viewBox="0 0 660 400">\n' + '<rect width="660" height="400" fill="#efe9dc"/>\n' + '<rect width="232" height="400" fill="#141313"/>\n' + '<g transform="translate(16 100) scale(0.390625)">\n' + SVG.split('>\n', 1)[1].rsplit('</svg>', 1)[0] + '</g>\n</svg>\n'
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
