# -*- coding: utf-8 -*-
"""real SendInput mouse test: shift/ctrl/marquee on files panel (Win+D exposed first)"""
import ctypes as ct
import ctypes.wintypes as w
import time
import re
import os

user32 = ct.windll.user32
user32.SetProcessDpiAwarenessContext(ct.c_void_p(-4))
MARGIN, PAD, GT, CW, CH = 26, 16, 46, 132, 86
LOG = os.path.join(os.environ['TEMP'], 'rdesktop.log')
LOG0 = 0

class MOUSEINPUT(ct.Structure):
    _fields_ = [('dx', ct.c_long), ('dy', ct.c_long), ('mouseData', ct.c_uint32),
                ('dwFlags', ct.c_uint32), ('time', ct.c_uint32),
                ('dwExtraInfo', ct.c_void_p)]
class KEYBDINPUT(ct.Structure):
    _fields_ = [('wVk', ct.c_uint16), ('wScan', ct.c_uint16), ('dwFlags', ct.c_uint32),
                ('time', ct.c_uint32), ('dwExtraInfo', ct.c_void_p)]
class INPUTUNION(ct.Union):
    _fields_ = [('mi', MOUSEINPUT), ('ki', KEYBDINPUT)]
class INPUT(ct.Structure):
    class _U(ct.Union):
        _fields_ = [('mi', MOUSEINPUT), ('ki', KEYBDINPUT)]
    _anonymous_ = ('u',)
    _fields_ = [('type', ct.c_uint32), ('u', _U)]

MOUSEEVENTF_MOVE = 1
MOUSEEVENTF_ABSOLUTE = 0x8000
MOUSEEVENTF_LEFTDOWN = 2
MOUSEEVENTF_LEFTUP = 4
KEYEVENTF_KEYUP = 2
VK_SHIFT, VK_CONTROL = 0x10, 0x11
SM_CXSCREEN, SM_CYSCREEN = 0, 1

def screen_w():
    return user32.GetSystemMetrics(SM_CXSCREEN)

def send_mouse(dx, dy, flags):
    inp = INPUT()
    inp.type = 0
    inp.mi = MOUSEINPUT(dx, dy, 0, flags, 0, None)
    user32.SendInput(1, ct.byref(inp), ct.sizeof(INPUT))

def abs_move(x, y):
    sw = user32.GetSystemMetrics(SM_CXSCREEN)
    sh = user32.GetSystemMetrics(SM_CYSCREEN)
    ax = int(x * 65535 / max(sw - 1, 1))
    ay = int(y * 65535 / max(sh - 1, 1))
    send_mouse(ax, ay, MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE)

def click_at(x, y, ctrl=False, shift=False):
    abs_move(x, y); time.sleep(0.08)
    if ctrl: user32.keybd_event(VK_CONTROL, 0, 0, 0)
    if shift: user32.keybd_event(VK_SHIFT, 0, 0, 0)
    time.sleep(0.05)
    send_mouse(0, 0, MOUSEEVENTF_LEFTDOWN)
    time.sleep(0.06)
    send_mouse(0, 0, MOUSEEVENTF_LEFTUP)
    time.sleep(0.05)
    if shift: user32.keybd_event(VK_SHIFT, 0, KEYEVENTF_KEYUP, 0)
    if ctrl: user32.keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0)
    time.sleep(0.25)

def drag(x0, y0, x1, y1, steps=6):
    abs_move(x0, y0); time.sleep(0.08)
    send_mouse(0, 0, MOUSEEVENTF_LEFTDOWN)
    time.sleep(0.08)
    for i in range(1, steps + 1):
        t = i / steps
        abs_move(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
        time.sleep(0.05)
    send_mouse(0, 0, MOUSEEVENTF_LEFTUP)
    time.sleep(0.3)

def real_wind():
    user32.keybd_event(0x5B, 0, 0, 0); time.sleep(0.02)
    user32.keybd_event(0x44, 0, 0, 0); time.sleep(0.02)
    user32.keybd_event(0x44, 0, 2, 0); time.sleep(0.02)
    user32.keybd_event(0x5B, 0, 2, 0)

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

def log_ns():
    txt = open(LOG, encoding='utf-8', errors='replace').read()
    return [int(x) for x in re.findall(r'multi n=(\d+)', txt)], \
           [int(x) for x in re.findall(r'marquee sel n=(\d+)', txt)]

def win_at(x, y):
    return user32.WindowFromPoint(ct.c_void_p(0)) if False else None

def pt_on_panel(p):
    """找一个 WindowFromPoint 命中该面板的单元格点"""
    L, T, W, H = rect(p)
    for (row, col) in [(0, 0), (0, 2), (1, 1), (2, 0), (3, 2)]:
        x = MARGIN + PAD + col * CW + CW // 2
        y = MARGIN + GT + row * CH + 30
        if x >= L + W - 20 or y >= T + H - 10:
            continue
        hp = w.POINT(x, y)
        hw = user32.WindowFromPoint(ct.byref(hp))
        hv = hw if isinstance(hw, int) else hw.value
        if hv == p:
            return (x, y)
    return None

# 1) Win+D 暴露面板
real_wind0 = lambda: None
user32.keybd_event(0x5B, 0, 0, 0); time.sleep(0.02)
user32.keybd_event(0x44, 0, 0, 0); time.sleep(0.02)
user32.keybd_event(0x44, 0, 2, 0); time.sleep(0.02)
user32.keybd_event(0x5B, 0, 2, 0)
time.sleep(1.2)

ps = [p for p in find('RdGroupPanel')
      if not (user32.GetWindowLongPtrW(ct.c_void_p(p), -20) & 0x20) and user32.IsWindowVisible(ct.c_void_p(p))]
target = max(ps, key=lambda p: rect(p)[2])
print('target 0x%x' % target)

pt0 = pt_on_panel(target)
print('clean point for item(0,0):', pt0)
assert pt0, 'no clickable point on target panel'

LOG0 = os.path.getsize(LOG) if os.path.exists(LOG) else 0
res = []

def new_multi():
    txt = open(LOG, encoding='utf-8', errors='replace').read()[LOG0:]
    return [int(x) for x in re.findall(r'multi n=(\d+)', txt)], \
           [int(x) for x in re.findall(r'marquee sel n=(\d+)', txt)]

# 1) shift: 点击(0,0) 再 shift 点击(0,3)
click_at(*pt0)
pt3 = (pt0[0] + 3 * CW, pt0[1])
click_at(*pt3, shift=True)
time.sleep(0.3)
m, _ = new_multi()
ok1 = m and m[-1] == 4
res.append(ok1)
print('1 shift range n=4:', 'PASS' if ok1 else 'FAIL %s' % (m[-3:]))

# 2) ctrl 点击 (1,0) → 5;再 ctrl → 4
pt4 = (pt0[0], pt0[1] + CH)
click_at(*pt4, ctrl=True); time.sleep(0.3)
m1, _ = new_multi()
click_at(*pt4, ctrl=True); time.sleep(0.3)
m2, _ = new_multi()
ok2 = m1 and m1[-1] == 5 and m2 and m2[-1] == 4
res.append(ok2)
print('2 ctrl 5->4:', 'PASS' if ok2 else 'FAIL %s %s' % (m1[-2:], m2[-2:]))

# 3) 框选:从 row2 左拖到 row4 右
mqx0 = MARGIN + PAD - 8
mqy0 = MARGIN + GT + 2 * CH - 6
mqx1 = MARGIN + PAD + 3 * CW - 12
mqy1 = MARGIN + GT + 4 * CH + 40
drag(mqx0, mqy0, mqx1, mqy1)
time.sleep(0.4)
_, mm = new_multi()
ok3 = mm and mm[-1] >= 3
res.append(ok3)
print('3 marquee sel n>=3:', 'PASS' if ok3 else 'FAIL (n=%s)' % (mm[-1:]))

# 4) 清理:空白处单击取消(框后点 row6 空区),再验证
click_at(MARGIN + PAD + 6 * CW + 40, MARGIN + GT + 6 * CH + 40)
time.sleep(0.3)
click_at(*pt0, ctrl=True); time.sleep(0.3)
m3, _ = new_multi()
ok4 = m3 and m3[-1] == 1
res.append(ok4)
print('4 clear then ctrl n=1:', 'PASS' if ok4 else 'FAIL (n=%s)' % (m3[-1:]))

# 恢复:Win+D 回原状
user32.keybd_event(0x5B, 0, 0, 0); time.sleep(0.02)
user32.keybd_event(0x44, 0, 0, 0); time.sleep(0.02)
user32.keybd_event(0x44, 0, 2, 0); time.sleep(0.02)
user32.keybd_event(0x5B, 0, 2, 0)
time.sleep(1.0)

print('RESULT:', 'PASS' if all(res) else 'FAIL', res)
