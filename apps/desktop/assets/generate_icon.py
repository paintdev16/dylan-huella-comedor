"""Genera el icono de Windows; requiere Pillow."""
from pathlib import Path
from PIL import Image, ImageDraw

out = Path(__file__).parent
scale = 3
im = Image.new("RGBA", (256 * scale, 256 * scale), (0, 0, 0, 0))
d = ImageDraw.Draw(im)
def box(coords):
    return tuple(round(v * scale) for v in coords)

d.rounded_rectangle(box((8, 8, 248, 248)), radius=54 * scale, fill="#2563eb")
# Plato, tenedor y cuchara: trazos gruesos para conservar legibilidad a 16 px.
d.ellipse(box((52, 48, 204, 200)), outline="white", width=10 * scale)
d.rounded_rectangle(box((86, 83, 94, 170)), radius=4 * scale, fill="white")
for x in (78, 88, 98):
    d.rounded_rectangle(box((x, 76, x + 6, 113)), radius=3 * scale, fill="white")
d.rounded_rectangle(box((78, 101, 104, 115)), radius=6 * scale, fill="white")
d.ellipse(box((143, 76, 167, 117)), fill="white")
d.rounded_rectangle(box((151, 108, 159, 170)), radius=4 * scale, fill="white")
im = im.resize((256, 256), Image.Resampling.LANCZOS)
im.save(out / "app.png")
im.save(out / "app.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
