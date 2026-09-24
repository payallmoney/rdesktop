# -*- coding: utf-8 -*-
"""设置清理:
1) 去掉全局「显示面板标题」勾选框(每面板右键/设置行已覆盖)
2) 每面板「显示面板」→「显示」
3) commit 不再引用 settings.show_title(纯 per-panel title_show 控制)
4) 确保全局设置(毛玻璃/自动整理/对齐/间距)生效"""
import io

def splice(path, old, new, tag, count=1):
    s = io.open(path, encoding='utf-8').read()
    assert s.count(old) == count, (tag, s.count(old))
    s = s.replace(old, new, count)
    io.open(path, 'w', encoding='utf-8', newline='').write(s)
    print('ok', tag)

S = 'src/settings.rs'

# 1) 删除全局 showtitle 勾选框的创建(populate 里的 mk 调用)
splice(S, '''    // 显示面板标题
    mk(
        hwnd,
        w!("BUTTON"),
        "显示面板标题",
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
        WINDOW_EX_STYLE(0),
        s(16),
        s(82),
        s(410),
        s(24),
        IDC_SHOWTITLE,
        font,
    );''', '''    // (全局「显示面板标题」已移除:每面板右键菜单和设置行内各自控制)''', 'remove global checkbox')

# 2) populate:去掉全局初值
splice(S, '    set_check(hwnd, IDC_SHOWTITLE, app.settings.show_title);\n', '', 'populate remove')

# 3) commit:去掉 showtitle 从 Settings 构造(不再从全局勾选读取)
splice(S, '''        show_title: get_check(hwnd, IDC_SHOWTITLE),''',
       '''        show_title: true, // per-panel title_show 为唯一控制''', 'commit remove showtitle')

# 4) commit:去掉全局批量覆盖块(已不需要,per-panel 循环直接覆盖)
splice(S, '''    if new.show_title != old.show_title {
        for g in app.groups.iter_mut() {
            g.title_show = new.show_title;
        }
    }
''', '', 'remove batch clobber')

# 5) 每面板勾选标签:「显示面板」→「显示」
splice(S, '"显示面板",', '"显示",', 'rename label')

# 6) 表头
splice(S, '"每个面板:图层 / 显示面板 / 标题"', '"每个面板:图层 / 显示 / 标题"', 'header')

# 7) IDC_SHOWTITLE const 保留但标 deprecated
splice(S, 'const IDC_SHOWTITLE: i32 = 209; // 显示面板标题',
       'const IDC_SHOWTITLE: i32 = 209; // (已移除对话框勾选框,保留 ID 避免冲突)', 'const note')

# 8) README 同步
r = io.open('README.md', encoding='utf-8').read()
old_r = '| **显示面板标题** | 关闭后不绘制标题文字,顶部收窄为细拖动条(仍可拖动面板,悬停显示移动光标),整版面立即重排;配置键 `showtitle` |\n'
assert r.count(old_r) == 1
r = r.replace(old_r, '', 1)
io.open('README.md', 'w', encoding='utf-8', newline='').write(r)
print('ok readme remove')

print('ALL DONE')
