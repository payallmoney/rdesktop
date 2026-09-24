# -*- coding: utf-8 -*-
"""menu features e2e v2: robust esc, rect-matched gi, hide polling"""
import ctypes as ct
import ctypes.wintypes as w
import time
import winreg

user32 = ct.windll.user32
user32.SetProcessDpiAwarenessContext(ct.c_void_p(-4))
WM_RBUTTONDOWN, WM_COMMAND, WM_TRAY, WM_RBUTTONUP = 0x204, 0x111, 0x8001, 0x0205
MN_GETHMENU = 0x01E1
MF_BYPOSITION = 0x400
MARGIN = 26
K = r'Software\rdesktop'

def pack(x, y):
    return ((y & 0xFFFF) << 16) | (x & 0xFFFF)

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

def menus_open():
    return [m for m in find('#32768') if user32.IsWindowVisible(ct.c_void_p(m))]

def rect(h):
    r = w.RECT(); user32.GetWindowRect(ct.c_void_p(h), ct.byref(r))
    return (r.left, r.top, r.right - r.left, r.bottom - r.top)

def val(name):
    try:
        k = winreg.OpenKey(winreg.HKEY_CURRENT_USER, K)
        return winreg.QueryValueEx(k, name)[0]
    except FileNotFoundError:
        return None

def real_panels():
    return [p for p in find('RdGroupPanel')
            if not (user32.GetWindowLongPtrW(ct.c_void_p(p), -20) & 0x20)]

def gi_of(hwnd):
    r = rect(hwnd)
    for i in range(8):
        if val('p%d.x' % i) == r[0] + MARGIN and val('p%d.y' % i) == r[1] + MARGIN:
            return i
    return None

def esc_all():
    for _ in range(15):
        ms = menus_open()
        if not ms:
            return True
        for m in ms:
            user32.SendMessageW(ct.c_void_p(m), 0x100, 0x1B, 0)
            user32.SendMessageW(ct.c_void_p(m), 0x101, 0x1B, 0)
        time.sleep(0.12)
    return not menus_open()

def open_menu(post_fn):
    esc_all()
    post_fn()
    for _ in range(25):
        time.sleep(0.1)
        ms = menus_open()
        if ms:
            return ms[0]
    return None

def menu_items(mh):
    hm = user32.SendMessageW(ct.c_void_p(mh), MN_GETHMENU, 0, 0)
    n = user32.GetMenuItemCount(ct.c_void_p(hm))
    out = []
    for i in range(n):
        b = ct.create_unicode_buffer(64)
        user32.GetMenuStringW(ct.c_void_p(hm), i, b, 64, MF_BYPOSITION)
        st = user32.GetMenuState(ct.c_void_p(hm), i, MF_BYPOSITION)
        out.append((b.value, bool(st & 0x8)))
    return out

def panel_rb(p):
    user32.PostMessageW(ct.c_void_p(p), WM_RBUTTONDOWN, 1, pack(MARGIN + 80, MARGIN + 15))

def tray_rb(p):
    user32.PostMessageW(ct.c_void_p(p), WM_TRAY, 0, WM_RBUTTONUP)

def wait_vis(hwnd, want, t=1.5):
    for _ in range(int(t * 10)):
        if bool(user32.IsWindowVisible(ct.c_void_p(hwnd))) == want:
            return True
        time.sleep(0.1)
    return False

res = []
panels = real_panels()
print('panels:', len(panels))
g0 = panels[0]
gi0 = gi_of(g0)
print('g0 -> gi', gi0)

# 1) gi0 菜单:显示标题(默认勾) + 隐藏面板,无删除
mh = open_menu(lambda: panel_rb(g0))
assert mh, 'panel menu fail'
items = menu_items(mh)
texts = [t for t, c in items]
print('gi0 menu texts:', texts)
esc_all()
ts = [c for t, c in items if t and '显示标题' in t]
ok1 = ts and ts[0] and any('隐藏面板' in t for t in texts) and \
      not any('删除面板' in t for t in texts)
res.append(bool(ok1))
print('1 menu content:', 'PASS' if ok1 else 'FAIL')

# 2) 标题切换:选 usersize=0 的面板(手动尺寸优先,高度不随标题变)
g0 = [p for p in real_panels()
      if val('p%d.usersize' % (gi_of(p) if gi_of(p) is not None else 99)) == 0][0]
gi0 = gi_of(g0)
print('step2 target gi', gi0, 'usersize', val('p%d.usersize' % gi0))
h0 = rect(g0)[3]
user32.PostMessageW(ct.c_void_p(g0), WM_COMMAND, 2200 + gi0, 0); time.sleep(0.7)
h1 = rect(g0)[3]
tshow = val('p%d.tshow' % gi0)
mh = open_menu(lambda: panel_rb(g0))
items2 = menu_items(mh) if mh else []
if not items2:
    mh = open_menu(lambda: panel_rb(g0))
    items2 = menu_items(mh) if mh else []
esc_all()
ts2 = [c for t, c in items2 if t and '显示标题' in t]
ok2 = tshow == 0 and h1 == h0 - 28 and ts2 and not ts2[0]
res.append(bool(ok2))
print('2 title off: h %d->%d tshow=%s checked=%s => %s'
      % (h0, h1, tshow, ts2, 'PASS' if ok2 else 'FAIL'))
user32.PostMessageW(ct.c_void_p(g0), WM_COMMAND, 2200 + gi0, 0); time.sleep(0.7)
ok2b = rect(g0)[3] == h0 and val('p%d.tshow' % gi0) == 1
res.append(ok2b)
print('2b title back:', 'PASS' if ok2b else 'FAIL')

# 3) 隐藏文件面板
g1 = [p for p in panels if rect(p)[2] > 500][0]
gi1 = gi_of(g1)
user32.PostMessageW(ct.c_void_p(g1), WM_COMMAND, 2000 + gi1, 0)
ok3 = wait_vis(g1, False) and val('p%d.hidden' % gi1) == 1
res.append(ok3)
print('3 hide g%d:' % gi1, 'PASS' if ok3 else 'FAIL vis=%s reg=%s'
      % (bool(user32.IsWindowVisible(ct.c_void_p(g1))), val('p%d.hidden' % gi1)))

# 4) 托盘重显
any_p = real_panels()[0]
mh = open_menu(lambda: tray_rb(any_p))
assert mh, 'tray menu fail'
items = menu_items(mh)
esc_all()
reshow = [t for t, c in items if t and t.startswith('显示面板:')]
ok4 = len(reshow) == 1
res.append(ok4)
print('4 tray reshow:', reshow, '=>', 'PASS' if ok4 else 'FAIL')
if reshow:
    user32.PostMessageW(ct.c_void_p(any_p), WM_COMMAND, 2100 + gi1, 0)
    ok4b = wait_vis(g1, True) and val('p%d.hidden' % gi1) == 0
    res.append(ok4b)
    print('4b reshow:', 'PASS' if ok4b else 'FAIL')

# 5) 自建面板含删除项
found_del = False
for p in real_panels():
    mh = open_menu(lambda pp=p: panel_rb(pp))
    if not mh:
        continue
    items = menu_items(mh)
    esc_all()
    if any(t and '删除面板' in t for t, c in items):
        found_del = True
        break
res.append(found_del)
print('5 delete on user panel:', 'PASS' if found_del else 'FAIL')

print('RESULT:', 'PASS' if all(res) else 'FAIL', res)
