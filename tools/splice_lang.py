# -*- coding: utf-8 -*-
"""语言切换功能接线:菜单项/持久化/设置与重命名对话框双语/托盘提示双语"""
import io

def splice(path, subs):
    s = io.open(path, encoding='utf-8').read()
    for old, new, cnt in subs:
        n = s.count(old)
        assert n == cnt, 'ANCHOR FAIL(%d!=%d) %s: %r' % (n, cnt, path, old[:60])
        s = s.replace(old, new)
    io.open(path, 'w', encoding='utf-8', newline='').write(s)
    print('ok', path, '(%d subs)' % len(subs))

# ---------- lang.rs:新键 + 持久化 + 默认名判断 ----------
splice('src/lang.rs', [
    ("""        "new_panel_name" => "新面板",
        _ => "",""",
     """        "new_panel_name" => "新面板",
        "app_name" => "桌面管理",
        "lang_switch" => "English",
        "tray_tip" => "桌面分组管理",
        "tray_info" => "已接管桌面图标:拖拽图标可在分组间移动,右键托盘退出",
        "show" => "显示",
        "title" => "标题",
        "hint_grid" => "拖动面板松手后,位置吸附到所选网格",
        "grid_off" => "关闭",
        "grid8" => "8 像素",
        "grid16" => "16 像素",
        "grid32" => "32 像素",
        _ => "",""", 1),
    ("""        "new_panel_name" => "New Panel",
        _ => "",""",
     """        "new_panel_name" => "New Panel",
        "app_name" => "Desktop Manager",
        "lang_switch" => "中文",
        "tray_tip" => "Desktop group manager",
        "tray_info" => "Desktop icons are managed in panels. Drag icons between groups; right-click the tray icon to exit.",
        "show" => "Show",
        "title" => "Title",
        "hint_grid" => "Panels snap to the selected grid when dropped",
        "grid_off" => "Off",
        "grid8" => "8 px",
        "grid16" => "16 px",
        "grid32" => "32 px",
        _ => "",""", 1),
    ("""pub fn group_title(gi: usize) -> &'static str {""",
     """/// 启动时读取持久化语言;无记录按系统 UI 语言探测
pub fn load_persisted() {
    match crate::regstore::get_dw("lang") {
        Some(v) => set_lang(if v != 0 { LANG_EN } else { LANG_ZH }),
        None => set_lang(detect_lang()),
    }
}

pub fn persist() {
    crate::regstore::set_dw("lang", get_lang() as u32);
}

/// 是否为任一语言下的默认组名(切语言时同步翻译未改名的组)
pub fn is_default_title(s: &str) -> bool {
    ["grp_folders", "grp_files", "grp_shortcuts", "grp_extras", "new_panel_name"]
        .iter()
        .any(|k| s == zh(k) || s == en(k))
}

pub fn group_title(gi: usize) -> &'static str {""", 1),
])

# ---------- main.rs:启动读持久化语言 ----------
splice('src/main.rs', [
    ("    crate::lang::set_lang(crate::lang::detect_lang());",
     "    crate::lang::load_persisted();", 1),
])

# ---------- app.rs:IDM_LANG + retitle_defaults ----------
splice('src/app.rs', [
    ("pub const IDM_LOCK: usize = 1012; // 锁定布局 // 开机自启动(HKCU Run 键)",
     "pub const IDM_LOCK: usize = 1012; // 锁定布局 // 开机自启动(HKCU Run 键)\npub const IDM_LANG: usize = 1013; // 切换语言(中文/English)", 1),
    ("    pub fn save_config(&self) {",
     """    /// 切换语言后,把未改名的默认组标题同步成新语言
    pub fn retitle_defaults(&mut self) {
        let keys = ["grp_folders", "grp_files", "grp_shortcuts", "grp_extras", "new_panel_name"];
        for g in self.groups.iter_mut() {
            for k in keys {
                if g.title == crate::lang::zh(k) || g.title == crate::lang::en(k) {
                    g.title = crate::lang::t(k).to_string();
                    break;
                }
            }
        }
    }

    pub fn save_config(&self) {""", 1),
])

# ---------- panel.rs:菜单项 + 分发 + 提示文案 ----------
splice('src/panel.rs', [
    ("    HOVER_TITLE, IDM_LOCK, IDM_PHIDE, IDM_PDEL, IDM_PALGN, IDM_PNEW, IDM_PRENAME, IDM_PSHOW, IDM_PTITLE,",
     "    HOVER_TITLE, IDM_LOCK, IDM_LANG, IDM_PHIDE, IDM_PDEL, IDM_PALGN, IDM_PNEW, IDM_PRENAME, IDM_PSHOW, IDM_PTITLE,", 1),
    # 两个菜单(面板/托盘)的自启动项后面都加语言切换
    ('        let _ = AppendMenuW(menu, MF_STRING | ar, IDM_AUTORUN, PCWSTR(ws(crate::lang::t("autostart")).as_ptr()));',
     '        let _ = AppendMenuW(menu, MF_STRING | ar, IDM_AUTORUN, PCWSTR(ws(crate::lang::t("autostart")).as_ptr()));\n        let _ = AppendMenuW(menu, MF_STRING, IDM_LANG, PCWSTR(ws(crate::lang::t("lang_switch")).as_ptr()));', 2),
    ('                let t = ws(&format!("显示面板:{}", g.title));',
     '                let t = ws(&format!("{}{}", crate::lang::t("show_panel"), g.title));', 1),
    ('        copy_wstr(&mut nid.szTip, "rdesktop · 桌面分组管理");',
     '        copy_wstr(&mut nid.szTip, &format!("rdesktop · {}", crate::lang::t("tray_tip")));', 1),
    ('        copy_wstr(&mut nid.szInfo, "已接管桌面图标:拖拽图标可在分组间移动,右键托盘退出");',
     '        copy_wstr(&mut nid.szInfo, crate::lang::t("tray_info"));', 1),
    ("""                IDM_AUTORUN => {
                    // 注册表即开关状态:读取现值取反写回
                    let on = !desktop::autorun_enabled();
                    desktop::autorun_set(on);
                }""",
     """                IDM_AUTORUN => {
                    // 注册表即开关状态:读取现值取反写回
                    let on = !desktop::autorun_enabled();
                    desktop::autorun_set(on);
                }
                IDM_LANG => {
                    crate::lang::set_lang(crate::lang::get_lang() ^ 1);
                    crate::lang::persist();
                    app.retitle_defaults();
                    app.save_config();
                    app.render_all();
                    // 设置窗口还是旧语言文案:关闭它,重开即新语言
                    if !app.settings_hwnd.is_invalid() {
                        let _ = PostMessageW(Some(app.settings_hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                }""", 1),
])

# ---------- settings.rs:全部文案走 t() ----------
splice('src/settings.rs', [
    ('const GRID_LABELS: [&str; 4] = ["关闭", "8 像素", "16 像素", "32 像素"];',
     """fn grid_labels() -> [&'static str; 4] {
    [crate::lang::t("grid_off"), crate::lang::t("grid8"),
     crate::lang::t("grid16"), crate::lang::t("grid32")]
}""", 1),
    ('const LAYER_LABELS: [&str; 2] = ["最低层(桌面之上,不遮挡应用)", "最高层(置顶显示)"];',
     """fn layer_labels() -> [&'static str; 2] {
    [crate::lang::t("layer_low"), crate::lang::t("layer_high")]
}""", 1),
    ("            combo_add(cb, &LAYER_LABELS);", "            combo_add(cb, &layer_labels());", 1),
    ("        combo_add(cb, &GRID_LABELS);", "        combo_add(cb, &grid_labels());", 1),
    ('        "毛玻璃(面板背景模糊)",', '        crate::lang::t("frosted"),', 1),
    ('        "圆角(px)",', '        crate::lang::t("radius"),', 1),
    ('        "自动整理(自动分组、按名称排序、桌面变化自动刷新)",', '        crate::lang::t("auto_tidy"),', 1),
    ('        "对齐",', '        crate::lang::t("grid_snap"),', 1),
    ('        "列间距(px)",', '        crate::lang::t("col_gap"),', 1),
    ('        "行间距(px)",', '        crate::lang::t("row_gap"),', 1),
    ('        "拖动面板松手后,位置吸附到所选网格",', '        crate::lang::t("hint_grid"),', 1),
    ('        "每个面板:图层 / 显示 / 标题",', '        crate::lang::t("per_panel_hdr"),', 1),
    ('            "显示",', '            crate::lang::t("show"),', 1),
    ('            "标题",', '            crate::lang::t("title"),', 1),
    ('        let title = ws("桌面管理");', '        let title = ws(crate::lang::t("app_name"));', 1),
    ('        let title = ws("重命名面板");', '        let title = ws(crate::lang::t("rename_title"));', 1),
    ('let _ = mk(hwnd, w!("BUTTON"), "确定", WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32), WINDOW_EX_STYLE(0), s(206), s(64), s(74), s(30), BTN_RNAME_OK, font);',
     'let _ = mk(hwnd, w!("BUTTON"), crate::lang::t("ok"), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32), WINDOW_EX_STYLE(0), s(206), s(64), s(74), s(30), BTN_RNAME_OK, font);', 1),
    ('let _ = mk(hwnd, w!("BUTTON"), "取消", WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32), WINDOW_EX_STYLE(0), s(290), s(64), s(74), s(30), BTN_RNAME_CANCEL, font);',
     'let _ = mk(hwnd, w!("BUTTON"), crate::lang::t("cancel"), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32), WINDOW_EX_STYLE(0), s(290), s(64), s(74), s(30), BTN_RNAME_CANCEL, font);', 1),
])
print('ALL DONE')
