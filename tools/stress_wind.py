# -*- coding: utf-8 -*-
"""Win+D stress: 8 cycles, probe desktop/restore states each half-cycle.
Checks per probe:
  V  every real panel visible and not iconic
  C  every panel has painted content (PrintWindow bright pixels)
  S  every panel above desktop shell (idx < first Progman/WorkerW idx)
  N  no visible normal window sits BELOW+OVERLAPPING a panel (panel wrongly on top)
"""
import ctypes as ct
import ctypes.wintypes as w
import time
import sys

u32 = ct.c_uint32
user32 = ct.windll.user32
gdi32 = ct.windll.gdi32
user32.SetProcessDpiAwarenessContext(ct.c_void_p(-4))

SHELLISH = {'Progman', 'WorkerW', 'Shell_TrayWnd', 'Shell_SecondaryTrayWnd'}
# 壳的透明输入/影子窗:z 在面板之下但不可视,不构成遮挡(N 检查忽略)
NOCOVER = {'EdgeUiInputTopWndClass', 'Windows.UI.Core.CoreWindow', 'DuiShadowWnd',
           'DummyDWMListenerWindow', 'Tao Thread Event Target', 'ThumbnailDeviceHelperWnd'}

def keybd(vk, up=False):
    user32.keybd_event(ct.c_ubyte(vk), 0, 0x0002 if up else 0, 0)  # KEYEVENTF_KEYUP=2

_emul_minimized = []   # hwnds we minimized for emulation
_emul_state = {'desktop': False}

def wind_toggle():
    """Emulate Show Desktop / restore without shell or input (lock screen safe):
    desktop phase = minimize all visible normal windows + raise Progman above
    panels (creates the exact burial pressure Win+D produces); restore phase =
    SW_RESTORE them back (fires MINIMIZESTART/FOREGROUND/REORDER for the app)."""
    global _emul_minimized
    if not _emul_state['desktop']:
        targets = []
        @ct.WINFUNCTYPE(ct.c_bool, w.HWND, w.LPARAM)
        def cb(h, lp):
            hv = h if isinstance(h, int) else h.value
            clsb = ct.create_unicode_buffer(128); user32.GetClassNameW(h, clsb, 128)
            c = clsb.value
            if not c or c in SHELLISH or c in NOCOVER or c in ('RdGroupPanel',) or c.startswith('#'):
                return True
            if user32.IsWindowVisible(hv) and not user32.IsIconic(hv):
                targets.append(hv)
            return True
        user32.EnumWindows(cb, 0)
        for hv in targets:
            user32.ShowWindow(ct.c_void_p(hv), 6)  # SW_MINIMIZE
        time.sleep(0.15)
        # raise Progman to top of normal band: wallpapers above panels => burial test
        prog = None
        @ct.WINFUNCTYPE(ct.c_bool, w.HWND, w.LPARAM)
        def cb2(h, lp):
            nonlocal prog
            clsb = ct.create_unicode_buffer(128); user32.GetClassNameW(h, clsb, 128)
            if clsb.value == 'Progman' and user32.IsWindowVisible(h):
                prog = h if isinstance(h, int) else h.value
                return False
            return True
        user32.EnumWindows(cb2, 0)
        if prog:
            user32.SetWindowPos(ct.c_void_p(prog), ct.c_void_p(0), 0, 0, 0, 0, 0x1 | 0x2 | 0x10)
        _emul_minimized = targets
        _emul_state['desktop'] = True
        return 'emul-desktop(%d windows)' % len(targets)
    else:
        for hv in _emul_minimized:
            user32.ShowWindow(ct.c_void_p(hv), 9)  # SW_RESTORE
        _emul_minimized = []
        # give one restored window foreground to fire EVENT_SYSTEM_FOREGROUND
        @ct.WINFUNCTYPE(ct.c_bool, w.HWND, w.LPARAM)
        def cb3(h, lp):
            hv = h if isinstance(h, int) else h.value
            clsb = ct.create_unicode_buffer(128); user32.GetClassNameW(h, clsb, 128)
            if clsb.value == 'Chrome_WidgetWin_1' and user32.IsWindowVisible(hv) and not user32.IsIconic(hv):
                user32.SetForegroundWindow(ct.c_void_p(hv))
                return False
            return True
        user32.EnumWindows(cb3, 0)
        _emul_state['desktop'] = False
        return 'emul-restore'

def enum_wins():
    out = []
    @ct.WINFUNCTYPE(ct.c_bool, w.HWND, w.LPARAM)
    def cb(h, lp):
        clsb = ct.create_unicode_buffer(128)
        user32.GetClassNameW(h, clsb, 128)
        ttlb = ct.create_unicode_buffer(256)
        user32.GetWindowTextW(h, ttlb, 256)
        r = w.RECT()
        user32.GetWindowRect(h, ct.byref(r))
        vis = bool(user32.IsWindowVisible(h))
        ico = bool(user32.IsIconic(h))
        hv = h if isinstance(h, int) else h.value
        out.append((hv, clsb.value, ttlb.value, (r.left, r.top, r.right, r.bottom), vis, ico))
        return True
    user32.EnumWindows(cb, 0)
    return out

def bright_pixels(hwnd, wdt, hgt):
    from PIL import Image
    hdc = user32.GetDC(0)
    mem = gdi32.CreateCompatibleDC(hdc)
    class BMIH(ct.Structure):
        _fields_ = [('biSize', u32), ('biWidth', ct.c_int32), ('biHeight', ct.c_int32),
                    ('biPlanes', ct.c_uint16), ('biBitCount', ct.c_uint16),
                    ('biCompression', u32), ('biSizeImage', u32), ('biXPelsPerMeter', ct.c_int32),
                    ('biYPelsPerMeter', ct.c_int32), ('biClrUsed', u32), ('biClrImportant', u32)]
    class BMI(ct.Structure):
        _fields_ = [('h', BMIH), ('c', u32 * 3)]
    bmi = BMI()
    bmi.h.biSize = ct.sizeof(BMIH); bmi.h.biWidth = wdt; bmi.h.biHeight = -hgt
    bmi.h.biPlanes = 1; bmi.h.biBitCount = 32
    bits = ct.c_void_p()
    hbm = gdi32.CreateDIBSection(mem, ct.byref(bmi), 0, ct.byref(bits), None, 0)
    old = gdi32.SelectObject(mem, hbm)
    ok = user32.PrintWindow(hwnd, mem, 2)
    n = 0
    if ok and bits.value:
        data = ct.string_at(bits.value, wdt * hgt * 4)
        img = Image.frombuffer('RGBA', (wdt, hgt), data, 'raw', 'BGRA', 0, 1)
        hist = img.convert('L').histogram()
        n = sum(hist[24:])  # L>24 = panel content (bg ~42), blank/transparent = 0
    gdi32.SelectObject(mem, old); gdi32.DeleteObject(hbm); gdi32.DeleteDC(mem)
    user32.ReleaseDC(0, hdc)
    return n

def overlap(a, b):
    x = min(a[2], b[2]) - max(a[0], b[0])
    y = min(a[3], b[3]) - max(a[1], b[1])
    return x * y if x > 0 and y > 0 else 0

def probe(label, expect):
    wins = enum_wins()
    idx = {h: i for i, (h, _, _, _, _, _) in enumerate(wins)}
    def is_ghost(h):
        return bool(user32.GetWindowLongPtrW(h, -20) & 0x20)  # WS_EX_TRANSPARENT
    panels = [(h, cls, ttl, rc) for (h, cls, ttl, rc, vis, ico) in wins
              if cls == 'RdGroupPanel' and not is_ghost(h)]
    fails = []
    # state detect: Chrome windows (ZCode/browsers) minimized => desktop
    chrome_vis = [1 for (h, cls, ttl, rc, vis, ico) in wins
                  if cls == 'Chrome_WidgetWin_1' and vis and not ico]
    state = 'restore' if chrome_vis else 'desktop'

    # V + C
    for (h, cls, ttl, rc, vis, ico) in wins:
        if cls == 'RdGroupPanel' and not is_ghost(h):
            if not vis or ico:
                fails.append('V:panel not visible/iconic h=%d' % h)
    # S: first VISIBLE shell idx (invisible WorkerW keep stale z slots - ignore)
    shell_first = None
    for i, (h, cls, ttl, rc, vis, ico) in enumerate(wins):
        if cls in ('Progman', 'WorkerW') and vis and not ico:
            shell_first = i
            break
    if shell_first is None:
        fails.append('S:no visible shell window found')
    else:
        for (h, cls, ttl, rc) in panels:
            if idx[h] >= shell_first:
                fails.append('S:panel h=%d idx=%d >= shell=%d (buried)' % (h, idx[h], shell_first))
    # C content
    for (h, cls, ttl, rc) in panels:
        bp = bright_pixels(h, rc[2]-rc[0], rc[3]-rc[1])
        if bp < 1500:
            fails.append('C:panel h=%d blank (%d bright px)' % (h, bp))
    # N: visible normal window below+overlapping panel
    for (ph, cls, ttl, prc) in panels:
        for (h, cls2, ttl2, rc, vis, ico) in wins:
            if not vis or ico:
                continue
            if cls2 in SHELLISH or cls2 == 'RdGroupPanel' or cls2.startswith('#'):
                continue
            if cls2 in NOCOVER:
                continue
            if is_ghost(h):
                continue
            if overlap(prc, rc) > 60 and idx[h] > idx[ph]:
                fails.append('N:panel h=%d on top of %s h=%d' % (ph, cls2, h))
    ok = not fails
    tag = 'PASS' if ok else 'FAIL'
    extra = '' if state == expect else ' (state=%s expected=%s)' % (state, expect)
    print('%-8s %-8s panels=%d %s%s' % (label, tag, len(panels),
          ';'.join(fails[:4]) if fails else 'all-ok', extra))
    return ok

total_pass = 0
total = 0
waits = [0.40, 0.55, 0.35, 0.65, 0.45, 0.50, 0.38, 0.60]
for i in range(8):
    w1 = waits[i]
    wind_toggle()           # -> desktop
    time.sleep(w1)
    total += 1; total_pass += probe('c%dd' % i, 'desktop')
    time.sleep(0.7)
    total += 1; total_pass += probe('c%dd2' % i, 'desktop')
    wind_toggle()           # -> restore
    time.sleep(w1 + 0.1)
    total += 1; total_pass += probe('c%dr' % i, 'restore')
    time.sleep(0.7)
    total += 1; total_pass += probe('c%dr2' % i, 'restore')
print('RESULT: %d/%d probes passed' % (total_pass, total))
sys.exit(0 if total_pass == total else 1)
