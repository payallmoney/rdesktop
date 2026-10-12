# -*- coding: utf-8 -*-
"""续接:render.rs + panel.rs(app.rs/lang.rs 已在前一脚本生效)"""
import io

def splice(path, subs):
    s = io.open(path, encoding='utf-8').read()
    for old, new, cnt in subs:
        n = s.count(old)
        assert n == cnt, 'ANCHOR FAIL(%d!=%d) %s: %r' % (n, cnt, path, old[:70])
        s = s.replace(old, new)
    io.open(path, 'w', encoding='utf-8', newline='').write(s)
    print('ok', path)

splice('src/render.rs', [
    ("ICON_TOP + ICON_SZ + 30;", "self.icon_top() + self.icon_sz() + 30;", 1),
    ("(ICON_TOP + ICON_SZ + 26) as f32", "(self.icon_top() + self.icon_sz() + 26) as f32", 1),
    ("cy + ICON_TOP as f32 + ICON_SZ as f32", "cy + self.icon_top() as f32 + self.icon_sz() as f32", 2),
    ("(cw - ICON_SZ) as f32", "(cw - self.icon_sz()) as f32", 1),
    ("(gw - ICON_SZ) / 2", "(gw - self.icon_sz()) / 2", 1),
    ("ICON_SZ as f32,", "self.icon_sz() as f32,", 7),
    ("(MARGIN + ICON_TOP + ICON_SZ + 5) as f32", "(MARGIN + self.icon_top() + self.icon_sz() + 5) as f32", 1),
])

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
