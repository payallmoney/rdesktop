# -*- coding: utf-8 -*-
"""multi-select e2e: plain/shift/ctrl selection + marquee, via posted input"""
import ctypes as ct
import ctypes.wintypes as w
import time
import os
import re

u32 = ct.c_uint32
user32 = ct.windll.user32
gdi32 = ct.windll.gdi32
user32.SetProcessDpiAwarenessContext(ct.c_void_p(-4))
WM_MOUSEWHEEL = 0x020A
MARGIN, PAD, GT, CW, CH = 26, 16, 46, 132, 86

def find(cls):
    out = []
    @ct.WINFUNCTYPE(ct.c_bool, w.HWND, w.LPARAM)
    def cb(h, lp):
        b = ct.create_unicode_buffer(64); user32.GetClassNameW(h, b, 64)
        if b.value == cls:
            out.append(h if isinstance(h, int) else h.value)
        return True
    user32.EnumWindows(cb, 0)
    return out

def rect(h):
    r = w.RECT(); user32.GetWindowRect(ct.c_void_p(h), ct.byref(r))
    return (r.left, r.top, r.right - r.left, r.bottom - r.top)

def pack(x, y):
    x, y = int(x), int(y)
    return ((y & 0xFFFF) << 16) | (x & 0xFFFF)

from PIL import Image

def grab(h, box):
    L, T, W, H = rect(h)
    hdc = user32.GetDC(0); mem = gdi32.CreateCompatibleDC(hdc)
    class BMIH(ct.Structure):
        _fields_ = [('biSize', u32), ('biWidth', ct.c_int32), ('biHeight', ct.c_int32),
                    ('biPlanes', ct.c_uint16), ('biBitCount', ct.c_uint16), ('biCompression', u32),
                    ('biSizeImage', u32), ('biXPelsPerMeter', ct.c_int32), ('biYPelsPerMeter', ct.c_int32),
                    ('biClrUsed', u32), ('biClrImportant', u32)]
    class BMI(ct.Structure):
        _fields_ = [('h', BMIH), ('c', u32 * 3)]
    bmi = BMI(); bmi.h.biSize = ct.sizeof(BMIH); bmi.h.biWidth = W; bmi.h.biHeight = -H
    bmi.h.biPlanes = 1; bmi.h.biBitCount = 32
    bits = ct.c_void_p()
    hbm = gdi32.CreateDIBSection(mem, ct.byref(bmi), 0, ct.byref(bits), None, 0)
    old = gdi32.SelectObject(mem, hbm)
    user32.PrintWindow(h, mem, 2)
    data = ct.string_at(bits.value, W * H * 4)
    gdi32.SelectObject(mem, old); gdi32.DeleteObject(hbm); gdi32.DeleteDC(mem)
    user32.ReleaseDC(0, hdc)
    img = Image.frombuffer('RGBA', (W, H), data, 'raw', 'BGRA', 0, 1).convert('RGB')
    return img.crop(box) if box else img

def cell_client(gi_rowcol_to_none, row, col):
    return (MARGIN + PAD + col * CW, MARGIN + GT + row * CH)

def cell_bright(h, row, col, rows_n=1):
    """测量 row..row+rows_n 单元格区域的高亮占比(选中底纹会更亮)"""
    im = grab(h, None)
    acc = 0; tot = 0
    for r in range(row, min(row + rows_n, 40)):
        x0 = MARGIN + PAD + col * CW + 6
        y0 = MARGIN + GT + r * CH + 2
        box = (x0, y0, x0 + CW - 12, y0 + 58)
        if box[2] >= rect(h)[2] - 10 or box[3] >= rect(h)[3] - 8:
            break
        c = im.crop(box); px = list(c.getdata())
        acc += sum(1 for rr, gg, bb in px if min(rr, gg, bb) > 90)
        tot += len(px)
    return acc / max(tot, 1)

panels = [p for p in find('RdGroupPanel')
          if not (user32.GetWindowLongPtrW(ct.c_void_p(p), -20) & 0x20)]
target = max(panels, key=lambda p: rect(p)[2])
W, Hh = rect(target)[2], rect(target)[3]
print('target 0x%x %dx%d' % (target, W, Hh))
res = []

def click_cell(row, col, flags=0):
    x = MARGIN + PAD + col * CW + CW // 2
    y = MARGIN + GT + row * CH + CH // 2
    user32.PostMessageW(ct.c_void_p(target), 0x201, flags, pack(x, y))
    time.sleep(0.06)
    user32.PostMessageW(ct.c_void_p(target), 0x202, flags, pack(x, y))
    time.sleep(0.2)

def log_count():
    import re
    txt = open(r'C:/Users/oyx/AppData/Local/Temp/rdesktop.log', encoding='utf-8', errors='replace').read()
    ns = re.findall(r'multi n=(\d+)', txt)
    return int(ns[-1]) if ns else -1
def log_marquee():
    txt = open(r'C:/Users/oyx/AppData/Local/Temp/rdesktop.log', encoding='utf-8', errors='replace').read()
    import re
    ns = re.findall(r'marquee sel n=(\d+)', txt)
    return int(ns[-1]) if ns else -1

res = []
LOG0 = os.path.getsize(r'C:/Users/oyx/AppData/Local/Temp/rdesktop.log')

def log_tail():
    with open(r'C:/Users/oyx/AppData/Local/Temp/rdesktop.log', encoding='utf-8', errors='replace') as f:
        f.seek(LOG0)
        return f.read()

# 0) 单击 item(0,0) → multi n=1(shift/ctrl 路径之外的普通单击不写 multi 日志,用后续验证)
click_cell(0, 0)
time.sleep(0.2)

# 1) SHIFT+click (0,3) → n=4
shift = 0x4
user32.PostMessageW(ct.c_void_p(target), 0x201, shift, pack(MARGIN + PAD + 3 * CW + CW // 2,
                                                            MARGIN + GT + 0 * CH + CH // 2))
time.sleep(0.06)
user32.PostMessageW(ct.c_void_p(target), 0x202, shift, pack(MARGIN + PAD + 3 * CW + CW // 2,
                                                            MARGIN + GT + 0 * CH + CH // 2))
time.sleep(0.3)
n1 = log_count()
ok1 = n1 == 4
res.append(ok1)
print('1 shift range n=4:', 'PASS' if ok1 else 'FAIL (n=%d)' % n1)

# 2) CTRL+click (1,0) → n=5;再CTRL → n=4
ctrl = 0x8
click_cell(1, 0, flags=ctrl); time.sleep(0.3)
n2 = log_count()
click_cell(1, 0, flags=ctrl); time.sleep(0.3)
n3 = log_count()
ok2 = n2 == 5 and n3 == 4
res.append(ok2)
print('2 ctrl add/remove 5->4:', 'PASS' if ok2 else 'FAIL (%d,%d)' % (n2, n3))

# 3) 框选 row2-3 col0-2 → marquee sel n>=3
x0 = MARGIN + PAD - 6
y0 = MARGIN + GT + 2 * CH - 8
x1 = MARGIN + PAD + 3 * CW - 10
y1 = MARGIN + GT + 3 * CH + CH - 10
user32.PostMessageW(ct.c_void_p(target), 0x201, 0, pack(x0, y0))
time.sleep(0.08)
for t in (0.3, 0.6, 0.9):
    user32.PostMessageW(ct.c_void_p(target), 0x200, 0, pack(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t))
    time.sleep(0.1)
user32.PostMessageW(ct.c_void_p(target), 0x202, 0, 0)
time.sleep(0.35)
nm = log_marquee()
ok3 = nm >= 3
res.append(ok3)
print('3 marquee sel n>=3:', 'PASS' if ok3 else 'FAIL (n=%d)' % nm)

# 4) 空白单击清除 → 之后 CTRL+click 任何格应 n=1(说明此前已清空)
click_cell(6, 6); time.sleep(0.3)
click_cell(2, 2, flags=ctrl); time.sleep(0.3)
n4 = log_count()
ok4 = n4 == 1
res.append(ok4)
print('4 clear then ctrl n=1:', 'PASS' if ok4 else 'FAIL (n=%d)' % n4)

# 清残留:普通单击
click_cell(0, 6); time.sleep(0.2)
print('RESULT:', 'PASS' if all(res) else 'FAIL', res)
print('RESULT:', 'PASS' if all(res) else 'FAIL', res)
