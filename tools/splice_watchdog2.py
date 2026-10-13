# -*- coding: utf-8 -*-
"""看门狗:UI_TICK 静态 + WM_WATCHDOG 臂 + spawn_watchdog + main 挂载"""
import io

# 1) panel.rs
p = 'src/panel.rs'
s = io.open(p, encoding='utf-8').read()
subs = [
    ('    WM_OPEN_SETTINGS, WM_TRAY, WM_Z_PAUSE, WM_Z_REZ, DRAG_THRESHOLD,',
     '    WM_OPEN_SETTINGS, WM_TRAY, WM_WATCHDOG, WM_Z_PAUSE, WM_Z_REZ, DRAG_THRESHOLD,'),
    ('''/// WinEvent 投递目标:任一面板句柄(同线程消息队列)
static Z_SINK: AtomicIsize = AtomicIsize::new(0);''',
     '''/// WinEvent 投递目标:任一面板句柄(同线程消息队列)
static Z_SINK: AtomicIsize = AtomicIsize::new(0);

/// 看门狗心跳计数:UI 线程每泵到一次 WM_WATCHDOG 自增
static UI_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// UI 线程看门狗:线程被 COM/shell 调用(如第三方 shell 扩展卡住 QueryContextMenu)
/// 阻塞时,Win+D 等事件会无声失效(表现为"面板/图标不显示"),这里写日志让现场可见
pub fn spawn_watchdog(hwnd: HWND) {
    std::thread::spawn(move || {
        let mut strikes = 0u32;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let before = UI_TICK.load(std::sync::atomic::Ordering::SeqCst);
            let _ = PostMessageW(Some(hwnd), WM_WATCHDOG, WPARAM(0), LPARAM(0));
            std::thread::sleep(std::time::Duration::from_millis(1500));
            if UI_TICK.load(std::sync::atomic::Ordering::SeqCst) == before {
                strikes += 1;
                if strikes == 2 {
                    dlog("WATCHDOG: UI 线程 ~10s 未泵消息(可能被 COM/shell 调用阻塞)");
                } else if strikes % 12 == 0 {
                    dlog(&format!("WATCHDOG: UI 线程已卡 ~{}s", strikes * 6));
                }
            } else {
                strikes = 0;
            }
        }
    });
}'''),
]
for old, new in subs:
    n = s.count(old)
    assert n == 1, 'panel anchor %d: %r' % (n, old[:60])
    s = s.replace(old, new)

# wndproc 臂:锚定 WM_TRAY 臂开头
old5 = '        WM_TRAY => {'
n5 = s.count(old5)
assert n5 == 1, 'WM_TRAY anchor %d' % n5
s = s.replace(old5, '''        WM_WATCHDOG => {
            UI_TICK.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            LRESULT(0)
        }
        WM_TRAY => {''')
io.open(p, 'w', encoding='utf-8', newline='').write(s)
print('panel.rs ok')

# 2) main.rs:面板创建后挂看门狗
p2 = 'src/main.rs'
s2 = io.open(p2, encoding='utf-8').read()
old6 = '    unsafe { panel::create_windows(&mut app, hinst) };'
assert s2.count(old6) == 1
s2 = s2.replace(old6, '''    unsafe { panel::create_windows(&mut app, hinst) };
    panel::spawn_watchdog(app.groups[0].hwnd);''')
io.open(p2, 'w', encoding='utf-8', newline='').write(s2)
print('main.rs ok')
