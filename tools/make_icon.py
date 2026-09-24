# -*- coding: utf-8 -*-
"""生成 rdesktop 工具图标:
深色玻璃圆角方底 + 四色分组磁贴(文件夹=琥珀 / 文件=浅白 / 快捷方式=蓝 / 快捷功能=紫),
对应工具的四块分组面板。4x 超采样抗锯齿,输出多尺寸 ico +256 预览图。
"""
import os
from PIL import Image, ImageDraw

OUT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'assets')
PREV = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'icon_preview.png')
ICO = os.path.join(OUT_DIR, 'rdesktop.ico')

BASE = 1024          # 基准画布
SS = 4               # 超采样倍数
BIG = BASE * SS      # 实际绘制尺寸


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def vgrad(size, top, bottom):
    """垂直渐变"""
    img = Image.new('RGB', (1, size))
    px = img.load()
    for y in range(size):
        px[0, y] = lerp(top, bottom, y / max(1, size - 1))
    return img.resize((size, size), Image.NEAREST)


def rounded_mask(size, radius):
    m = Image.new('L', (size, size), 0)
    d = ImageDraw.Draw(m)
    d.rounded_rectangle([0, 0, size - 1, size - 1], radius=radius, fill=255)
    return m


def draw():
    S = BIG
    # ---------- 底板:深色玻璃圆角方 ----------
    base = vgrad(S, (34, 38, 48), (13, 15, 20)).convert('RGBA')
    # 顶部高光
    hl = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    hd = ImageDraw.Draw(hl)
    hd.ellipse([-S * 0.25, -S * 0.55, S * 1.25, S * 0.35], fill=(255, 255, 255, 26))
    base = Image.alpha_composite(base, hl)

    r = int(S * 0.225)
    mask = rounded_mask(S, r)
    icon = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    icon.paste(base, (0, 0), mask)

    d = ImageDraw.Draw(icon)

    # 内描边(玻璃边缘)
    d.rounded_rectangle([3, 3, S - 4, S - 4], radius=r, outline=(255, 255, 255, 34), width=int(S * 0.008))

    # ---------- 四块磁贴 ----------
    m = int(S * 0.155)      # 外边距
    gap = int(S * 0.045)
    tile = (S - 2 * m - gap) // 2
    tr = int(tile * 0.22)   # 磁贴圆角

    def tile_pos(i, j):
        return (m + j * (tile + gap), m + i * (tile + gap))

    def grad_tile(top, bottom):
        g = vgrad(tile, top, bottom).convert('RGBA')
        tm = rounded_mask(tile, tr)
        layer = Image.new('RGBA', (S, S), (0, 0, 0, 0))
        layer.paste(g, tile_pos(0, 0), None)
        # 用磁贴蒙版裁
        cut = Image.new('RGBA', (S, S), (0, 0, 0, 0))
        cut.paste(g.convert('RGBA'), tile_pos(0, 0))
        out = Image.new('RGBA', (S, S), (0, 0, 0, 0))
        # 组合:先把磁贴画到独立小图再贴
        timg = Image.new('RGBA', (tile, tile), (0, 0, 0, 0))
        timg.paste(g.convert('RGBA'), (0, 0))
        timg.putalpha(tm)
        icon.alpha_composite(timg, tile_pos(0, 0))
        return timg

    def place_tile(i, j, top, bottom):
        x, y = tile_pos(i, j)
        g = vgrad(tile, top, bottom).convert('RGBA')
        tm = rounded_mask(tile, tr)
        g.putalpha(tm)
        icon.alpha_composite(g, (x, y))
        # 上边缘高光
        dd = ImageDraw.Draw(icon)
        dd.rounded_rectangle([x + 2, y + 2, x + tile - 3, y + tile - 3],
                             radius=tr, outline=(255, 255, 255, 40), width=int(S * 0.006))

    # TL 文件夹(琥珀) / TR 文件(浅白) / BL 快捷方式(蓝) / BR 快捷功能(紫)
    place_tile(0, 0, (255, 196, 74), (240, 150, 25))
    place_tile(0, 1, (243, 245, 248), (214, 218, 224))
    place_tile(1, 0, (64, 156, 255), (20, 106, 235))
    place_tile(1, 1, (156, 123, 255), (110, 72, 240))

    # ---------- 磁贴上的白色字形 ----------
    def glyph_color(alpha=245):
        return (255, 255, 255, alpha)

    def dark_color(alpha=235):
        return (52, 57, 64, alpha)

    # 文件夹:白色 folder(带小凸页)
    x, y = tile_pos(0, 0)
    gx, gy = x + tile * 0.22, y + tile * 0.30
    gw, gh = tile * 0.56, tile * 0.42
    d.rounded_rectangle([gx, gy - gh * 0.18, gx + gw * 0.42, gy + gh * 0.10],
                        radius=tile * 0.05, fill=glyph_color(220))
    d.rounded_rectangle([gx - tile * 0.02, gy, gx + gw, gy + gh],
                        radius=tile * 0.07, fill=glyph_color(245))

    # 文件:白色纸张(右上折角)+ 灰线
    x, y = tile_pos(0, 1)
    pw, ph = tile * 0.46, tile * 0.58
    px0, py0 = x + (tile - pw) / 2, y + (tile - ph) / 2
    fold = tile * 0.16
    d.polygon([(px0, py0), (px0 + pw - fold, py0), (px0 + pw, py0 + fold),
               (px0 + pw, py0 + ph), (px0, py0 + ph)],
              fill=(255, 255, 255, 250))
    d.polygon([(px0 + pw - fold, py0), (px0 + pw, py0 + fold), (px0 + pw - fold, py0 + fold)],
              fill=(196, 202, 210, 255))
    for k in range(3):
        ly = py0 + ph * (0.42 + k * 0.17)
        d.rounded_rectangle([px0 + pw * 0.16, ly, px0 + pw * 0.84, ly + tile * 0.045],
                            radius=tile * 0.02, fill=(110, 118, 128, 255))

    # 快捷方式:白色 ↗ 箭头(粗杆 + 干净的角状箭头 + 拖尾点)
    x, y = tile_pos(1, 0)
    cx, cy = x + tile / 2, y + tile / 2
    L = tile * 0.30
    wdt = max(2, int(tile * 0.085))
    ex, ey = cx + L * 0.48, cy - L * 0.48   # 箭头尖端
    d.line([(cx - L * 0.62, cy + L * 0.62), (ex, ey)],
           fill=glyph_color(), width=wdt)
    # 角状箭头(尖端向右上)
    d.line([(ex - L * 0.52, ey), (ex, ey), (ex, ey + L * 0.52)],
           fill=glyph_color(), width=wdt, joint='curve')
    # 拖尾两点(沿反方向渐隐)
    rr = tile * 0.05
    for t, a in [(0.0, 200), (0.3, 120)]:
        ox = cx - L * (0.86 + t * 0.42)
        oy = cy + L * (0.86 + t * 0.42)
        d.ellipse([ox - rr, oy - rr, ox + rr, oy + rr], fill=(255, 255, 255, a))

    # 快捷功能:白色闪电
    x, y = tile_pos(1, 1)
    cx, cy = x + tile / 2, y + tile / 2 + tile * 0.02
    s = tile * 0.34
    d.polygon([(cx + s * 0.18, cy - s * 1.0), (cx - s * 0.52, cy + s * 0.12),
               (cx - s * 0.06, cy + s * 0.12), (cx - s * 0.18, cy + s * 1.0),
               (cx + s * 0.52, cy - s * 0.16), (cx + s * 0.04, cy - s * 0.16)],
              fill=glyph_color())

    # ---------- 降采样 ----------
    final = icon.resize((BASE, BASE), Image.LANCZOS)
    final.save(PREV)

    sizes = [(16, 16), (20, 20), (24, 24), (32, 32), (40, 40), (48, 48), (64, 64), (128, 128), (256, 256)]
    imgs = [final.resize(sz, Image.LANCZOS) for sz in sizes]
    os.makedirs(OUT_DIR, exist_ok=True)
    imgs[-1].save(ICO, format='ICO', sizes=sizes, append_images=imgs[:-1])
    print('saved', ICO)
    print('preview', PREV)


if __name__ == '__main__':
    draw()
