# -*- coding: utf-8 -*-
"""双语支持剩余步骤:替换所有菜单/对话框硬编码中文为 lang::t() 调用"""
import io

def splice(path, old, new, tag, count=1):
    s = io.open(path, encoding='utf-8').read()
    assert s.count(old) == count, (tag, s.count(old))
    s = s.replace(old, new, count)
    io.open(path, 'w', encoding='utf-8', newline='').write(s)
    print('ok', tag)

P = 'src/panel.rs'

# 托盘菜单
splice(P, 'w!("设置(&T)...")', 'PCWSTR(ws(crate::lang::t("settings")).as_ptr())', 'tray settings')
splice(P, 'w!("刷新桌面分组(&R)")', 'PCWSTR(ws(crate::lang::t("refresh")).as_ptr())', 'tray refresh')
splice(P, 'w!("显示分组面板(&G)")', 'PCWSTR(ws(crate::lang::t("show_panels")).as_ptr())', 'tray show panels')
splice(P, 'w!("显示系统桌面图标(&S)")', 'PCWSTR(ws(crate::lang::t("show_sys_icons")).as_ptr())', 'tray show icons')
splice(P, 'w!("开机自启动(&A)")', 'PCWSTR(ws(crate::lang::t("autostart")).as_ptr())', 'tray autostart')
splice(P, 'w!("退出并恢复桌面(&X)")', 'PCWSTR(ws(crate::lang::t("exit")).as_ptr())', 'tray exit')

# 面板菜单
splice(P, 'w!("新建面板(&N)...")', 'PCWSTR(ws(crate::lang::t("new_panel")).as_ptr())', 'panel new')
splice(P, 'w!("重命名面板(&M)...")', 'PCWSTR(ws(crate::lang::t("rename_panel")).as_ptr())', 'panel rename')
splice(P, 'w!("标题对齐(&A)")', 'PCWSTR(ws(crate::lang::t("title_align")).as_ptr())', 'panel title_align')
splice(P, 'w!("显示标题(&I)")', 'PCWSTR(ws(crate::lang::t("show_title")).as_ptr())', 'panel show_title')
splice(P, 'w!("隐藏面板(&H)")', 'PCWSTR(ws(crate::lang::t("hide_panel")).as_ptr())', 'panel hide')
splice(P, 'w!("删除面板(&D)")', 'PCWSTR(ws(crate::lang::t("delete_panel")).as_ptr())', 'panel delete')

# 对齐子菜单
splice(P, '''        for (i, label) in ["居左(&L)", "居中(&C)", "居右(&R)"].iter().enumerate() {''',
'''        let align_labels = if crate::lang::is_en() {
            ["Left(&L)", "Center(&C)", "Right(&R)"]
        } else {
            ["居左(&L)", "居中(&C)", "居右(&R)"]
        };
        for (i, label) in align_labels.iter().enumerate() {''', 'align labels')

# 托盘重显项(已有 ws(format) 模式,跳过)

# render.rs: 拖拽提示
R = 'src/render.rs'
splice(R, '"拖拽桌面图标到这里"', 'crate::lang::t("drag_hint")', 'render drag hint')

# app.rs: GROUP_TITLES 初始标题用 lang
s = io.open(A := 'src/app.rs', encoding='utf-8').read()
old_gt = 'pub const GROUP_TITLES'
if old_gt in s:
    # GROUP_TITLES 在 app.rs 中定义 — 替换为函数调用
    # 查找:大概 pub const GROUP_TITLES: [&str; 4] = ["文件夹","文件","快捷方式","其他快捷功能"];
    import re
    m = re.search(r'pub (?:const|static) GROUP_TITLES.*?\n', s)
    if m:
        print('GROUP_TITLES found:', m.group()[:80])
        # 改为 lang 感知的 fn
        s = s.replace(m.group(), '''pub fn group_titles() -> [&'static str; 4] {
    [crate::lang::t("grp_folders"), crate::lang::t("grp_files"),
     crate::lang::t("grp_shortcuts"), crate::lang::t("grp_extras")]
}
''')
        # 替换所有 GROUP_TITLES[i] 为 group_titles()[i]
        s = s.replace('GROUP_TITLES[i]', 'group_titles()[i]')
        io.open(A, 'w', encoding='utf-8', newline='').write(s)
        print('ok GROUP_TITLES → group_titles()')
    else:
        print('GROUP_TITLES pattern not found')

print('ALL DONE')
