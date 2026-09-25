// 应用双语支持:中文(默认)/ English
use std::sync::atomic::{AtomicU8, Ordering};
static LANG: AtomicU8 = AtomicU8::new(0);

pub const LANG_ZH: u8 = 0;
pub const LANG_EN: u8 = 1;

pub fn set_lang(l: u8) { LANG.store(l, Ordering::SeqCst); }
pub fn get_lang() -> u8 { LANG.load(Ordering::SeqCst) }
pub fn is_en() -> bool { get_lang() == LANG_EN }

pub fn detect_lang() -> u8 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    let id = unsafe { GetUserDefaultUILanguage() };
    if id & 0x3FF == 0x04 { LANG_ZH } else { LANG_EN }
}

pub fn t(key: &str) -> &'static str {
    if is_en() { en(key) } else { zh(key) }
}

pub fn zh(key: &str) -> &'static str {
    match key {
        "settings" => "设置(&T)...",
        "refresh" => "刷新桌面分组(&R)",
        "show_panels" => "显示分组面板(&G)",
        "show_sys_icons" => "显示系统桌面图标(&S)",
        "autostart" => "开机自启动(&A)",
        "lock_layout" => "锁定布局(&L)",
        "exit" => "退出并恢复桌面(&X)",
        "new_panel" => "新建面板(&N)...",
        "rename_panel" => "重命名面板(&M)...",
        "title_align" => "标题对齐(&A)",
        "show_title" => "显示标题(&I)",
        "hide_panel" => "隐藏面板(&H)",
        "delete_panel" => "删除面板(&D)",
        "show_panel" => "显示面板:",
        "frosted" => "毛玻璃(面板背景模糊)",
        "radius" => "圆角(px)",
        "auto_tidy" => "自动整理(自动分组、按名称排序、桌面变化自动刷新)",
        "grid_snap" => "对齐",
        "col_gap" => "列间距(px)",
        "row_gap" => "行间距(px)",
        "per_panel_hdr" => "每个面板:图层 / 显示 / 标题",
        "layer_low" => "最低层(桌面之上,不遮挡应用)",
        "layer_high" => "最高层(置顶)",
        "ok" => "确定",
        "cancel" => "取消",
        "rename_title" => "重命名面板",
        "drag_hint" => "拖拽桌面图标到这里",
        "grp_folders" => "文件夹",
        "grp_files" => "文件",
        "grp_shortcuts" => "快捷方式",
        "grp_extras" => "其他快捷功能",
        "new_panel_name" => "新面板",
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
        _ => "",
    }
}

pub fn en(key: &str) -> &'static str {
    match key {
        "settings" => "Settings(&T)...",
        "refresh" => "Refresh desktop groups(&R)",
        "show_panels" => "Show group panels(&G)",
        "show_sys_icons" => "Show system desktop icons(&S)",
        "autostart" => "Start on boot(&A)",
        "lock_layout" => "Lock layout(&L)",
        "exit" => "Exit & restore desktop(&X)",
        "new_panel" => "New panel(&N)...",
        "rename_panel" => "Rename panel(&M)...",
        "title_align" => "Title alignment(&A)",
        "show_title" => "Show title(&I)",
        "hide_panel" => "Hide panel(&H)",
        "delete_panel" => "Delete panel(&D)",
        "show_panel" => "Show panel:",
        "frosted" => "Frosted glass (DWM blur)",
        "radius" => "Radius",
        "auto_tidy" => "Auto-tidy (group by type, sort by name, auto-refresh)",
        "grid_snap" => "Grid snap",
        "col_gap" => "Col gap",
        "row_gap" => "Row gap",
        "per_panel_hdr" => "Per panel: layer / show / title",
        "layer_low" => "Low layer (above desktop, below apps)",
        "layer_high" => "High layer (topmost)",
        "ok" => "OK",
        "cancel" => "Cancel",
        "rename_title" => "Rename panel",
        "drag_hint" => "Drag desktop icons here",
        "grp_folders" => "Folders",
        "grp_files" => "Files",
        "grp_shortcuts" => "Shortcuts",
        "grp_extras" => "Quick Actions",
        "new_panel_name" => "New Panel",
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
        _ => "",
    }
}

/// 启动时读取持久化语言;无记录按系统 UI 语言探测
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

pub fn group_title(gi: usize) -> &'static str {
    match gi {
        0 => t("grp_folders"),
        1 => t("grp_files"),
        2 => t("grp_shortcuts"),
        3 => t("grp_extras"),
        _ => t("new_panel_name"),
    }
}
