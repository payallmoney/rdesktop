# -*- coding: utf-8 -*-
"""Rapid Win+D stress: fast toggles (250ms), probe final state after settle.
Fallback to shell emulation if injected Win+D doesn't take."""
import ctypes as ct
import ctypes.wintypes as w
import time

user32 = ct.windll.user32
user32.SetProcessDpiAwarenessContext(ct.c_void_p(-4))
SHELLISH = {'Progman', 'WorkerW', 'Shell_TrayWnd', 'Shell_SecondaryTrayWnd'}
NOCOVER = {'EdgeUiInputTopWndClass', 'Windows.UI.Core.CoreWindow', 'DuiShadowWnd',
           'DummyDWMListenerWindow', 'Tao Thread Event Target', 'ThumbnailDeviceHelperWnd'}
_persist_min = []
_state = {'desktop': False}

def keybd(vk, up=False):
    user32.keybd_event(ct.c_ubyte(vk), 0, 0x0002 if up else 0, 0)

def real_wind():
    keybd(0x5B); time.sleep(0.02)
    keybd(0x44); time.sleep(0.02)
    keybd(0x44, True); time.sleep(0.02)
    keybd(0x5B, True)

def enum_all():
    out = []
    @ct.WINFUNCTYPE(ct.c_bool, w.HWND, w.LPARAM)
    def cb(h, lp):
        hv = h if isinstance(h, int) else h.value
        clsb = ct.create_unicode_buffer(64); user32.GetClassNameW(h, clsb, 64)
        vis = bool(user32.IsWindowVisible(hv)); ico = bool(user32.IsIconic(hv))
        r = w.RECT(); user32.GetWindowRect(ct.c_void_p(hv), ct.byref(r))
        out.append((hv, clsb.value, vis, ico, (r.left, r.top, r.right, r.bottom)))
        return True
    user32.EnumWindows(cb, 0)
    return out

def toggle(mode):
    """mode='keybd' real Win+D; mode='emu' shell emulation"""
    global _persist_min
    if mode == 'keybd':
        real_wind()
        return
    if not _state['desktop']:
        targets = []
        for (hv, c, vis, ico, rc) in enum_all():
            if c and c not in SHELLISH and c not in NOCOVER and c != 'RdGroupPanel' \
               and not c.startswith('#') and vis and not ico:
                targets.append(hv)
        for hv in targets:
            user32.ShowWindow(ct.c_void_p(hv), 6)
        time.sleep(0.08)
        prog = None
        for (hv, c, vis, ico, rc) in enum_all():
            if c == 'Progman' and vis:
                prog = hv; break
        if prog:
            user32.SetWindowPos(ct.c_void_p(prog), ct.c_void_p(0), 0, 0, 0, 0, 0x1 | 0x2 | 0x10)
        _persist_min = targets
        _state['desktop'] = True
    else:
        for hv in _persist_min:
            user32.ShowWindow(ct.c_void_p(hv), 9)
        _persist_min = []
        _state['desktop'] = False

def panels():
    out = []
    for (hv, c, vis, ico, rc) in enum_all():
        if c == 'RdGroupPanel' and not (user32.GetWindowLongPtrW(ct.c_void_p(hv), -20) & 0x20):
            out.append((hv, vis, ico, rc))
    return out

def probe(tag, expect_desktop):
    fails = []
    ps = panels()
    for (hv, vis, ico, rc) in ps:
        if not vis:
            fails.append('V:hidden h=%d' % hv)
        if ico:
            fails.append('V:iconic h=%d' % hv)
    # first visible shell
    shell_idx = None
    wins = enum_all()
    shell_hv = None
    for i, (hv, c, vis, ico, rc) in enumerate(wins):
        if c in ('Progman', 'WorkerW') and vis and not ico:
            shell_idx = i; shell_hv = hv; break
    if shell_idx is None:
        fails.append('S:no visible shell')
    elif shell_hv is not None:
        ex = user32.GetWindowLongPtrW(ct.c_void_p(shell_hv), -20)
        if ex & 8:
            fails.append('S:shell TOPMOST h=%d idx=%d' % (shell_hv, shell_idx))
    else:
        for (hv, vis, ico, rc) in ps:
            i = next((k for k, x in enumerate(wins) if x[0] == hv), -1)
            if i >= shell_idx:
                fails.append('S:panel h=%d idx=%d>=%d' % (hv, i, shell_idx))
    # cover checks
    def overlap(a, b):
        x = min(a[2], b[2]) - max(a[0], b[0])
        y = min(a[3], b[3]) - max(a[1], b[1])
        return x * y if x > 0 and y > 0 else 0
    for (ph, pv, pi, prc) in ps:
        for (hv, c, vis, ico, rc) in wins:
            if hv == ph or not vis or ico:
                continue
            if not c or c in SHELLISH or c in NOCOVER or c == 'RdGroupPanel' or c.startswith('#'):
                continue
            i_w = next((k for k, x in enumerate(wins) if x[0] == hv), -1)
            i_p = next((k for k, x in enumerate(wins) if x[0] == ph), -1)
            if overlap(prc, rc) > 60 and i_w > i_p:
                fails.append('N:panel h=%d under %s' % (ph, c))
            if not expect_desktop and overlap(prc, rc) > 60 and i_w > i_p:
                fails.append('N2:panel h=%d under %s' % (ph, c))
    ok = not fails
    print('%-16s %s %s' % (tag, 'PASS' if ok else 'FAIL', ';'.join(fails[:3])))
    return ok

def chrome_visible():
    for (hv, c, vis, ico, rc) in enum_all():
        if c == 'Chrome_WidgetWin_1' and vis and not ico:
            return True
    return False

def chrome_iconic():
    n = 0
    for (hv, c, vis, ico, rc) in enum_all():
        if c == 'Chrome_WidgetWin_1' and ico:
            n += 1
    return n

# 探测:注入 Win+D 是否生效(Chrome 最小化数应增加)
ico0 = chrome_iconic()
real_wind()
time.sleep(1.0)
ico1 = chrome_iconic()
if ico1 > ico0:
    mode = 'keybd'
    print('real Win+D works (%d -> %d iconic)' % (ico0, ico1))
    real_wind(); time.sleep(1.0)  # 恢复回原状
else:
    mode = 'emu'
    print('keybd Win+D inert -> use emulation')
    real_wind(); time.sleep(0.6)  # 可能没生效,保险起见同步一下状态
    if chrome_iconic() > ico0:
        _state['desktop'] = True  # 其实生效了一半?按最小化状态收敛
        for hv in []:
            pass

N = 10
INTERVAL = 0.25
for i in range(N):
    toggle(mode)
    time.sleep(INTERVAL)
# final toggle: if N odd -> desktop, even -> restore
expect_desktop = (N % 2 == 1)
if mode == 'keybd':
    # parity by keybd: N keybd presses total; started from restore -> odd = desktop
    pass
time.sleep(1.2)
p1 = probe('mid-settle', expect_desktop)
time.sleep(1.8)
p2 = probe('after-2s', expect_desktop)
# toggle back to opposite state and probe
toggle(mode)
time.sleep(1.5)
p3 = probe('next-state', not expect_desktop)
toggle(mode)
time.sleep(1.5)
p4 = probe('final', expect_desktop)
print('RESULT:', 'PASS' if all([p1, p2, p3, p4]) else 'FAIL',
      [p1, p2, p3, p4], 'mode=', mode)
