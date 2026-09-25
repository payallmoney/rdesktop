# -*- coding: utf-8 -*-
"""语言切换端到端验证:IDM_LANG 切 EN/中文,检查注册表、设置窗口标题与文案、默认组名翻译"""
import ctypes as ct, ctypes.wintypes as wt, time, subprocess
u = ct.windll.user32; g = ct.windll.gdi32
u.SetProcessDpiAwarenessContext(ct.c_void_p(-4))
WM_COMMAND, WM_CLOSE, WM_SETTEXT = 0x111, 0x0010, 0x000C
IDM_SETTINGS, IDM_LANG = 1010, 1013

def reg(name):
    r = subprocess.run(['reg','query',r'HKCU\Software\rdesktop','/v',name], capture_output=True, text=True)
    for ln in r.stdout.splitlines():
        if ln.strip().startswith(name): return ln.split()[2]
    return '?'

def shot(path):
    sh = u.FindWindowW('RdSettingsWnd', None)
    u.SetForegroundWindow(sh); time.sleep(0.5)
    wr = wt.RECT(); u.GetWindowRect(ct.c_void_p(sh), ct.byref(wr))
    W, H = wr.right-wr.left, wr.bottom-wr.top
    hdc = u.GetDC(0); mem = g.CreateCompatibleDC(hdc)
    class BMIH(ct.Structure):
        _fields_ = [('biSize', ct.c_uint32), ('biWidth', ct.c_int32), ('biHeight', ct.c_int32),
                    ('biPlanes', ct.c_uint16), ('biBitCount', ct.c_uint16), ('biCompression', ct.c_uint32),
                    ('biSizeImage', ct.c_uint32), ('biXPelsPerMeter', ct.c_int32), ('biYPelsPerMeter', ct.c_int32),
                    ('biClrUsed', ct.c_uint32), ('biClrImportant', ct.c_uint32)]
    class BMI(ct.Structure):
        _fields_ = [('h', BMIH), ('c', ct.c_uint32 * 3)]
    bmi = BMI(); bmi.h.biSize = ct.sizeof(BMIH); bmi.h.biWidth = W; bmi.h.biHeight = -H
    bmi.h.biPlanes = 1; bmi.h.biBitCount = 32
    bits = ct.c_void_p()
    hbm = g.CreateDIBSection(mem, ct.byref(bmi), 0, ct.byref(bits), None, 0)
    old = g.SelectObject(mem, hbm)
    g.BitBlt(mem, 0, 0, W, H, hdc, wr.left, wr.top, 0x00CC0020)
    from PIL import Image
    Image.frombuffer('RGBA', (W, H), ct.string_at(bits.value, W*H*4), 'raw', 'BGRA', 0, 1).save(path)
    g.SelectObject(mem, old); g.DeleteObject(hbm); g.DeleteDC(mem); u.ReleaseDC(0, hdc)
    return H

panels = []
@ct.WINFUNCTYPE(ct.c_bool, ct.c_void_p, ct.c_void_p)
def pcb(h, l):
    b = ct.create_unicode_buffer(64); u.GetClassNameW(h, b, 64)
    if b.value == 'RdGroupPanel': panels.append(h)
    return True
u.EnumWindows(pcb, 0)
print('panels:', len(panels))

# 中文基线
u.PostMessageW(ct.c_void_p(panels[0]), WM_COMMAND, ct.c_void_p(IDM_SETTINGS), ct.c_void_p(0))
time.sleep(1.0)
h_zh = u.FindWindowW('RdSettingsWnd', None)
n = u.GetWindowTextLengthW(ct.c_void_p(h_zh))
buf = ct.create_unicode_buffer(n+2); u.GetWindowTextW(ct.c_void_p(h_zh), buf, n+2)
print('1 zh title:', buf.value, '| lang reg:', reg('lang'))
H1 = shot('tools/lang_zh.png')
u.PostMessageW(ct.c_void_p(h_zh), WM_CLOSE, ct.c_void_p(0), ct.c_void_p(0)); time.sleep(0.4)

# 切到英语
u.PostMessageW(ct.c_void_p(panels[0]), WM_COMMAND, ct.c_void_p(IDM_LANG), ct.c_void_p(0))
time.sleep(0.8)
print('2 after toggle: lang reg =', reg('lang'), '(expect 0x1)')
u.PostMessageW(ct.c_void_p(panels[0]), WM_COMMAND, ct.c_void_p(IDM_SETTINGS), ct.c_void_p(0))
time.sleep(1.0)
h_en = u.FindWindowW('RdSettingsWnd', None)
n = u.GetWindowTextLengthW(ct.c_void_p(h_en))
buf = ct.create_unicode_buffer(n+2); u.GetWindowTextW(ct.c_void_p(h_en), buf, n+2)
print('3 en title:', buf.value)
H2 = shot('tools/lang_en.png')
u.PostMessageW(ct.c_void_p(h_en), WM_CLOSE, ct.c_void_p(0), ct.c_void_p(0)); time.sleep(0.4)
print('   window H: zh=%s en=%s' % (H1, H2))

# 切回中文
u.PostMessageW(ct.c_void_p(panels[0]), WM_COMMAND, ct.c_void_p(IDM_LANG), ct.c_void_p(0))
time.sleep(0.8)
print('4 toggle back: lang reg =', reg('lang'), '(expect 0x0)')

# 持久化:写 lang=1 重启后应仍为 EN(仅检查注册表由启动代码读取)
print('done')
