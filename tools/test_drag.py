# -*- coding: utf-8 -*-
"""Drag/resize smoothness test: sequential posted messages where each lparam
is computed against the CURRENT window rect (emulates queued-message frame
mixing that made the old client-delta math bounce).
Asserts: every step applies the exact intended screen delta (monotonic, no
back-jump), and a reverse drag returns the panel to its exact origin."""
import ctypes as ct
import ctypes.wintypes as w
import time

user32 = ct.windll.user32
user32.SetProcessDpiAwarenessContext(ct.c_void_p(-4))
WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_LBUTTONUP = 0x201, 0x200, 0x202
MARGIN, TITLE_H = 26, 40

def pack(x, y):
    return ((y & 0xFFFF) << 16) | (x & 0xFFFF)

def rect(h):
    r = w.RECT()
    user32.GetWindowRect(ct.c_void_p(h), ct.byref(r))
    return (r.left, r.top, r.right, r.bottom)

def wait_moved(h, prev, timeout=0.4):
    t0 = time.time()
    while time.time() - t0 < timeout:
        cur = rect(h)
        if cur != prev:
            return cur
        time.sleep(0.005)
    return rect(h)

# pick a visible non-ghost panel
panel = None
@ct.WINFUNCTYPE(ct.c_bool, w.HWND, w.LPARAM)
def cb(h, lp):
    global panel
    clsb = ct.create_unicode_buffer(64); user32.GetClassNameW(h, clsb, 64)
    if clsb.value == 'RdGroupPanel':
        ex = user32.GetWindowLongPtrW(h, -20)
        if not (ex & 0x20) and user32.IsWindowVisible(h) and panel is None:
            panel = h if isinstance(h, int) else h.value
    return True
user32.EnumWindows(cb, 0)
assert panel, 'no panel'
H = ct.c_void_p(panel)

fails = []
def check(cond, msg):
    if not cond:
        fails.append(msg)
    print(('  ok  ' if cond else '  FAIL') + ' ' + msg)

# ---------- drag round-trip ----------
o0 = rect(panel)
c0 = (MARGIN + 60, MARGIN + 10)          # title area
s0 = (o0[0] + c0[0], o0[1] + c0[1])      # screen pos of press
user32.PostMessageW(H, WM_LBUTTONDOWN, 1, pack(*c0))
time.sleep(0.05)

N, DX, DY = 10, 7, 3
prev = rect(panel)
ok_steps = True
for k in range(1, N + 1):
    st = (s0[0] + DX * k, s0[1] + DY * k)     # intended cursor screen pos
    cur = rect(panel)
    c = (st[0] - cur[0], st[1] - cur[1])      # lparam vs CURRENT frame
    user32.PostMessageW(H, WM_MOUSEMOVE, 0, pack(*c))
    after = wait_moved(panel, cur)
    dl = after[0] - prev[0]
    dt = after[1] - prev[1]
    if dl < 0 or dt < 0:
        ok_steps = False
        fails.append('step %d back-jump d=(%d,%d)' % (k, dl, dt))
    prev = after
    print('  step %2d win=(%d,%d) d=(%d,%d) want=(%d,%d)%s'
          % (k, after[0], after[1], dl, dt, DX, DY,
             '' if (dl == DX and dt == DY) else '  << MISMATCH'))
    if dl != DX or dt != DY:
        ok_steps = False
    time.sleep(0.03)
user32.PostMessageW(H, WM_LBUTTONUP, 0, 0)
time.sleep(0.1)
f1 = rect(panel)
check(ok_steps, 'every step exact delta, monotonic (no bounce)')
check(f1[0] - o0[0] == N * DX and f1[1] - o0[1] == N * DY,
      'after drag1: offset=(%d,%d) want=(%d,%d)'
      % (f1[0] - o0[0], f1[1] - o0[1], N * DX, N * DY))

# ---------- reverse drag: back to origin (also fixes saved config) ----------
user32.PostMessageW(H, WM_LBUTTONDOWN, 1, pack(*c0))
time.sleep(0.05)
for k in range(1, N + 1):
    st = (s0[0] + DX * (N - k), s0[1] + DY * (N - k))
    cur = rect(panel)
    c = (st[0] - cur[0], st[1] - cur[1])
    user32.PostMessageW(H, WM_MOUSEMOVE, 0, pack(*c))
    wait_moved(panel, cur)
    time.sleep(0.03)
user32.PostMessageW(H, WM_LBUTTONUP, 0, 0)
time.sleep(0.15)
f2 = rect(panel)
check(f2 == o0, 'reverse drag restores exact origin: %s vs %s' % (str(f2), str(o0)))

# ---------- resize round-trip on right edge ----------
o = rect(panel)
w0, h0 = o[2] - o[0], o[3] - o[1]
cedge = (w0 - 5, MARGIN + h0 // 4)           # right edge of window, below title
rbase = (o[0] + cedge[0], o[1] + cedge[1])   # screen pos at press
user32.PostMessageW(H, WM_LBUTTONDOWN, 1, pack(*cedge))
time.sleep(0.05)
ok_r = True
prev = rect(panel)
for k in range(1, 6):
    cur = rect(panel)
    st = (rbase[0] + 5 * k, cur[1])          # cursor x +5/step (y fixed)
    c = (st[0] - cur[0], st[1] - cur[1])
    user32.PostMessageW(H, WM_MOUSEMOVE, 0, pack(*c))
    after = wait_moved(panel, cur)
    dw = (after[2] - after[0]) - (prev[2] - prev[0])
    if dw != 5:
        ok_r = False
        print('  resize step %d dw=%d want=5 << MISMATCH' % (k, dw))
    prev = after
    time.sleep(0.03)
user32.PostMessageW(H, WM_LBUTTONUP, 0, 0)
time.sleep(0.15)
check(ok_r, 'resize steps exact +5/step (no bounce)')

# resize back — press point must be on the CURRENT (grown) right edge
cur = rect(panel)
cw_now = cur[2] - cur[0]
new_edge = (cw_now - 5, MARGIN + h0 // 4)              # client point on current right edge
user32.PostMessageW(H, WM_LBUTTONDOWN, 1, pack(*new_edge))
time.sleep(0.05)
start_x = cur[0] + new_edge[0]                          # app's start = screen of press
prev = rect(panel)
ok_rb = True
for k in range(1, 6):
    cur2 = rect(panel)
    st = (start_x - 5 * k, cur2[1])
    c = (st[0] - cur2[0], st[1] - cur2[1])
    user32.PostMessageW(H, WM_MOUSEMOVE, 0, pack(*c))
    after = wait_moved(panel, cur2)
    dw = (after[2] - after[0]) - (prev[2] - prev[0])
    if dw != -5:
        ok_rb = False
        print('  resize-back step %d dw=%d want=-5 << MISMATCH' % (k, dw))
    prev = after
    time.sleep(0.03)
user32.PostMessageW(H, WM_LBUTTONUP, 0, 0)
time.sleep(0.15)
f3 = rect(panel)
check(ok_rb, 'resize-back steps exact -5/step')
check(f3 == o0, 'resize round-trip restores exact size/pos: %s vs %s'
      % (str(f3), str(o0)))

print('RESULT: %s (%d fails)' % ('PASS' if not fails else 'FAIL', len(fails)))
for f in fails:
    print('  -', f)
