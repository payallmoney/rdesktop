# -*- coding: utf-8 -*-
"""图标大小可调:icon_pct(75~200%)持久化 + 菜单子菜单(托盘/面板) + 渲染/布局全链路生效"""
import io

def splice(path, subs):
    s = io.open(path, encoding='utf-8').read()
    for old, new, cnt in subs:
        n = s.count(old)
        assert n == cnt, 'ANCHOR FAIL(%d!=%d) %s: %r' % (n, cnt, path, old[:70])
        s = s.replace(old, new)
    io.open(path, 'w', encoding='utf-8', newline='').write(s)
    print('ok', path)

# ---------- app.rs:字段/方法/持久化/IDM ----------
splice('src/app.rs', [
    ("pub const IDM_LANG: usize = 1013; // 切换语言(中文/English)",
     "pub const IDM_LANG: usize = 1013; // 切换语言(中文/English)\npub const IDM_ICONSZ: usize = 1020; // +i 图标大小档位(0..4)", 1),
    ("    pub scale: f32, // dpi / 96.0",
     "    pub scale: f32, // dpi / 96.0\n    pub icon_pct: i32, // 图标大小百分比(50~200,默认 100,菜单可调)", 1),
    ("                icon_cache: Default::default(),",
     "                icon_pct: 100,\n                icon_cache: Default::default(),", 1),
    ("""    pub fn cell_w(&self) -> i32 {
        CONTENT_W + self.settings.col_gap.clamp(0, 48)
    }

    /// 当前行高 = 内容高 + 行间距
    pub fn cell_h(&self) -> i32 {
        CONTENT_H + self.settings.row_gap.clamp(0, 40)
    }""",
     """    /// 图标缩放比例(菜单可调 75%~200%,默认 100%)
    pub fn icon_scale(&self) -> f32 {
        self.icon_pct.clamp(50, 200) as f32 / 100.0
    }

    pub fn icon_sz(&self) -> i32 {
        (ICON_SZ as f32 * self.icon_scale()).round() as i32
    }

    pub fn icon_top(&self) -> i32 {
        (ICON_TOP as f32 * self.icon_scale()).round() as i32
    }

    pub fn content_w(&self) -> i32 {
        (CONTENT_W as f32 * self.icon_scale()).round() as i32
    }

    /// 内容高 = 图标上边距 + 图标 + 标签块(25 固定,文字不随图标缩放)
    pub fn content_h(&self) -> i32 {
        self.icon_top() + self.icon_sz() + 25
    }

    pub fn cell_w(&self) -> i32 {
        self.content_w() + self.settings.col_gap.clamp(0, 48)
    }

    /// 当前行高 = 内容高 + 行间距
    pub fn cell_h(&self) -> i32 {
        self.content_h() + self.settings.row_gap.clamp(0, 40)
    }""", 1),
    ("""        if let Some(v) = rs::get_dw("rowgap") {
            self.settings.row_gap = (v as i32).clamp(0, 40);
        }""",
     """        if let Some(v) = rs::get_dw("rowgap") {
            self.settings.row_gap = (v as i32).clamp(0, 40);
        }
        if let Some(v) = rs::get_dw("iconpct") {
            self.icon_pct = (v as i32).clamp(50, 200);
        }""", 1),
    ('        rs::set_dw("rowgap", self.settings.row_gap as u32);',
     '        rs::set_dw("rowgap", self.settings.row_gap as u32);\n        rs::set_dw("iconpct", self.icon_pct.clamp(50, 200) as u32);', 1),
])

# ---------- lang.rs:菜单文案 ----------
splice('src/lang.rs', [
    ('        "app_name" => "桌面管理",',
     '''        "app_name" => "桌面管理",
        "icon_size" => "图标大小",
        "is_small" => "小",
        "is_std" => "标准",
        "is_large" => "大",
        "is_xl" => "特大",
        "is_huge" => "超大",''', 1),
    ('        "app_name" => "Desktop Manager",',
     '''        "app_name" => "Desktop Manager",
        "icon_size" => "Icon size",
        "is_small" => "Small",
        "is_std" => "Standard",
        "is_large" => "Large",
        "is_xl" => "Extra large",
        "is_huge" => "Huge",''', 1),
])

# ---------- render.rs:全部走 icon_sz()/icon_top() ----------
splice('src/render.rs', [
    ("ICON_TOP + ICON_SZ + 30;", "self.icon_top() + self.icon_sz() + 30;", 1),
    ("(ICON_TOP + ICON_SZ + 26) as f32", "(self.icon_top() + self.icon_sz() + 26) as f32", 1),
    ("cy + ICON_TOP as f32 + ICON_SZ as f32", "cy + self.icon_top() as f32 + self.icon_sz() as f32", 2),
    ("(cw - ICON_SZ) as f32", "(cw - self.icon_sz()) as f32", 1),
    ("(gw - ICON_SZ) / 2", "(gw - self.icon_sz()) / 2", 1),
    ("ICON_SZ as f32,", "self.icon_sz() as f32,", 8),
    ("(MARGIN + ICON_TOP + ICON_SZ + 5) as f32", "(MARGIN + self.icon_top() + self.icon_sz() + 5) as f32", 1),
])

# ---------- panel.rs:导入/子菜单/分发 ----------
splice('src/panel.rs', [
    ("    HOVER_TITLE, IDM_LOCK, IDM_LANG, IDM_PHIDE,",
     "    HOVER_TITLE, IDM_ICONSZ, IDM_LOCK, IDM_LANG, IDM_PHIDE,", 1),
    ('        let _ = AppendMenuW(menu, MF_STRING, IDM_LANG, PCWSTR(ws(crate::lang::t("lang_switch")).as_ptr()));',
     '''        let _ = AppendMenuW(menu, MF_STRING, IDM_LANG, PCWSTR(ws(crate::lang::t("lang_switch")).as_ptr()));
        // 图标大小子菜单(档位见 IDM_ICONSZ 分发)
        if let Ok(sub_is) = CreatePopupMenu() {
            let sz_pcts = [75i32, 100, 125, 150, 200];
            let sz_keys = ["is_small", "is_std", "is_large", "is_xl", "is_huge"];
            for (i, pct) in sz_pcts.iter().enumerate() {
                let chk = if app.icon_pct == *pct { MF_CHECKED } else { MF_UNCHECKED };
                let lbl = ws(&format!("{} ({}%)", crate::lang::t(sz_keys[i]), pct));
                let _ = AppendMenuW(sub_is, MF_STRING | chk, IDM_ICONSZ + i, PCWSTR(lbl.as_ptr()));
            }
            let _ = AppendMenuW(menu, MF_POPUP, sub_is.0 as usize, PCWSTR(ws(crate::lang::t("icon_size")).as_ptr()));
        }''', 2),
    ("""                IDM_LANG => {
                    crate::lang::set_lang(crate::lang::get_lang() ^ 1);
                    crate::lang::persist();
                    app.retitle_defaults();
                    app.save_config();
                    app.render_all();
                    // 设置窗口还是旧语言文案:关闭它,重开即新语言
                    if !app.settings_hwnd.is_invalid() {
                        let _ = PostMessageW(Some(app.settings_hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                }""",
     """                IDM_LANG => {
                    crate::lang::set_lang(crate::lang::get_lang() ^ 1);
                    crate::lang::persist();
                    app.retitle_defaults();
                    app.save_config();
                    app.render_all();
                    // 设置窗口还是旧语言文案:关闭它,重开即新语言
                    if !app.settings_hwnd.is_invalid() {
                        let _ = PostMessageW(Some(app.settings_hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                }
                x if (IDM_ICONSZ..IDM_ICONSZ + 8).contains(&x) => {
                    let pcts = [75i32, 100, 125, 150, 200];
                    if let Some(&p) = pcts.get(x - IDM_ICONSZ) {
                        if app.icon_pct != p {
                            app.icon_pct = p;
                            app.save_config();
                            app.place_groups();
                            app.render_all();
                        }
                    }
                }""", 1),
])
print('ALL DONE')
