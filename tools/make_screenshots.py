# -*- coding: utf-8 -*-
"""发布截图(零移动/零删除真实文件):
1) attrib +H 原地隐藏全部真实桌面条目(枚举已按 HIDDEN 属性过滤)
2) 创建纯测试内容(文件/文件夹/快捷方式)→ 重启应用
3) PrintWindow 截取各面板 + 设置窗口 → screenshots/
4) finally: 删除测试内容(仅本脚本创建)+ attrib -H 恢复 + 重启 + 校验104项
"""
import ctypes as ct
import ctypes.wintypes as w
import os
import shutil
import subprocess
import time
import zipfile

from PIL import Image, ImageDraw

u32 = ct.c_uint32
user32 = ct.windll.user32
gdi32 = ct.windll.gdi32
user32.SetProcessDpiAwarenessContext(ct.c_void_p(-4))

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DESK = os.path.join(os.environ['USERPROFILE'], 'Desktop')
SHOTS = os.path.join(ROOT, 'screenshots')
EXE = os.path.join(ROOT, 'target', 'release', 'rdesktop.exe')

TEST_NAMES = [
    '测试文件夹A', '测试文件夹B', '测试文本.txt', '会议纪要.docx', '数据表格.xlsx',
    '演示文稿.pptx', '说明文档.pdf', '项目压缩包.zip', '图片素材.png',
    '记事本快捷方式.lnk', '计算器快捷方式.lnk', '示例网站.url',
]

def kill():
    subprocess.run(['taskkill', '/IM', 'rdesktop.exe', '/F'], capture_output=True)

def start():
    subprocess.Popen([EXE])

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

def panels():
    return [p for p in find('RdGroupPanel')
            if not (user32.GetWindowLongPtrW(ct.c_void_p(p), -20) & 0x20)]

def snap(hwnd):
    r = w.RECT(); user32.GetWindowRect(ct.c_void_p(hwnd), ct.byref(r))
    W, H = r.right - r.left, r.bottom - r.top
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
    user32.PrintWindow(hwnd, mem, 2)
    data = ct.string_at(bits.value, W * H * 4)
    gdi32.SelectObject(mem, old); gdi32.DeleteObject(hbm); gdi32.DeleteDC(mem)
    user32.ReleaseDC(0, hdc)
    return Image.frombuffer('RGBA', (W, H), data, 'raw', 'BGRA', 0, 1)

def shot(hwnd, path):
    snap(hwnd).save(path)
    return path
    r = w.RECT(); user32.GetWindowRect(ct.c_void_p(hwnd), ct.byref(r))
    W, H = r.right - r.left, r.bottom - r.top
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
    user32.PrintWindow(hwnd, mem, 2)
    data = ct.string_at(bits.value, W * H * 4)
    gdi32.SelectObject(mem, old); gdi32.DeleteObject(hbm); gdi32.DeleteDC(mem)
    user32.ReleaseDC(0, hdc)
    img = Image.frombuffer('RGBA', (W, H), data, 'raw', 'BGRA', 0, 1)
    img.save(path)
    return path

def attrib(path, plus):
    flag = '+H' if plus else '-H'
    return subprocess.run(['attrib', flag, path], capture_output=True).returncode == 0

PUB = os.path.join('C:' + os.sep, 'Users', 'Public', 'Desktop')
real_before = os.listdir(DESK)
pub_before = [e for e in os.listdir(PUB) if not e.lower().startswith('desktop.ini')]
hidden = []      # 我们加了 H 的条目(用户桌面 + 公共桌面)
created = []     # 我们创建的测试条目
ok_all = True
try:
    # 1) 隐藏真实条目(仅属性,不动数据)
    for name in real_before:
        p = os.path.join(DESK, name)
        if not attrib(p, True):
            ok_all = False
            print('HIDE FAIL:', name)
            break
        hidden.append(name)
    assert ok_all, 'hide incomplete, abort before touching desktop further'
    for name in pub_before:
        attrib(os.path.join(PUB, name), True)
        hidden.append('PUBLIC::' + name)
    print('hidden real items:', len(hidden), '(+public %d)' % len(pub_before))

    # 2) 测试内容
    os.makedirs(os.path.join(DESK, '测试文件夹A'), exist_ok=True)
    os.makedirs(os.path.join(DESK, '测试文件夹B'), exist_ok=True)
    created += ['测试文件夹A', '测试文件夹B']
    def mk(name, data=b''):
        with open(os.path.join(DESK, name), 'wb') as f:
            f.write(data)
        created.append(name)
    mk('测试文本.txt', '这是测试文本文件\nrdesktop screenshot fixture\n'.encode('utf-8'))
    mk('会议纪要.docx', b'PK\x03\x04' + b'\x00' * 64)
    mk('数据表格.xlsx', b'PK\x03\x04' + b'\x00' * 64)
    mk('演示文稿.pptx', b'PK\x03\x04' + b'\x00' * 64)
    mk('说明文档.pdf', b'%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n')
    with zipfile.ZipFile(os.path.join(DESK, '项目压缩包.zip'), 'w') as z:
        z.writestr('readme.txt', 'test fixture')
    created.append('项目压缩包.zip')
    im = Image.new('RGB', (64, 64), (70, 130, 220))
    d = ImageDraw.Draw(im)
    d.ellipse((8, 8, 56, 56), fill=(240, 240, 245))
    im.save(os.path.join(DESK, '图片素材.png'))
    created.append('图片素材.png')
    try:
        import win32com.client
        ws = win32com.client.Dispatch('WScript.Shell')
        s1 = ws.CreateShortcut(os.path.join(DESK, '记事本快捷方式.lnk'))
        s1.TargetPath = r'C:\Windows\System32\notepad.exe'; s1.save()
        s2 = ws.CreateShortcut(os.path.join(DESK, '计算器快捷方式.lnk'))
        s2.TargetPath = r'C:\Windows\System32\calc.exe'; s2.save()
        created += ['记事本快捷方式.lnk', '计算器快捷方式.lnk']
    except Exception as e:
        print('lnk fail:', e)
    with open(os.path.join(DESK, '示例网站.url'), 'w', encoding='utf-8') as f:
        f.write('[InternetShortcut]\nURL=https://example.com/\n')
    created.append('示例网站.url')
    print('test items:', len(created))

    # 3) 重启 + 截图
    kill(); time.sleep(1)
    if os.path.isdir(SHOTS):
        shutil.rmtree(SHOTS)
    os.makedirs(SHOTS, exist_ok=True)
    start(); time.sleep(5)
    # 面板按注册表 p{i}.x 从左到右命名;空面板(中心无内容)跳过
    import winreg
    k = winreg.OpenKey(winreg.HKEY_CURRENT_USER, 'Software\\rdesktop')
    order = []
    for gi in range(20):
        try:
            x = winreg.QueryValueEx(k, 'p%d.x' % gi)[0]
            t = winreg.QueryValueEx(k, 'p%d.title' % gi)[0]
            order.append((x, gi, t))
        except FileNotFoundError:
            break
    order.sort()
    x_to_gi = {x: (gi, t) for x, gi, t in order}
    named = []
    for p in panels():
        r = w.RECT(); user32.GetWindowRect(ct.c_void_p(p), ct.byref(r))
        gx = r.left + 26
        gi_title = x_to_gi.get(gx)
        if not gi_title:
            continue
        img = snap(p).convert('RGB')
        W, H = img.size
        center = img.crop((60, 120, max(61, W - 60), max(121, H - 60)))
        pxs = list(center.getdata())
        ink = sum(1 for rr, gg, bb in pxs if min(rr, gg, bb) > 70)
        if ink < 400:
            print('skip empty panel gi=%s' % gi_title[0])
            continue
        named.append((p, gi_title[1]))
    tmap = {'文件夹': 'folders', '文件': 'files', '快捷方式': 'shortcuts', '其他快捷功能': 'extras'}
    for p, t in named:
        nm = tmap.get(t, 'panel-' + t)
        shot(p, os.path.join(SHOTS, 'panel-%s.png' % nm))
        print('shot panel-%s.png' % nm)
    user32.PostMessageW(ct.c_void_p(ps[0]), 0x8002, 0, 0)
    time.sleep(0.9)
    sw = find('RdSettingsWnd')
    if sw:
        shot(sw[0], os.path.join(SHOTS, 'settings.png'))
        print('shot settings.png')
        user32.PostMessageW(ct.c_void_p(sw[0]), 0x111, 208, 0)
        time.sleep(0.3)
finally:
    # 4) 无条件恢复:删测试内容 → 取消隐藏 → 重启 → 校验
    kill(); time.sleep(1)
    for name in created:
        p = os.path.join(DESK, name)
        try:
            if os.path.isdir(p):
                shutil.rmtree(p, ignore_errors=True)
                if os.path.isdir(p):
                    print('WARN leftover dir:', name)
            elif os.path.exists(p):
                os.remove(p)
        except Exception as e:
            print('test cleanup fail:', name, e)
    for name in hidden:
        if name.startswith('PUBLIC::'):
            attrib(os.path.join(PUB, name.split('PUBLIC::', 1)[1]), False)
        else:
            attrib(os.path.join(DESK, name), False)
    after = os.listdir(DESK)
    restored = sorted(after) == sorted(real_before)
    print('desktop restored:', 'PASS' if restored else 'FAIL',
          len(after), 'vs', len(real_before))
    if not restored:
        missing = set(real_before) - set(after)
        extra = set(after) - set(real_before)
        print('  missing:', sorted(missing)[:10])
        print('  extra:', sorted(extra)[:10])
    start()
    print('shots:', sorted(os.listdir(SHOTS)) if os.path.isdir(SHOTS) else [])
