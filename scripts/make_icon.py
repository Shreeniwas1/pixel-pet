"""Draw the app icon from the cat sprite. Run: python scripts/make_icon.py  (needs Pillow)"""
from PIL import Image

GRID = [
    ".KK......KK.",
    ".KOK....KOK.",
    ".KOOKKKKOOK.",
    "KOOOOOOOOOOK",
    "KOOEOOOOEOOK",
    "KOOOOPPOOOOK",
    "KOWWOOOOWWOK",
    ".KOOOOOOOOK.",
    ".KOOOOOOOOK.",
    ".KWWKOOKWWK.",
    "..KKK..KKK..",
]
COL = {"K": (0x2B, 0x21, 0x40), "E": (0x2B, 0x21, 0x40), "O": (0xF2, 0xA6, 0x5A),
       "P": (0xFF, 0x8F, 0xA3), "W": (0xFF, 0xF1, 0xD6)}

def render(size):
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    gw, gh = len(GRID[0]), len(GRID)
    px = max(1, (size * 9 // 10) // gw)
    ox, oy = (size - gw * px) // 2, (size - gh * px) // 2
    for j, row in enumerate(GRID):
        for i, ch in enumerate(row):
            if ch in COL:
                for y in range(px):
                    for x in range(px):
                        img.putpixel((ox + i * px + x, oy + j * px + y), COL[ch] + (255,))
    return img

sizes = [16, 24, 32, 48, 64, 128, 256]
base = render(256)
base.save("assets/pixel-pet.ico", sizes=[(s, s) for s in sizes],
          append_images=[render(s) for s in sizes if s != 256])
print("wrote assets/pixel-pet.ico")
