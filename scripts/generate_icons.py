#!/usr/bin/env python3
"""Procedural icon and logo generator for Z-Engine.

Generates pixel-perfect Apple-grade desktop app icons (.icns, .ico, PNGs)
and in-app vector assets from pure mathematical geometry and shaders.
"""

import os
import shutil
import subprocess
from pathlib import Path
from PIL import Image

REPO_ROOT = Path(__file__).resolve().parent.parent
ICONS_DIR = REPO_ROOT / "crates/z-engine-gui/src-tauri/icons"
UI_PUBLIC_DIR = REPO_ROOT / "crates/z-engine-gui/ui/public"

# Master SVG Definition (512x512 canvas)
# Continuous Apple squircle, liquid titanium kinetic Z, radiant solar core
MASTER_SVG = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <defs>
    <!-- Deep Obsidian Studio Canvas -->
    <linearGradient id="apple-bg" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#26262A" />
      <stop offset="50%" stop-color="#1B1B1E" />
      <stop offset="100%" stop-color="#101012" />
    </linearGradient>

    <!-- Squircle Specular Chamfer Highlight (Continuous Edge Reflection) -->
    <linearGradient id="squircle-bevel" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.25" />
      <stop offset="12%" stop-color="#FFFFFF" stop-opacity="0.08" />
      <stop offset="85%" stop-color="#FFFFFF" stop-opacity="0.02" />
      <stop offset="100%" stop-color="#FFFFFF" stop-opacity="0.07" />
    </linearGradient>

    <!-- Liquid Titanium Z Mark Gradient (Directional studio light) -->
    <linearGradient id="ti-body" x1="15%" y1="10%" x2="85%" y2="90%">
      <stop offset="0%" stop-color="#FFFFFF" />
      <stop offset="22%" stop-color="#ECECF2" />
      <stop offset="55%" stop-color="#C5C5D0" />
      <stop offset="85%" stop-color="#A5A5B2" />
      <stop offset="100%" stop-color="#8E8E9B" />
    </linearGradient>

    <!-- Specular Hairline Bevel for Z Mark -->
    <linearGradient id="ti-edge" x1="20%" y1="0%" x2="80%" y2="100%">
      <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.80" />
      <stop offset="35%" stop-color="#FFFFFF" stop-opacity="0.20" />
      <stop offset="70%" stop-color="#FFFFFF" stop-opacity="0.05" />
      <stop offset="100%" stop-color="#FFFFFF" stop-opacity="0.35" />
    </linearGradient>

    <!-- Radiant Solar Core Orb Gradient -->
    <radialGradient id="solar-sphere" cx="34%" cy="30%" r="70%">
      <stop offset="0%" stop-color="#FFB366" />
      <stop offset="30%" stop-color="#FF7322" />
      <stop offset="70%" stop-color="#EA4400" />
      <stop offset="95%" stop-color="#BE2800" />
      <stop offset="100%" stop-color="#9C1E00" />
    </radialGradient>

    <!-- Ambient Solar Backlight Bloom -->
    <filter id="solar-bloom" x="-60%" y="-60%" width="220%" height="220%">
      <feGaussianBlur stdDeviation="24" result="glow" />
    </filter>

    <!-- Multi-stage Depth Drop Shadows -->
    <filter id="z-shadow" x="-30%" y="-30%" width="160%" height="160%">
      <feDropShadow dx="0" dy="18" stdDeviation="20" flood-color="#000000" flood-opacity="0.55" />
      <feDropShadow dx="0" dy="6" stdDeviation="8" flood-color="#000000" flood-opacity="0.35" />
      <feDropShadow dx="0" dy="2" stdDeviation="2" flood-color="#000000" flood-opacity="0.25" />
    </filter>

    <filter id="orb-shadow" x="-30%" y="-30%" width="160%" height="160%">
      <feDropShadow dx="0" dy="14" stdDeviation="16" flood-color="#000000" flood-opacity="0.50" />
      <feDropShadow dx="0" dy="4" stdDeviation="6" flood-color="#000000" flood-opacity="0.30" />
    </filter>
  </defs>

  <!-- Apple Standard Continuous Squircle Base -->
  <rect width="512" height="512" rx="116" fill="url(#apple-bg)" />
  <rect x="1" y="1" width="510" height="510" rx="115" fill="none" stroke="url(#squircle-bevel)" stroke-width="2" />

  <!-- Ambient Solar Flare Reflection onto Dark Titanium Bed -->
  <circle cx="356" cy="148" r="76" fill="#FF5500" opacity="0.20" filter="url(#solar-bloom)" />

  <!-- Sculptural Liquid Titanium Z Mark -->
  <g filter="url(#z-shadow)">
    <path d="
      M 144 110
      C 185 110, 225 110, 248 110
      C 266 110, 278 120, 280 136
      C 282 152, 275 168, 260 192
      C 236 228, 206 278, 184 318
      C 176 330, 184 338, 200 338
      L 364 338
      C 386 338, 404 354, 404 372
      C 404 391, 386 406, 364 406
      L 144 406
      C 122 406, 104 391, 104 372
      C 104 354, 114 340, 128 322
      C 152 290, 182 240, 204 200
      C 212 188, 204 180, 188 180
      L 144 180
      C 122 180, 104 165, 104 145
      C 104 125, 122 110, 144 110 Z
    " fill="url(#ti-body)" />

    <path d="
      M 144 110
      C 185 110, 225 110, 248 110
      C 266 110, 278 120, 280 136
      C 282 152, 275 168, 260 192
      C 236 228, 206 278, 184 318
      C 176 330, 184 338, 200 338
      L 364 338
      C 386 338, 404 354, 404 372
      C 404 391, 386 406, 364 406
      L 144 406
      C 122 406, 104 391, 104 372
      C 104 354, 114 340, 128 322
      C 152 290, 182 240, 204 200
      C 212 188, 204 180, 188 180
      L 144 180
      C 122 180, 104 165, 104 145
      C 104 125, 122 110, 144 110 Z
    " fill="none" stroke="url(#ti-edge)" stroke-width="1.5" />
  </g>

  <!-- Radiant Solar Energy Core (The Engine Combustion Core) -->
  <g filter="url(#orb-shadow)">
    <circle cx="356" cy="148" r="60" fill="url(#solar-sphere)" />
    <circle cx="356" cy="148" r="59.5" fill="none" stroke="#FFB366" stroke-opacity="0.45" stroke-width="1" />
    <ellipse cx="340" cy="128" rx="19" ry="10" transform="rotate(-26 340 128)" fill="#FFFFFF" opacity="0.38" />
  </g>
</svg>
"""


def render_svg_to_png(svg_path: Path, png_path: Path, width: int, height: int):
    """Render SVG to raster PNG using rsvg-convert."""
    subprocess.run(
        ["rsvg-convert", "-w", str(width), "-h", str(height), str(svg_path), "-o", str(png_path)],
        check=True,
    )


def generate_icns(master_png: Path, icns_path: Path):
    """Build macOS .icns using iconutil."""
    tmp_iconset = icns_path.parent / "tmp.iconset"
    if tmp_iconset.exists():
        shutil.rmtree(tmp_iconset)
    tmp_iconset.mkdir(parents=True)

    iconset_specs = [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ]

    master = Image.open(master_png)
    for filename, size in iconset_specs:
        resized = master.resize((size, size), Image.Resampling.LANCZOS)
        resized.save(tmp_iconset / filename)

    subprocess.run(["iconutil", "-c", "icns", str(tmp_iconset), "-o", str(icns_path)], check=True)
    shutil.rmtree(tmp_iconset)


def generate_ico(master_png: Path, ico_path: Path):
    """Build multi-resolution Windows .ico file."""
    master = Image.open(master_png)
    sizes = [(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    master.save(str(ico_path), format="ICO", sizes=sizes)


def main():
    ICONS_DIR.mkdir(parents=True, exist_ok=True)
    UI_PUBLIC_DIR.mkdir(parents=True, exist_ok=True)

    # 1. Write SVG assets
    svg_icon_path = ICONS_DIR / "icon.svg"
    favicon_path = UI_PUBLIC_DIR / "favicon.svg"

    svg_icon_path.write_text(MASTER_SVG, encoding="utf-8")
    favicon_path.write_text(MASTER_SVG, encoding="utf-8")
    print(f"Wrote {svg_icon_path} and {favicon_path}")

    # 2. Render 1024x1024 master raster PNG
    master_png_path = ICONS_DIR / "icon-1024.png"
    render_svg_to_png(svg_icon_path, master_png_path, 1024, 1024)

    # 3. Generate standard PNG sizes
    master_img = Image.open(master_png_path)
    target_sizes = {
        "icon.png": (512, 512),
        "icon-512.png": (512, 512),
        "icon-256.png": (256, 256),
        "128x128.png": (128, 128),
        "128x128@2x.png": (256, 256),
        "64x64.png": (64, 64),
        "32x32.png": (32, 32),
        # Windows Appx Logos
        "StoreLogo.png": (50, 50),
        "Square30x30Logo.png": (30, 30),
        "Square44x44Logo.png": (44, 44),
        "Square71x71Logo.png": (71, 71),
        "Square89x89Logo.png": (89, 89),
        "Square107x107Logo.png": (107, 107),
        "Square142x142Logo.png": (142, 142),
        "Square150x150Logo.png": (150, 150),
        "Square284x284Logo.png": (284, 284),
        "Square310x310Logo.png": (310, 310),
    }

    for fname, size in target_sizes.items():
        out_file = ICONS_DIR / fname
        resized = master_img.resize(size, Image.Resampling.LANCZOS)
        resized.save(out_file)
        print(f"Generated {out_file.name} ({size[0]}x{size[1]})")

    # 4. Generate native macOS icon.icns
    icns_path = ICONS_DIR / "icon.icns"
    generate_icns(master_png_path, icns_path)
    print(f"Generated {icns_path.name}")

    # 5. Generate Windows icon.ico
    ico_path = ICONS_DIR / "icon.ico"
    generate_ico(master_png_path, ico_path)
    print(f"Generated {ico_path.name}")

    # Remove temporary 1024 png if not strictly required by Tauri
    if master_png_path.exists():
        master_png_path.unlink()

    print("Procedural icon generation completed successfully!")


if __name__ == "__main__":
    main()
