// 设置窗口:毛玻璃 / 圆角 / 自动整理 / 对齐(网格吸附)
// 从托盘"设置"打开,确定后应用并持久化,取消则丢弃。
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND, DwmEnableBlurBehindWindow,
};
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, CreateFontW, CreateRoundRectRgn, CreateRectRgn, DeleteObject,
    GetSysColorBrush, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_QUALITY, FW_NORMAL,
    HFONT, HGDIOBJ, OUT_DEFAULT_PRECIS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::{ws, App, MARGIN};

const SETTINGS_CLASS: PCWSTR = w!("RdSettingsWnd");

// 控件 ID
const IDC_FROST: i32 = 201;
const IDC_RADIUS: i32 = 202;
const IDC_TIDY: i32 = 203;
const IDC_GRID: i32 = 204;
const TID_APPLY: usize = 3; // 数字输入防抖定时器
const IDC_CGAP: i32 = 205; // 列间距
const IDC_RGAP: i32 = 206; // 行间距
const IDC_SHOWTITLE: i32 = 209; // (已移除对话框勾选框,保留 ID 避免冲突)
// 间距档位
// 间距/圆角为直接输入的像素值(见 get_i32),按布局约束钳制:
// 列0..48 / 行0..40(与 cell_w/h 的 clamp 一致),圆角0..64
const IDC_Z0: i32 = 211; // 图层选择,IDC_Z0 + 组号(0..3)
const IDC_PHIDE: i32 = 240; // 每面板隐藏勾选:240+gi
const IDC_PTSHOW: i32 = 350; // 每面板显示标题勾选:350+gi(避开 PHIDE 区间 240..340)

// 圆角档位 / 对齐网格档位(与下拉顺序一一对应)
const GRIDS: [i32; 4] = [0, 8, 16, 32];

fn grid_labels() -> [&'static str; 4] {
    [crate::lang::t("grid_off"), crate::lang::t("grid8"),
     crate::lang::t("grid16"), crate::lang::t("grid32")]
}

const BASE_W: i32 = 440;
const BASE_H: i32 = 418;

// 图层选项(每个面板)
fn layer_labels() -> [&'static str; 2] {
    [crate::lang::t("layer_low"), crate::lang::t("layer_high")]
}

pub unsafe fn register_class(hinst: HINSTANCE) -> bool {
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: WNDCLASS_STYLES(0),
        lpfnWndProc: Some(settings_wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: crate::panel::tool_icon(),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: SETTINGS_CLASS,
        hIconSm: crate::panel::tool_icon(),
    };
    let ok1 = RegisterClassExW(&wc) != 0;
    let wc2 = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: WNDCLASS_STYLES(0),
        lpfnWndProc: Some(name_wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: crate::panel::tool_icon(),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: NAME_CLASS,
        hIconSm: crate::panel::tool_icon(),
    };
    let ok2 = RegisterClassExW(&wc2) != 0;
    ok1 && ok2
}

pub const NAME_CLASS: PCWSTR = w!("RdnNameWnd");
const EDT_NAME: i32 = 300;
const BTN_RNAME_OK: i32 = 301;
const BTN_RNAME_CANCEL: i32 = 302;

/// 打开"重命名面板"对话框(预填当前名称)
pub fn open_rename(app: &mut App, gi: usize) {
    if gi >= app.groups.len() {
        return;
    }
    unsafe {
    crate::panel::dlog("rename: enter");
        if !app.name_hwnd.is_invalid() {
            app.name_target = Some(gi);
            if let Ok(e) = GetDlgItem(Some(app.name_hwnd), EDT_NAME) {
                let t = ws(&app.groups[gi].title);
                let _ = SetWindowTextW(e, PCWSTR(t.as_ptr()));
            }
            force_foreground(app.name_hwnd);
            return;
        }
        // 字体(与设置窗共用缓存)
        if app.settings_font.is_invalid() {
        crate::panel::dlog("rename: fontbegin");
            let face = ws("Microsoft YaHei UI");
            app.settings_font = CreateFontW(
                -(13.0 * app.scale) as i32,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                DEFAULT_QUALITY,
                0,
                PCWSTR(face.as_ptr()),
            );
        }
        let font = app.settings_font;
        crate::panel::dlog("rename: font ok");
        let scale = app.scale;
        let s = move |v: i32| (v as f32 * scale) as i32;
        let mut rect = RECT { left: 0, top: 0, right: s(380), bottom: s(130) };
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
        let _ = AdjustWindowRectEx(&mut rect, style, false, WS_EX_DLGMODALFRAME);
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        // 显示在目标面板中心
        let g = &app.groups[gi];
        let cx = g.gx + g.panel_w / 2 - w / 2;
        let cy = g.gy + g.panel_h / 2 - h / 2;
        let x = cx.clamp(app.work.left + 8, (app.work.right - w - 8).max(app.work.left + 8));
        let y = cy.clamp(app.work.top + 8, (app.work.bottom - h - 8).max(app.work.top + 8));

        let ptr = app as *mut App;
        let Some(hinst) = module_handle() else { return };
        crate::panel::dlog("rename: hinst ok");
        let title = ws(crate::lang::t("rename_title"));
        let hwnd = match CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            NAME_CLASS,
            PCWSTR(title.as_ptr()),
            style,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(hinst),
            Some(ptr as *const core::ffi::c_void),
        ) {
            Ok(h) => h,
            Err(e) => {
                crate::panel::dlog(&format!("rename dlg create err: {e}"));
                return;
            }
        };
        app.name_hwnd = hwnd;
        app.name_target = Some(gi);
        crate::panel::dlog("rename: dlg ok");

        crate::panel::dlog("rename: before edit");
        let cur = ws(&app.groups[gi].title);
        let edt_res = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR(cur.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            s(16),
            s(16),
            s(348),
            s(28),
            Some(hwnd),
            Some(HMENU(EDT_NAME as usize as *mut core::ffi::c_void)),
            Some(hinst),
            None,
        );
        match edt_res {
            Ok(edt) => {
                crate::panel::dlog("rename: edit created");
                let _ = SendMessageW(edt, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1)));
                crate::panel::dlog("rename: setfont ok");
                crate::panel::dlog("rename: setsel ok");
            }
            Err(e) => crate::panel::dlog(&format!("rename: edit ERR {e}")),
        }
        crate::panel::dlog("rename: mk1 begin");
        let _ = mk(hwnd, w!("BUTTON"), crate::lang::t("ok"), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32), WINDOW_EX_STYLE(0), s(206), s(64), s(74), s(30), BTN_RNAME_OK, font);
        let _ = mk(hwnd, w!("BUTTON"), crate::lang::t("cancel"), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32), WINDOW_EX_STYLE(0), s(290), s(64), s(74), s(30), BTN_RNAME_CANCEL, font);
        crate::panel::dlog("rename: controls ok");
        crate::panel::dlog("rename: buttons ok");
        let _ = ShowWindow(hwnd, SW_SHOW);
        force_foreground(hwnd);
        crate::panel::dlog("rename: done");
    }
}

unsafe extern "system" fn name_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let app_ptr: *mut App = if msg == WM_NCCREATE {
        let cs = &*(lparam.0 as *const CREATESTRUCTW);
        let p = cs.lpCreateParams as *mut App;
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, p as isize);
        p
    } else {
        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App
    };
    match msg {
        // 必须转发给 DefWindowProc:窗口文本(标题)正是在这一步从
        // CREATESTRUCT 存入的,直接返回 1 会让标题永远为空
        WM_NCCREATE => DefWindowProcW(hwnd, msg, wparam, lparam),
        WM_COMMAND => {
            if app_ptr.is_null() {
                return LRESULT(0);
            }
            let app = &mut *app_ptr;
            let id = (wparam.0 & 0xFFFF) as i32;
            let code = ((wparam.0 >> 16) & 0xFFFF) as i32;
            {
                let mut b0 = [0u16; 64];
                let n0 = GetDlgItemTextW(hwnd, EDT_NAME, &mut b0);
                let t0 = if n0 > 0 { String::from_utf16_lossy(&b0[..n0 as usize]) } else { "<empty>".into() };
                crate::panel::dlog(&format!("name WM_COMMAND id={id} code={code} text={t0}"));
            }
            if id == BTN_RNAME_OK {
                let mut buf = [0u16; 160];
                let n1 = GetWindowTextW(hwnd, &mut buf); // 对照1:窗口级
                let n2 = GetDlgItemTextW(hwnd, EDT_NAME, &mut buf); // 对照2:控件级(主用)
                crate::panel::dlog(&format!(
                    "rename: GetWindowTextW={} GetDlgItemTextW={} raw={:04x} {:04x} {:04x}",
                    n1, n2, buf[0], buf[1], buf[2]
                ));
                if n2 > 0 {
                    let s = String::from_utf16_lossy(&buf[..n2 as usize]);
                    crate::panel::dlog(&format!("rename: text={s}"));
                    if let Some(gi) = app.name_target {
                        app.rename_panel(gi, &s);
                        crate::panel::dlog(&format!("rename: committed gi={gi}"));
                    }
                }
                let _ = DestroyWindow(hwnd);
            } else if id == BTN_RNAME_CANCEL {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            // 关闭改名框绝不退出主程序
            if !app_ptr.is_null() {
                (*app_ptr).name_hwnd = HWND::default();
                (*app_ptr).name_target = None;
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ================= 打开 =================

pub fn open_settings(app: &mut App) {
    unsafe {
        if !app.settings_hwnd.is_invalid() {
            force_foreground(app.settings_hwnd);
            let _ = ShowWindow(app.settings_hwnd, SW_SHOW);
            return;
        }
        // 字体(随 DPI,进程内复用)
        if app.settings_font.is_invalid() {
            let face = ws("Microsoft YaHei UI");
            app.settings_font = CreateFontW(
                -(13.0 * app.scale) as i32,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                DEFAULT_QUALITY,
                0,
                PCWSTR(face.as_ptr()),
            );
        }
        let font = app.settings_font;

        let scale = app.scale;
        let s = move |v: i32| (v as f32 * scale) as i32;
        let n_rows = app.groups.len() as i32;
        let mut rect = RECT { left: 0, top: 0, right: s(BASE_W), bottom: s(268 + 36 * n_rows) };
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
        let _ = AdjustWindowRectEx(&mut rect, style, false, WS_EX_DLGMODALFRAME);
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        // 右下角(工作区内、任务栏上方)
        let x = app.work.right - w - s(24);
        let y = app.work.bottom - h - s(28);

        let ptr = app as *mut App;
        let Some(hinst) = module_handle() else {
            crate::panel::dlog("settings: GetModuleHandle failed");
            return;
        };
        let title = ws(crate::lang::t("app_name"));
        let hwnd = match CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            SETTINGS_CLASS,
            PCWSTR(title.as_ptr()),
            style,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(hinst),
            Some(ptr as *const core::ffi::c_void),
        ) {
            Ok(h) => h,
            Err(e) => {
                crate::panel::dlog(&format!("settings create err: {e}"));
                return;
            }
        };
        app.settings_hwnd = hwnd;
        populate(app, hwnd, font);
        // 置顶打开:保证不被"最高层"面板或其他置顶窗口挡住
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = ShowWindow(hwnd, SW_SHOW);
        force_foreground(hwnd);
    }
}

fn force_foreground(hwnd: HWND) {
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            keybd_event, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_MENU,
        };
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_EXTENDEDKEY, 0);
        let _ = SetForegroundWindow(hwnd);
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
    }
}

fn module_handle() -> Option<HINSTANCE> {
    unsafe { GetModuleHandleW(None).ok().map(|hm| HINSTANCE(hm.0)) }
}

fn mk(
    parent: HWND,
    class: PCWSTR,
    text: &str,
    style: WINDOW_STYLE,
    ex: WINDOW_EX_STYLE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: i32,
    font: HFONT,
) -> Option<HWND> {
    unsafe {
        let t = ws(text);
        let r = CreateWindowExW(
            ex,
            class,
            PCWSTR(t.as_ptr()),
            style,
            x,
            y,
            w,
            h,
            Some(parent),
            Some(HMENU(id as usize as *mut core::ffi::c_void)),
            module_handle(),
            None,
        );
        if let Ok(ctrl) = &r {
            let _ = SendMessageW(
                *ctrl,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        r.ok()
    }
}

unsafe fn set_check(hwnd: HWND, id: i32, on: bool) {
    if let Ok(c) = GetDlgItem(Some(hwnd), id) {
        let _ = SendMessageW(c, BM_SETCHECK, Some(WPARAM(if on { 1 } else { 0 })), None);
    }
}

fn get_check(hwnd: HWND, id: i32) -> bool {
    unsafe {
        match GetDlgItem(Some(hwnd), id) {
            Ok(c) => SendMessageW(c, BM_GETCHECK, None, None).0 == 1,
            Err(_) => false,
        }
    }
}

unsafe fn combo_add(cb: HWND, items: &[&str]) {
    for it in items {
        let w = ws(it);
        let _ = SendMessageW(cb, CB_ADDSTRING, None, Some(LPARAM(w.as_ptr() as isize)));
    }
}

fn combo_sel(hwnd: HWND, id: i32, idx: usize) {
    unsafe {
        if let Ok(c) = GetDlgItem(Some(hwnd), id) {
            let _ = SendMessageW(c, CB_SETCURSEL, Some(WPARAM(idx)), None);
        }
    }
}

fn combo_get(hwnd: HWND, id: i32) -> Option<usize> {
    unsafe {
        let c = GetDlgItem(Some(hwnd), id).ok()?;
        let i = SendMessageW(c, CB_GETCURSEL, None, None).0;
        if i < 0 {
            None
        } else {
            Some(i as usize)
        }
    }
}

unsafe fn populate(app: &App, hwnd: HWND, font: HFONT) {
    let s = |v: i32| (v as f32 * app.scale) as i32;
    let chk = |style_extra: WINDOW_STYLE| {
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | style_extra
    };
    let _ = chk;

    // 毛玻璃
    mk(
        hwnd,
        w!("BUTTON"),
        crate::lang::t("frosted"),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
        WINDOW_EX_STYLE(0),
        s(16),
        s(14),
        s(400),
        s(24),
        IDC_FROST,
        font,
    );
    // 圆角(像素,直接输入)
    mk(
        hwnd,
        w!("STATIC"),
        crate::lang::t("radius"),
        WS_CHILD | WS_VISIBLE,
        WINDOW_EX_STYLE(0),
        s(16),
        s(52),
        s(64),
        s(20),
        0,
        font,
    );
    mk(
        hwnd,
        w!("EDIT"),
        &app.settings.radius.to_string(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER
            | WINDOW_STYLE((ES_AUTOHSCROLL | ES_NUMBER) as u32),
        WINDOW_EX_STYLE(0),
        s(84),
        s(48),
        s(72),
        s(24),
        IDC_RADIUS,
        font,
    );
    // (全局「显示面板标题」已移除:每面板右键菜单和设置行内各自控制)
    // 自动整理
    mk(
        hwnd,
        w!("BUTTON"),
        crate::lang::t("auto_tidy"),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
        WINDOW_EX_STYLE(0),
        s(16),
        s(86),
        s(410),
        s(24),
        IDC_TIDY,
        font,
    );
    // 对齐
    mk(
        hwnd,
        w!("STATIC"),
        crate::lang::t("grid_snap"),
        WS_CHILD | WS_VISIBLE,
        WINDOW_EX_STYLE(0),
        s(16),
        s(124),
        s(60),
        s(20),
        0,
        font,
    );
    let cb_g = mk(
        hwnd,
        w!("COMBOBOX"),
        "",
        WS_CHILD | WS_VISIBLE | WS_VSCROLL | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
        WINDOW_EX_STYLE(0),
        s(80),
        s(120),
        s(200),
        s(200),
        IDC_GRID,
        font,
    );
    // 行列间距(像素,直接输入)
    mk(
        hwnd,
        w!("STATIC"),
        crate::lang::t("col_gap"),
        WS_CHILD | WS_VISIBLE,
        WINDOW_EX_STYLE(0),
        s(16),
        s(158),
        s(64),
        s(22),
        0,
        font,
    );
    mk(
        hwnd,
        w!("EDIT"),
        &app.settings.col_gap.to_string(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER
            | WINDOW_STYLE((ES_AUTOHSCROLL | ES_NUMBER) as u32),
        WINDOW_EX_STYLE(0),
        s(84),
        s(154),
        s(72),
        s(24),
        IDC_CGAP,
        font,
    );
    mk(
        hwnd,
        w!("STATIC"),
        crate::lang::t("row_gap"),
        WS_CHILD | WS_VISIBLE,
        WINDOW_EX_STYLE(0),
        s(200),
        s(158),
        s(64),
        s(22),
        0,
        font,
    );
    mk(
        hwnd,
        w!("EDIT"),
        &app.settings.row_gap.to_string(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER
            | WINDOW_STYLE((ES_AUTOHSCROLL | ES_NUMBER) as u32),
        WINDOW_EX_STYLE(0),
        s(268),
        s(154),
        s(72),
        s(24),
        IDC_RGAP,
        font,
    );
    mk(
        hwnd,
        w!("STATIC"),
        crate::lang::t("hint_grid"),
        WS_CHILD | WS_VISIBLE,
        WINDOW_EX_STYLE(0),
        s(16),
        s(194),
        s(410),
        s(36),
        0,
        font,
    );
    // 图层(每个面板)
    mk(
        hwnd,
        w!("STATIC"),
        crate::lang::t("per_panel_hdr"),
        WS_CHILD | WS_VISIBLE,
        WINDOW_EX_STYLE(0),
        s(16),
        s(236),
        s(410),
        s(20),
        0,
        font,
    );
    for gi in 0..app.groups.len() {
        mk(
            hwnd,
            w!("STATIC"),
            &app.groups[gi].title,
            WS_CHILD | WS_VISIBLE,
            WINDOW_EX_STYLE(0),
            s(16),
            s(268 + gi as i32 * 36),
            s(118),
            s(22),
            0,
            font,
        );
        let cb = mk(
            hwnd,
            w!("COMBOBOX"),
            "",
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            WINDOW_EX_STYLE(0),
            s(140),
            s(264 + gi as i32 * 36),
            s(150),
            s(180),
            IDC_Z0 + gi as i32,
            font,
        );
        if let Some(cb) = cb {
            combo_add(cb, &layer_labels());
            let idx = if app.groups[gi].z_top { 1 } else { 0 };
            let _ = SendMessageW(cb, CB_SETCURSEL, Some(WPARAM(idx)), None);
        }
        // 每面板:隐藏 / 显示标题
        mk(
            hwnd,
            w!("BUTTON"),
            crate::lang::t("show"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            WINDOW_EX_STYLE(0),
            s(300),
            s(268 + gi as i32 * 36),
            s(64),
            s(22),
            IDC_PHIDE + gi as i32,
            font,
        );
        mk(
            hwnd,
            w!("BUTTON"),
            crate::lang::t("title"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            WINDOW_EX_STYLE(0),
            s(364),
            s(268 + gi as i32 * 36),
            s(60),
            s(22),
            IDC_PTSHOW + gi as i32,
            font,
        );
        set_check(hwnd, IDC_PHIDE + gi as i32, !app.groups[gi].hidden); // 勾=显示面板
        set_check(hwnd, IDC_PTSHOW + gi as i32, app.groups[gi].title_show);
    }
    // (无确定/取消按钮:所有设置项均为即时生效)

    // 初值
    set_check(hwnd, IDC_FROST, app.settings.frosted);
    set_check(hwnd, IDC_TIDY, app.settings.auto_tidy);
    if let Some(cb) = cb_g {
        combo_add(cb, &grid_labels());
        let idx = GRIDS.iter().position(|&g| g == app.settings.snap_grid).unwrap_or(0);
        let _ = SendMessageW(cb, CB_SETCURSEL, Some(WPARAM(idx)), None);
    }
}

/// 读取数字输入框:空/非法 → None(调用方回退旧值)
fn get_i32(hwnd: HWND, id: i32) -> Option<i32> {
    unsafe {
        let mut buf = [0u16; 16];
        let n = GetDlgItemTextW(hwnd, id, &mut buf);
        if n <= 0 {
            return None;
        }
        String::from_utf16_lossy(&buf[..n.max(0).min(16) as usize])
            .trim()
            .parse::<i32>()
            .ok()
    }
}

// ================= 即时应用 =================

/// 数字输入框防抖到期:读取并应用圆角/间距(空/非法回退旧值)
unsafe fn apply_numbers(app: &mut App, hwnd: HWND) {
    let old = app.settings.clone();
    let radius = get_i32(hwnd, IDC_RADIUS)
        .map(|v| v.clamp(0, 64))
        .unwrap_or(old.radius);
    let col_gap = get_i32(hwnd, IDC_CGAP)
        .map(|v| v.clamp(0, 48))
        .unwrap_or(old.col_gap);
    let row_gap = get_i32(hwnd, IDC_RGAP)
        .map(|v| v.clamp(0, 40))
        .unwrap_or(old.row_gap);
    if radius == old.radius && col_gap == old.col_gap && row_gap == old.row_gap {
        return;
    }
    app.settings.radius = radius;
    app.settings.col_gap = col_gap;
    app.settings.row_gap = row_gap;
    app.save_config();
    if col_gap != old.col_gap || row_gap != old.row_gap {
        // 间距变化:重新流式摆位 + 全量重绘
        app.place_groups();
    }
    app.render_all();
    apply_frosted(app);
    crate::panel::dlog(&format!(
        "settings live radius={radius} col={col_gap} row={row_gap}"
    ));
}

/// 启用/禁用各面板的 DWM 毛玻璃(模糊区域 = 面板圆角矩形)
pub fn apply_frosted(app: &mut App) {
    unsafe {
        for gi in 0..app.groups.len() {
            let hwnd = app.groups[gi].hwnd;
            if hwnd.is_invalid() {
                continue;
            }
            if app.settings.frosted && app.groups[gi].panel_w > 0 {
                let r = app.settings.radius.max(0);
                let l = MARGIN;
                let t = MARGIN;
                let rt = MARGIN + app.groups[gi].panel_w;
                let b = MARGIN + app.groups[gi].panel_h;
                let rg = if r <= 0 {
                    CreateRectRgn(l, t, rt, b)
                } else {
                    CreateRoundRectRgn(l, t, rt, b, r * 2, r * 2)
                };
                let bb = DWM_BLURBEHIND {
                    dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
                    fEnable: BOOL(1),
                    hRgnBlur: rg,
                    fTransitionOnMaximized: BOOL(0),
                };
                if let Err(e) = DwmEnableBlurBehindWindow(hwnd, &bb) {
                    crate::panel::dlog(&format!("frosted({gi}) err: {e}"));
                }
                let _ = DeleteObject(HGDIOBJ(rg.0));
            } else {
                let bb = DWM_BLURBEHIND {
                    dwFlags: DWM_BB_ENABLE,
                    fEnable: BOOL(0),
                    hRgnBlur: Default::default(),
                    fTransitionOnMaximized: BOOL(0),
                };
                let _ = DwmEnableBlurBehindWindow(hwnd, &bb);
            }
        }
    }
}

// ================= 窗口过程 =================

unsafe extern "system" fn settings_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let app_ptr: *mut App = if msg == WM_NCCREATE {
        let cs = &*(lparam.0 as *const CREATESTRUCTW);
        let p = cs.lpCreateParams as *mut App;
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, p as isize);
        p
    } else {
        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App
    };

    match msg {
        WM_NCCREATE => DefWindowProcW(hwnd, msg, wparam, lparam),
        WM_COMMAND => {
            if app_ptr.is_null() {
                return LRESULT(0);
            }
            let app = &mut *app_ptr;
            let id = (wparam.0 & 0xFFFF) as i32;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
            if code == BN_CLICKED {
                match id {
                    // 全局开关:点击立即生效
                    IDC_FROST => {
                        app.settings.frosted = get_check(hwnd, IDC_FROST);
                        app.save_config();
                        apply_frosted(app);
                        app.render_all();
                    }
                    IDC_TIDY => {
                        app.settings.auto_tidy = get_check(hwnd, IDC_TIDY);
                        app.save_config();
                        if app.settings.auto_tidy {
                            // 重新开启:清空手动序,回到自动排序并重排
                            app.saved_order.clear();
                            for gi in 0..app.groups.len() {
                                app.groups[gi].manual_ord = false;
                            }
                            app.refresh_from_disk();
                        }
                    }
                    // 每面板「隐藏」「标题」勾选:点击立即生效并保存
                    // (不必依赖“确定”;点 X 关闭同样已生效)
                    x if (IDC_PHIDE..IDC_PHIDE + 100).contains(&x) => {
                        let gi = (x - IDC_PHIDE) as usize;
                        if gi < app.groups.len() {
                            let show = get_check(hwnd, x); // 勾=显示面板
                            if app.groups[gi].hidden == show {
                                app.groups[gi].hidden = !show;
                                let h = app.groups[gi].hwnd;
                                if !h.is_invalid() {
                                    if show {
                                        if app.groups_visible {
                                            let _ = ShowWindow(h, SW_RESTORE);
                                            app.render_panel(gi);
                                        }
                                    } else {
                                        let _ = ShowWindow(h, SW_HIDE);
                                    }
                                }
                                app.save_config();
                                if !show {
                                    crate::panel::ensure_z_order(app);
                                }
                                crate::panel::dlog(&format!(
                                    "settings: panel show gi={gi} -> {show}"
                                ));
                            }
                        }
                    }
                    x if (IDC_PTSHOW..IDC_PTSHOW + 100).contains(&x) => {
                        let gi = (x - IDC_PTSHOW) as usize;
                        if gi < app.groups.len() {
                            let on = get_check(hwnd, x);
                            if app.groups[gi].title_show != on {
                                app.groups[gi].title_show = on;
                                app.save_config();
                                app.place_groups();
                                app.render_all();
                                crate::panel::dlog(&format!("settings: panel title gi={gi} -> {on}"));
                            }
                        }
                    }
                    _ => {}
                }
            }
            // 非点击通知:下拉选择、输入变化 → 立即生效
            match id {
                IDC_GRID if code == CBN_SELCHANGE => {
                    if let Some(i) = combo_get(hwnd, IDC_GRID) {
                        if let Some(&g) = GRIDS.get(i) {
                            if app.settings.snap_grid != g {
                                app.settings.snap_grid = g;
                                app.save_config();
                            }
                        }
                    }
                }
                IDC_RADIUS | IDC_CGAP | IDC_RGAP if code == EN_CHANGE => {
                    // 输入停顿 400ms 后应用,避免每个按键都重排
                    let _ = SetTimer(Some(hwnd), TID_APPLY, 400, None);
                }
                x if (IDC_Z0..IDC_Z0 + 100).contains(&x) && code == CBN_SELCHANGE => {
                    let gi = (x - IDC_Z0) as usize;
                    if gi < app.groups.len() {
                        let top = combo_get(hwnd, x)
                            .map(|i| i == 1)
                            .unwrap_or(app.groups[gi].z_top);
                        if app.groups[gi].z_top != top {
                            app.groups[gi].z_top = top;
                            app.save_config();
                            crate::panel::ensure_z_order(app);
                        }
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TID_APPLY {
                let _ = KillTimer(Some(hwnd), TID_APPLY);
                if !app_ptr.is_null() {
                    apply_numbers(&mut *app_ptr, hwnd);
                }
                return LRESULT(0);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            // 注意:关闭设置不能退出整个程序;关闭前应用输入框里未到期的改动
            if !app_ptr.is_null() {
                let _ = KillTimer(Some(hwnd), TID_APPLY);
                apply_numbers(&mut *app_ptr, hwnd);
                (*app_ptr).settings_hwnd = HWND::default();
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
