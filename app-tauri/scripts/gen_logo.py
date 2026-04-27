#!/usr/bin/env python3
"""生成 Aura Trade 应用图标（1024x1024 PNG）
风格：深色圆角方形 + 黄金光环 + 上升趋势蜡烛图
依赖：Pillow
用法：python3 gen_logo.py [out.png]
"""
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

SIZE = 1024
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).parent.parent / "icons" / "source.png"

# —— 配色（与前端主题一致）——
BG_TOP   = (18, 22, 30, 255)     # 顶部深蓝黑
BG_BOT   = (8, 10, 14, 255)      # 底部更暗
BULL     = (61, 160, 110, 255)   # 涨绿
BEAR     = (200, 92, 86, 255)    # 跌红
ACCENT   = (120, 160, 210, 255)  # 冷蓝（光环内圈）
GOLD     = (220, 175, 95, 255)   # 暖金（光环外圈）

# —— 1. 底板：圆角方形渐变 ——
img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))

bg = Image.new("RGBA", (SIZE, SIZE), BG_BOT)
bg_draw = ImageDraw.Draw(bg)
for y in range(SIZE):
    t = y / SIZE
    r = int(BG_TOP[0] * (1 - t) + BG_BOT[0] * t)
    g = int(BG_TOP[1] * (1 - t) + BG_BOT[1] * t)
    b = int(BG_TOP[2] * (1 - t) + BG_BOT[2] * t)
    bg_draw.line([(0, y), (SIZE, y)], fill=(r, g, b, 255))

# 圆角 mask（Apple 推荐 squircle ≈ 22.5% 半径）
R = int(SIZE * 0.225)
mask = Image.new("L", (SIZE, SIZE), 0)
ImageDraw.Draw(mask).rounded_rectangle([0, 0, SIZE - 1, SIZE - 1], R, fill=255)
img.paste(bg, (0, 0), mask)

# —— 2. 光环（radial glow）——
halo = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
hdraw = ImageDraw.Draw(halo)
CX, CY = SIZE // 2, SIZE // 2 + 20
# 外圈暖金
for r in range(460, 200, -10):
    alpha = int(55 * (460 - r) / 260) + 5
    hdraw.ellipse([CX - r, CY - r, CX + r, CY + r], fill=(*GOLD[:3], alpha))
halo = halo.filter(ImageFilter.GaussianBlur(55))
# 内圈冷蓝小光点
halo2 = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
h2 = ImageDraw.Draw(halo2)
for r in range(260, 80, -8):
    alpha = int(65 * (260 - r) / 180) + 5
    h2.ellipse([CX - r, CY - r, CX + r, CY + r], fill=(*ACCENT[:3], alpha))
halo2 = halo2.filter(ImageFilter.GaussianBlur(35))
img = Image.alpha_composite(img, halo)
img = Image.alpha_composite(img, halo2)

# —— 3. 蜡烛图（上升趋势，5 根）——
# 每根：x 中心、body 高度、wick 高度、颜色
# 让整体向右上方倾斜
candles_layout = [
    # (x, body_top, body_bottom, wick_top, wick_bottom, color)
    (280, 620, 760, 580, 800, BEAR),
    (420, 500, 660, 460, 700, BULL),
    (560, 420, 580, 380, 620, BULL),
    (700, 320, 500, 280, 540, BULL),
    (840, 360, 480, 320, 520, BEAR),
]

CW = 96  # 蜡烛体宽
WW = 10  # 上下影线宽

candle_layer = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
cdraw = ImageDraw.Draw(candle_layer)

for (x, bt, bb, wt, wb, color) in candles_layout:
    # 影线（实色）
    cdraw.rounded_rectangle(
        [x - WW // 2, wt, x + WW // 2, wb], radius=4, fill=color,
    )
    # 实体 + 内部高光
    cdraw.rounded_rectangle(
        [x - CW // 2, bt, x + CW // 2, bb], radius=12, fill=color,
    )
    # 内部柔和高光（左上角）
    gloss = (min(color[0] + 40, 255), min(color[1] + 40, 255), min(color[2] + 40, 255), 90)
    cdraw.rounded_rectangle(
        [x - CW // 2 + 8, bt + 6, x - CW // 2 + 34, bb - 6],
        radius=8, fill=gloss,
    )

img = Image.alpha_composite(img, candle_layer)

# —— 4. 轻微外阴影 + 抗锯齿收尾 ——
# 在整张图的圆角边沿内侧加一层 1px 的内描边，提升清晰感
stroke = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
sdraw = ImageDraw.Draw(stroke)
sdraw.rounded_rectangle(
    [6, 6, SIZE - 7, SIZE - 7], R - 4,
    outline=(255, 255, 255, 18), width=2,
)
img = Image.alpha_composite(img, stroke)

# —— 5. 保存 ——
OUT.parent.mkdir(parents=True, exist_ok=True)
img.save(OUT, "PNG", optimize=True)
print(f"✅ Logo written to {OUT}  ({OUT.stat().st_size / 1024:.1f} KB)")
