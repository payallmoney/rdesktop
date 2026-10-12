# -*- coding: utf-8 -*-
"""文字宽度可设:label_mult10(图标宽 × 1.0/1.2/1.5/2.0),子菜单+持久化+列宽联动"""
import io

def splice(path, subs):
    s = io.open(path, encoding='utf-8').read()
    for old, new, cnt in subs:
        n = s.count(old)
        assert n == cnt, 'ANCHOR FAIL(%d!=%d) %s: %r' % (n, cnt, path, old[:70])
        s = s.replace(old, new)
    io.open(path, 'w', encoding='utf-8', newline='').write(s)
    print('ok', path)

# ---------- app.rs ----------
splice('src/app.rs', [
    ("pub const IDM_ICONSZ: usize = 1020; // +i 图标大小档位(0..4)",
     "pub const IDM_ICONSZ: usize = 1020; // +i 图标大小档位(0..4)\npub const IDM_LBLW: usize = 1030; // +i 文字宽度档位(0..3)", 1),
    ("    pub icon_pct: i32, // 图标大小百分比(50~200,默认 100,菜单可调)",
     "    pub icon_pct: i32, // 图标大小百分比(50~200,默认 100,菜单可调)\n    pub label_mult10: i32, // 文字宽度 = 图标宽 × 倍数(10=1.0/12=1.2/15=1.5/20=2.0)", 1),
    ("                icon_pct: 100,",
     "                icon_pct: 100,\n                label_mult10: 10,", 1),
    ("""    pub fn content_w(&self) -> i32 {
        (CONTENT_W as f32 * self.icon_scale()).round() as i32
    }""",
     """    /// 格子内容宽 = 图标宽 × 文字宽度倍数(文字区与列宽由此决定)
    pub fn content_w(&self) -> i32 {
        ((ICON_SZ as f32 * self.icon_scale()) * (self.label_mult10 as f32 / 10.0)).round() as i32
    }""", 1),
    ('        rs::set_dw("iconpct", self.icon_pct.clamp(50, 200) as u32);',
     '        rs::set_dw("iconpct", self.icon_pct.clamp(50, 200) as u32);\n        rs::set_dw("labelmult", self.label_mult10.clamp(10, 20) as u32);', 1),
    ("""        if let Some(v) = rs::get_dw("iconpct") {
            self.icon_pct = (v as i32).clamp(50, 200);
        }""",
     """        if let Some(v) = rs::get_dw("iconpct") {
            self.icon_pct = (v as i32).clamp(50, 200);
        }
        if let Some(v) = rs::get_dw("labelmult") {
            let v = v as i32;
            self.label_mult10 = if (10..=20).contains(&v) { v } else { 10 };
        }""", 1),
])

# ---------- lang.rs ----------
splice('src/lang.rs', [
    ('        "icon_size" => "图标大小",',
     '        "icon_size" => "图标大小",\n        "label_width" => "文字宽度(图标倍数)",', 1),
    ('        "icon_size" => "Icon size",',
     '        "icon_size" => "Icon size",\n        "label_width" => "Text width (x icon)",', 1),
])

# ---------- panel.rs ----------
splice('src/panel.rs', [
    ("    HOVER_TITLE, IDM_ICONSZ, IDM_LOCK, IDM_LANG, IDM_PHIDE,",
     "    HOVER_TITLE, IDM_ICONSZ, IDM_LBLW, IDM_LOCK, IDM_LANG, IDM_PHIDE,", 1),
    ("""            let _ = AppendMenuW(menu, MF_POPUP, sub_is.0 as usize, PCWSTR(ws(crate::lang::t("icon_size")).as_ptr()));
        }""",
     """            let _ = AppendMenuW(menu, MF_POPUP, sub_is.0 as usize, PCWSTR(ws(crate::lang::t("icon_size")).as_ptr()));
        }
        // 文字宽度子菜单(图标宽度的倍数)
        if let Ok(sub_lw) = CreatePopupMenu() {
            let mults = [10i32, 12, 15, 20];
            for (i, m) in mults.iter().enumerate() {
                let chk = if app.label_mult10 == *m { MF_CHECKED } else { MF_UNCHECKED };
                let lbl = ws(&format!("{}×", *m as f32 / 10.0));
                let _ = AppendMenuW(sub_lw, MF_STRING | chk, IDM_LBLW + i, PCWSTR(lbl.as_ptr()));
            }
            let _ = AppendMenuW(menu, MF_POPUP, sub_lw.0 as usize, PCWSTR(ws(crate::lang::t("label_width")).as_ptr()));
        }""", 2),
    ("""                x if (IDM_ICONSZ..IDM_ICONSZ + 8).contains(&x) => {""",
     """                x if (IDM_LBLW..IDM_LBLW + 8).contains(&x) => {
                    let mults = [10i32, 12, 15, 20];
                    if let Some(&m) = mults.get(x - IDM_LBLW) {
                        if app.label_mult10 != m {
                            app.label_mult10 = m;
                            // 列宽变化:面板保持尺寸(与图标大小同样解耦),内容重排
                            app.place_groups();
                            app.save_config();
                            app.render_all();
                        }
                    }
                }
                x if (IDM_ICONSZ..IDM_ICONSZ + 8).contains(&x) => {""", 1),
])

# ---------- render.rs:标签框宽 = content_w,居中于格子 ----------
splice('src/render.rs', [
    ("""                // 标签
                Self::draw_text_layout(
                    &rt,
                    &self.dwrite,
                    &self.fmt_label,
                    &g.items[idx].name,
                    cx + 4.0,
                    cy + self.icon_top() as f32 + self.icon_sz() as f32 + 5.0,
                    (cw - 8) as f32,
                    20.0,""",
     """                // 标签(宽度 = 图标宽 × 倍数,居中于格子;超宽自动加 "...")
                let lw = self.content_w() as f32;
                let lx = cx + (cw as f32 - lw) / 2.0;
                Self::draw_text_layout(
                    &rt,
                    &self.dwrite,
                    &self.fmt_label,
                    &g.items[idx].name,
                    lx,
                    cy + self.icon_top() as f32 + self.icon_sz() as f32 + 5.0,
                    lw,
                    20.0,""", 1),
])
print('ALL DONE')
