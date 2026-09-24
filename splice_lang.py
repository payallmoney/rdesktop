# -*- coding: utf-8 -*-
"""双语支持:lang.rs + 全部菜单/对话框字符串替换 + 语言检测 + 设置切换"""
import io

def splice(path, old, new, tag, count=1):
    s = io.open(path, encoding='utf-8').read()
    assert s.count(old) == count, (tag, s.count(old))
    s = s.replace(old, new, count)
    io.open(path, 'w', encoding='utf-8', newline='').write(s)
    print('ok', tag)

# ========== panel.rs: tray + panel menu 双语 ==========
P = 'src/panel.rs'
# tray menu
splice(P, '''        let _ = AppendMenuW(menu, MF_STRING, IDM_SETTINGS, w!("设置(&T)..."));
        let mut any_hidden = false;''',
'''        let _ = AppendMenuW(menu, MF_STRING, IDM_SETTINGS, PCWSTR(ws(crate::lang::t("settings")).as_ptr()));
        let mut any_hidden = false;''', 'tray settings')
splice(P, '''        let _ = AppendMenuW(menu, MF_STRING, IDM_REFRESH, w!("刷新桌面分组(&R)"));''',
'''        let _ = AppendMenuW(menu, MF_STRING, IDM_REFRESH, PCWSTR(ws(crate::lang::t("refresh")).as_ptr()));''', 'tray refresh')
splice(P, '''        let _ = AppendMenuW(menu, MF_STRING | vis, IDM_TOGGLE_GROUPS, w!("显示分组面板(&G)"));''',
'''        let _ = AppendMenuW(menu, MF_STRING | vis, IDM_TOGGLE_GROUPS, PCWSTR(ws(crate::lang::t("show_panels")).as_ptr()));''', 'tray show panels')
splice(P, '''        let _ = AppendMenuW(menu, MF_STRING | org, IDM_TOGGLE_ORIG, w!("显示系统桌面图标(&S)"));''',
'''        let _ = AppendMenuW(menu, MF_STRING | org, IDM_TOGGLE_ORIG, PCWSTR(ws(crate::lang::t("show_sys_icons")).as_ptr()));''', 'tray show icons')
splice(P, '''        let _ = AppendMenuW(menu, MF_STRING | ar, IDM_AUTORUN, w!("开机自启动(&A)"));''',
'''        let _ = AppendMenuW(menu, MF_STRING | ar, IDM_AUTORUN, PCWSTR(ws(crate::lang::t("autostart")).as_ptr()));''', 'tray autostart')
splice(P, '''        let _ = AppendMenuW(menu, MF_STRING, IDM_EXIT, w!("退出并恢复桌面(&X)"));''',
'''        let _ = AppendMenuW(menu, MF_STRING, IDM_EXIT, PCWSTR(ws(crate::lang::t("exit")).as_ptr()));''', 'tray exit')

# panel menu
splice(P, '''        let _ = AppendMenuW(menu, MF_STRING, IDM_PNEW, w!("新建面板(&N)..."));''',
'''        let _ = AppendMenuW(menu, MF_STRING, IDM_PNEW, PCWSTR(ws(crate::lang::t("new_panel")).as_ptr()));''', 'panel new')
splice(P, '''        let _ = AppendMenuW(menu, MF_STRING, IDM_PRENAME + gi, w!("重命名面板(&M)..."));''',
'''        let _ = AppendMenuW(menu, MF_STRING, IDM_PRENAME + gi, PCWSTR(ws(crate::lang::t("rename_panel")).as_ptr()));''', 'panel rename')
splice(P, '''        let _ = AppendMenuW(sub, f, IDM_PALGN + gi * 4 + i, PCWSTR(t.as_ptr()));''',
'''        let _ = AppendMenuW(sub, f, IDM_PALGN + gi * 4 + i, PCWSTR(t.as_ptr()));''', 'noop align')  # already uses ws
splice(P, '''        let _ = AppendMenuW(menu, MF_POPUP, sub.0 as usize, w!("标题对齐(&A)"));''',
'''        let _ = AppendMenuW(menu, MF_POPUP, sub.0 as usize, PCWSTR(ws(crate::lang::t("title_align")).as_ptr()));''', 'panel title_align')
splice(P, '''        let ts = if app.groups[gi].title_show {''', '''        let ts = if app.groups[gi].title_show {''', 'noop ts')
splice(P, '''        let _ = AppendMenuW(menu, MF_STRING | ts, IDM_PTITLE + gi, w!("显示标题(&I)"));''',
'''        let _ = AppendMenuW(menu, MF_STRING | ts, IDM_PTITLE + gi, PCWSTR(ws(crate::lang::t("show_title")).as_ptr()));''', 'panel show_title')
splice(P, '''        let _ = AppendMenuW(menu, MF_STRING, IDM_PHIDE + gi, w!("隐藏面板(&H)"));''',
'''        let _ = AppendMenuW(menu, MF_STRING, IDM_PHIDE + gi, PCWSTR(ws(crate::lang::t("hide_panel")).as_ptr()));''', 'panel hide')
splice(P, '''            let _ = AppendMenuW(menu, MF_STRING, IDM_PDEL + gi, w!("删除面板(&D)"));''',
'''            let _ = AppendMenuW(menu, MF_STRING, IDM_PDEL + gi, PCWSTR(ws(crate::lang::t("delete_panel")).as_ptr()));''', 'panel delete')
# tray/panel exit already uses PCWSTR ✓
splice(P, '''        let _ = AppendMenuW(menu, MF_STRING, IDM_EXIT, PCWSTR(ws(crate::lang::t("exit")).as_ptr()));''',
'''        let _ = AppendMenuW(menu, MF_STRING, IDM_EXIT, PCWSTR(ws(crate::lang::t("exit")).as_ptr()));''', 'noop exit')

# 对齐子菜单双语
old_align = '''        for (i, label) in ["居左(&L)", "居中(&C)", "居右(&R)"].iter().enumerate() {'''
new_align = '''        let align_labels = if crate::lang::get_lang() == crate::lang::LANG_EN {
            ["Left(&L)", "Center(&C)", "Right(&R)"]
        } else {
            ["居左(&L)", "居中(&C)", "居右(&R)"]
        };
        for (i, label) in align_labels.iter().enumerate() {'''
splice(P, old_align, new_align, 'align labels')

print('ALL panel.rs DONE')
