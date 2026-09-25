# -*- coding: utf-8 -*-
# 打开设置对话框,截图验证空行已消除
import ctypes as ct, time, sys
u = ct.windll.user32
WM_COMMAND = 0x111
IDM_SETTINGS = 1010

def enum_cb(h, l):
    buf = ct.create_unicode_buffer(256)
    u.GetClassNameW(h, buf, 256)
    if buf.value == 'RdesktopPanel':
        globals().panels.append(h)
    return True

panels = []
WNDENUMPROC = ct.WINFUNCTYPE(ct.c_bool, ct.c_void_p, ct.c_void_p)
u.EnumWindows(WNDENUMPROC(enum_cb), 0)
if not panels:
    print('FAIL: no panel'); sys.exit(1)

u.PostMessageW(ct.c_void_p(panels[0]), WM_COMMAND, IDM_SETTINGS, 0)
time.sleep(1.0)

# 找设置窗口(标题 桌面管理 / Desktop Manager)
sh = u.FindWindowW(None, '桌面管理')
if not sh:
    sh = u.FindWindowW(None, 'Desktop Manager')
if not sh:
    print('FAIL: settings window not found'); sys.exit(1)

r = ct.wintypes.RECT() if hasattr(ct, 'wintypes') else None
import ctypes.wintypes as wt
u.GetWindowRect(sh, ct.byref(r))
print('settings rect', r.left, r.top, r.right, r.bottom, 'h=', r.bottom-r.top)
u.SetForegroundWindow(sh)
time.sleep(0.5)

from PIL import ImageGrab
img = ImageGrab.grab(bbox=(r.left, r.top, r.right, r.bottom))
img.save('tools/settings_gap.png')
print('OK saved tools/settings_gap.png')

# 关闭对话框(取消)
IDC_CANCEL = 1002
u.PostMessageW(ct.c_void_p(sh), WM_COMMAND, IDC_CANCEL, 0)
