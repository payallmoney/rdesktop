# -*- coding: utf-8 -*-
"""scrollbar e2e: find panel with bar, wheel scroll roundtrip, thumb drag, track jump"""
import ctypes as ct
import ctypes.wintypes as w
import time
import io

u32 = ct.c_uint32
user32 = ct.windll.user32
gdi32 = ct.windll.gdi32
user32.SetProcessDpiAwarenessContext(ct.c_void_p(-4))
WM_MOUSEWHEEL = 0x020A
MARGIN, PAD, GT = 26, 16, 46
CH = 86

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
    return ((y & 0xFFFF) << 16) | (x & 0xFFFF)

def grab(h, box):
    """PrintWindow -> PIL image of window"""
    L, T, W, H = rect(h)
    from PIL import Image
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
    if box:
        img = img.crop(box)
    return img

def grid_hash(h):
    L, T, W, H = rect(h)
    # 内容网格区(不含右侧滚动条):客户 MARGIN..panel_w-14
    pw = W - 2 * MARGIN
    im = grab(h, (MARGIN + 4, MARGIN + GT + 4, MARGIN + pw - 16, MARGIN + GT + 100))
    return hash(im.tobytes())

def track_strip(h):
    """返回滚动条轨道像素段(客户坐标换算到窗口位图)"""
    L, T, W, H = rect(h)
    pw = W - 2 * MARGIN
    tx = MARGIN + pw - 18
    ty = MARGIN + GT
    th = ((H - 2 * MARGIN - GT - 14) // CH) * CH  # rows*ch 近似(够用)
    return (tx, ty, th)

def bright_ratio(im):
    px = list(im.getdata())
    n = max(len(px), 1)
    return sum(1 for r, g, b in px if min(r, g, b) > 60) / n

panels = [p for p in find('RdGroupPanel')
          if not (user32.GetWindowLongPtrW(ct.c_void_p(p), -20) & 0x20)]
print('panels:', len(panels))

# 预处理:文件面板(最宽)缩矮280px,制造内容溢出(用户尺寸恰好都装得下)
def widest():
    return max(panels, key=lambda p: rect(p)[2])

force = widest()
FW, FH = rect(force)[2], rect(force)[3]
# 按住底边向上拖280(两步)
user32.PostMessageW(ct.c_void_p(force), 0x201, 1, pack(FW // 2, FH - 4))
time.sleep(0.1)
for step in (280, 560):
    user32.PostMessageW(ct.c_void_p(force), 0x200, 0, pack(FW // 2, FH - 4 - step))
    time.sleep(0.15)
user32.PostMessageW(ct.c_void_p(force), 0x202, 0, 0)
time.sleep(0.5)
FH2 = rect(force)[3]
print('shrunk: %d -> %d (delta %d)' % (FH, FH2, FH2 - FH))

# 扫描:找轨道上出现滑块(亮像素)的面板
target = None
for p in panels:
    tx, ty, th = track_strip(p)
    im = grab(p, (tx, ty, tx + 6, ty + th))
    br = bright_ratio(im)
    print('panel 0x%x track bright=%.3f' % (p, br))
    if br > 0.02:
        target = p
if target is None:
    print('NO panel has scrollbar -> FAIL')
    raise SystemExit(1)
print('target with scrollbar: 0x%x' % target)

res = []
H0 = grid_hash(target)
print('H0 =', H0)

def wheel(n, up=False):
    L, T, W, H = rect(target)
    sp = (L + W // 2, T + H // 2)
    delta = -120 if up else 120
    for _ in range(abs(n)):
        user32.PostMessageW(ct.c_void_p(target), WM_MOUSEWHEEL,
                            ((delta & 0xFFFF) << 16), pack(*sp))
        time.sleep(0.12)

# 滚轮下1行
wheel(1)
time.sleep(0.3)
H1 = grid_hash(target)
ok_w1 = H1 != H0
print('wheel down changes content:', 'PASS' if ok_w1 else 'FAIL')
res.append(ok_w1)

# 再下1
wheel(1)
time.sleep(0.3)
H2 = grid_hash(target)
res.append(H2 != H1)
print('wheel down2 changes:', 'PASS' if H2 != H1 else 'FAIL')

# 滚回顶部:必须精确复原
wheel(2, up=True)
time.sleep(0.3)
H3 = grid_hash(target)
ok_back = H3 == H0
print('wheel back to top exact:', 'PASS' if ok_back else 'FAIL (hash differs)')
res.append(ok_back)

# 轨道点击跳转
tx, ty, th = track_strip(target)
click_y = ty + th - 6   # 轨道底部 -> 跳到底
user32.PostMessageW(ct.c_void_p(target), 0x201, 1, pack(tx + 3, click_y))
time.sleep(0.25)
user32.PostMessageW(ct.c_void_p(target), 0x202, 0, 0)
time.sleep(0.3)
H4 = grid_hash(target)
ok_jump = H4 != H0
print('track click jumps:', 'PASS' if ok_jump else 'FAIL')
res.append(ok_jump)

# 滑块拖动:轨道点击已把内容跳到底(滑块此时贴底部),按住滑块往上拖
tx, ty, th = track_strip(target)
user32.PostMessageW(ct.c_void_p(target), 0x201, 1, pack(tx + 3, ty + th - 14))
time.sleep(0.15)
user32.PostMessageW(ct.c_void_p(target), 0x200, 0, pack(tx + 3, ty + th - 14 - (th // 2)))
time.sleep(0.15)
user32.PostMessageW(ct.c_void_p(target), 0x200, 0, pack(tx + 3, ty + 6))
time.sleep(0.15)
user32.PostMessageW(ct.c_void_p(target), 0x202, 0, 0)
time.sleep(0.3)
H5 = grid_hash(target)
ok_drag = H5 != H4
print('thumb drag changes:', 'PASS' if ok_drag else 'FAIL')
res.append(ok_drag)

# 清理:滚回顶
for _ in range(30):
    L, T, W, H = rect(target)
    wheel(5, up=True)
    hh = grid_hash(target)
    if hh == H0:
        break
time.sleep(0.4)
H6 = grid_hash(target)
ok_top = H6 == H0
print('cleanup back to top:', 'PASS' if ok_top else 'FAIL')
res.append(ok_top)

# 无滚动条面板:轨道区应无亮滑块
short = [p for p in panels if p != target]
no_bar_ok = True
for p in short[:2]:
    tx2, ty2, th2 = track_strip(p)
    br = bright_ratio(grab(p, (tx2, ty2, tx2 + 6, ty2 + max(th2, 10))))
    if br > 0.05:
        no_bar_ok = False
        print('panel 0x%x unexpectedly has bright track %.3f' % (p, br))
res.append(no_bar_ok)
print('short panels no thumb:', 'PASS' if no_bar_ok else 'FAIL')

# 复原:再拖回+280(基点=按下时的位置,固定不变)
rF = rect(force)
bx0, by0 = rF[0] + FW // 2, rF[1] + rF[3] - 4   # 按下时屏幕点
user32.PostMessageW(ct.c_void_p(force), 0x201, 1, pack(FW // 2, rF[3] - 4))
time.sleep(0.1)
for step in (280, 560):
    user32.PostMessageW(ct.c_void_p(force), 0x200, 0, pack(FW // 2 + 0, rF[3] - 4 + step))
    # 屏幕目标 = 基点+step:换算客户端(窗口上移使 y 变化)——用屏幕坐标协议
    time.sleep(0.15)
user32.PostMessageW(ct.c_void_p(force), 0x202, 0, 0)
time.sleep(0.5)
FH3 = rect(force)[3]
restored = abs(FH3 - FH) <= 4
print('restored height:', FH3, 'orig', FH, '=>', 'PASS' if restored else 'FAIL')
res.append(restored)

print('RESULT:', 'PASS' if all(res) else 'FAIL', res)
