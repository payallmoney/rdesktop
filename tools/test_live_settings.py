# -*- coding: utf-8 -*-
# 验证设置即时生效:无确定/取消按钮;勾选/下拉/输入变化后注册表立即更新
import ctypes as ct, ctypes.wintypes as wt, time, subprocess
u = ct.windll.user32
u.SetProcessDpiAwarenessContext(ct.c_void_p(-4))
WM_COMMAND = 0x111
WM_KEYDOWN, WM_KEYUP, WM_CHAR = 0x100, 0x101, 0x102
IDC_FROST, IDC_TIDY, IDC_GRID, IDC_RADIUS, IDC_CGAP, IDC_RGAP, IDC_Z0 = 201, 203, 204, 205, 206, 209, 211
CBN_SELCHANGE, EN_CHANGE = 1, 0x300

def reg(name):
    r = subprocess.run(['reg', 'query', r'HKCU\Software\rdesktop', '/v', name],
                       capture_output=True, text=True)
    for ln in r.stdout.splitlines():
        ln = ln.strip()
        if ln.startswith(name):
            return ln.split()[2]
    return '?'

def dlg_item(d, i):
    return u.GetDlgItem(ct.c_void_p(d), i)

def mkwp(id_, code):
    return ct.c_void_p(id_ | (code << 16))

def wp_click(id_):
    return ct.c_void_p(id_)  # hiword 0 = BN_CLICKED

panels = []
@ct.WINFUNCTYPE(ct.c_bool, ct.c_void_p, ct.c_void_p)
def pcb(h, l):
    b = ct.create_unicode_buffer(64); u.GetClassNameW(h, b, 64)
    if b.value == 'RdGroupPanel': panels.append(h)
    return True
u.EnumWindows(pcb, 0)
u.PostMessageW(ct.c_void_p(panels[0]), WM_COMMAND, wp_click(1010), ct.c_void_p(0))
time.sleep(1.0)
sh = u.FindWindowW('RdSettingsWnd', None)
ok = bool(sh)
print('1 dialog opened:', ok)
if not ok: raise SystemExit(1)

# 2) 无确定/取消按钮
ok_ok = (u.GetDlgItem(ct.c_void_p(sh), 207) or u.GetDlgItem(ct.c_void_p(sh), 208))
print('2 OK/Cancel buttons gone:', not ok_ok)

wr = wt.RECT(); u.GetWindowRect(ct.c_void_p(sh), ct.byref(wr))
print('   window H =', wr.bottom - wr.top, '(was 734 with buttons)')

# 3) 毛玻璃:翻转勾选 → 注册表立即变化
before = reg('frosted')
chk = dlg_item(sh, IDC_FROST)
cur = u.SendMessageW(ct.c_void_p(chk), 0x00F0, ct.c_void_p(0), ct.c_void_p(0))  # BM_GETCHECK
u.SendMessageW(ct.c_void_p(chk), 0x00F1, ct.c_void_p(1 - cur), ct.c_void_p(0))  # BM_SETCHECK
u.PostMessageW(ct.c_void_p(sh), WM_COMMAND, wp_click(IDC_FROST), ct.c_void_p(0))
time.sleep(0.5)
after = reg('frosted')
print('3 frosted immediate:', before, '->', after, 'PASS' if before != after else 'FAIL')

# 翻回原值
u.SendMessageW(ct.c_void_p(chk), 0x00F1, ct.c_void_p(cur), ct.c_void_p(0))
u.PostMessageW(ct.c_void_p(sh), WM_COMMAND, wp_click(IDC_FROST), ct.c_void_p(0))
time.sleep(0.3)
print('   frosted restored:', reg('frosted'))

# 4) 对齐下拉:CB_SETCURSEL + CBN_SELCHANGE → grid 立即变化
before = reg('grid')
cbg = dlg_item(sh, IDC_GRID)
old_sel = u.SendMessageW(ct.c_void_p(cbg), 0x0147, ct.c_void_p(0), ct.c_void_p(0))  # CB_GETCURSEL
new_sel = (old_sel + 1) % 4
u.SendMessageW(ct.c_void_p(cbg), 0x014E, ct.c_void_p(new_sel), ct.c_void_p(0))  # CB_SETCURSEL
u.PostMessageW(ct.c_void_p(sh), WM_COMMAND, mkwp(IDC_GRID, CBN_SELCHANGE), ct.c_void_p(cbg))
time.sleep(0.5)
after = reg('grid')
print('4 grid immediate:', before, '->', after, 'PASS' if before != after else 'FAIL')
u.SendMessageW(ct.c_void_p(cbg), 0x014E, ct.c_void_p(old_sel), ct.c_void_p(0))
u.PostMessageW(ct.c_void_p(sh), WM_COMMAND, mkwp(IDC_GRID, CBN_SELCHANGE), ct.c_void_p(cbg))
time.sleep(0.3)
print('   grid restored:', reg('grid'))

# 5) 圆角输入:WM_CHAR 注入 '0' → EN_CHANGE → 400ms 防抖 → 钳制到 64
before = reg('radius')
ed = dlg_item(sh, IDC_RADIUS)
u.PostMessageW(ct.c_void_p(ed), WM_CHAR, ord('0'), ct.c_void_p(0))
time.sleep(0.15)
mid = reg('radius')
time.sleep(0.7)
after = reg('radius')
print('5 radius debounce:', before, '(typing) ->', after, 'PASS' if after == '64' else 'FAIL', ' mid was', mid)

# 退格恢复 24
for _ in range(1):
    u.PostMessageW(ct.c_void_p(ed), WM_CHAR, 8, ct.c_void_p(0))
time.sleep(0.7)
print('   radius backspace ->', reg('radius'))

# 6) 图层下拉 gi=0:切到最高层 → WS_EX_TOPMOST 立即变化
g0 = panels[0]
ex0 = u.GetWindowLongW(ct.c_void_p(g0), -20)  # GWL_EXSTYLE
cb0 = dlg_item(sh, IDC_Z0 + 0)
u.SendMessageW(ct.c_void_p(cb0), 0x014E, ct.c_void_p(1), ct.c_void_p(0))
u.PostMessageW(ct.c_void_p(sh), WM_COMMAND, mkwp(IDC_Z0, CBN_SELCHANGE), ct.c_void_p(cb0))
time.sleep(0.6)
ex1 = u.GetWindowLongW(ct.c_void_p(g0), -20)
topmost = 8
print('6 layer immediate:', bool(ex0 & topmost), '->', bool(ex1 & topmost),
      'PASS' if (ex0 & topmost) != (ex1 & topmost) else 'FAIL')
# 切回
u.SendMessageW(ct.c_void_p(cb0), 0x014E, ct.c_void_p(0), ct.c_void_p(0))
u.PostMessageW(ct.c_void_p(sh), WM_COMMAND, mkwp(IDC_Z0, CBN_SELCHANGE), ct.c_void_p(cb0))
time.sleep(0.4)
ex2 = u.GetWindowLongW(ct.c_void_p(g0), -20)
print('   layer restored:', bool(ex2 & topmost))

# 关闭
u.PostMessageW(ct.c_void_p(sh), WM_COMMAND, wp_click(208), ct.c_void_p(0))
print('done')
