# -*- coding: utf-8 -*-
"""新增面板的命名弹窗居屏幕中央;普通重命名保持面板中心"""
import io

p = 'src/settings.rs'
s = io.open(p, encoding='utf-8').read()
subs = [
    ('/// 打开"重命名面板"对话框(预填当前名称)\npub fn open_rename(app: &mut App, gi: usize) {',
     '''/// 打开"重命名面板"对话框(预填当前名称)
/// center=true 时居屏幕(工作区)中央——新增面板的命名弹窗;false 居目标面板中心
pub fn open_rename(app: &mut App, gi: usize, center: bool) {'''),
    ('''        // 显示在目标面板中心
        let g = &app.groups[gi];
        let cx = g.gx + g.panel_w / 2 - w / 2;
        let cy = g.gy + g.panel_h / 2 - h / 2;''',
     '''        // 定位:新面板命名居屏幕(工作区)中央;重命名已有面板居该面板中心
        let (cx, cy) = if center {
            (
                app.work.left + (app.work.right - app.work.left - w) / 2,
                app.work.top + (app.work.bottom - app.work.top - h) / 2,
            )
        } else {
            let g = &app.groups[gi];
            (g.gx + g.panel_w / 2 - w / 2, g.gy + g.panel_h / 2 - h / 2)
        };'''),
]
for old, new in subs:
    n = s.count(old)
    assert n == 1, 'settings anchor %d: %r' % (n, old[:60])
    s = s.replace(old, new)
io.open(p, 'w', encoding='utf-8', newline='').write(s)
print('settings.rs ok')

p2 = 'src/panel.rs'
s2 = io.open(p2, encoding='utf-8').read()
subs2 = [
    ('                    crate::settings::open_rename(app, gi);',
     '                    crate::settings::open_rename(app, gi, true); // 新面板:命名弹窗居屏幕中央'),
    ('                    crate::settings::open_rename(app, x - IDM_PRENAME);',
     '                    crate::settings::open_rename(app, x - IDM_PRENAME, false);'),
]
for old, new in subs2:
    n = s2.count(old)
    assert n == 1, 'panel anchor %d: %r' % (n, old[:60])
    s2 = s2.replace(old, new)
io.open(p2, 'w', encoding='utf-8', newline='').write(s2)
print('panel.rs ok')
