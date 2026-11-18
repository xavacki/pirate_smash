#!/usr/bin/env python3
"""Trace open water on the harbor plate. Requires Pillow."""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

root = Path(__file__).resolve().parents[1]
# Coordinates trace the harbor illustration in a 720×1480 portrait preview.
outline = [
    (0, 480), (25, 492), (64, 497), (99, 494), (122, 487),
    (468, 487), (478, 496), (503, 501), (556, 502), (629, 505), (720, 505),
    (720, 828), (695, 804), (680, 799), (679, 767), (664, 754),
    (636, 754), (623, 765), (626, 807), (633, 840), (634, 875),
    (608, 880), (565, 884), (504, 881), (436, 887), (377, 882), (317, 888),
    (244, 884), (166, 887), (96, 876), (87, 858), (88, 814),
    (96, 801), (88, 779), (86, 763), (72, 754), (45, 755), (34, 766),
    (33, 797), (18, 808), (0, 835),
]
mask = Image.new("L", (720, 1480))
draw = ImageDraw.Draw(mask)
draw.polygon(outline, fill=255)
# Keep the rope and both mooring posts stable as the reflection samples nearby pixels.
draw.polygon([(0, 744), (74, 745), (86, 774), (92, 870), (0, 895)], fill=0)
draw.polygon([(720, 744), (638, 745), (620, 768), (619, 870), (720, 891)], fill=0)
mask = mask.filter(ImageFilter.MinFilter(7)).filter(ImageFilter.GaussianBlur(5.0))
texture = Image.new("RGBA", mask.size, (255, 255, 255, 255))
texture.putalpha(mask)
texture.save(root / "assets/pirate/water-mask.png", optimize=True)
