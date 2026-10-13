# -*- coding: utf-8 -*-
"""ctx 菜单全路径日志 + UI 线程看门狗"""
import io

# 1) desktop.rs:ctx 菜单路径布点
p = 'src/desktop.rs'
s = io.open(p, encoding='utf-8').read()
subs = [
    ('        if let Some(pidl) = item_pidl(item) {',
     '''        crate::panel::dlog("ctx: begin pidl bind");
        if let Some(pidl) = item_pidl(item) {
            crate::panel::dlog("ctx: pidl ok, GetUIObjectOf...");'''),
    ('''            if cm.is_none() {
                // 兜底:桌面文件夹 + 绝对 PIDL
                if let Ok(desk) = windows::Win32::UI::Shell::SHGetDesktopFolder() {''',
     '''            if cm.is_none() {
                crate::panel::dlog("ctx: GetUIObjectOf none, fallback desktop folder");
                // 兜底:桌面文件夹 + 绝对 PIDL
                if let Ok(desk) = windows::Win32::UI::Shell::SHGetDesktopFolder() {'''),
    ('''            if let Some(cm) = cm {
                    let hr = cm.QueryContextMenu(menu, 0, 1, 0x7FFF, 0);
                    if hr.is_ok() {''',
     '''            if let Some(cm) = cm {
                    crate::panel::dlog("ctx: QueryContextMenu enter(加载 shell 扩展,可能被第三方扩展卡住)");
                    let hr = cm.QueryContextMenu(menu, 0, 1, 0x7FFF, 0);
                    crate::panel::dlog("ctx: QueryContextMenu done");
                    if hr.is_ok() {'''),
]
for old, new in subs:
    n = s.count(old)
    assert n == 1, 'desktop anchor %d: %r' % (n, old[:60])
    s = s.replace(old, new)
io.open(p, 'w', encoding='utf-8', newline='').write(s)
print('desktop.rs ok')

# 2) app.rs:看门狗消息常量
p3 = 'src/app.rs'
s3 = io.open(p3, encoding='utf-8').read()
old3 = 'pub const WM_Z_REZ: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 5; // 事件驱动:立即纠正 z 序/救回最小化'
new3 = old3 + '\npub const WM_WATCHDOG: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 9; // 看门狗心跳:UI 线程泵到即自增'
assert s3.count(old3) == 1
s3 = s3.replace(old3, new3)
io.open(p3, 'w', encoding='utf-8', newline='').write(s3)
print('app.rs ok')
