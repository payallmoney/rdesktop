// 桌面 shell 交互:隐藏/恢复系统图标、枚举桌面、提取图标、打开、右键菜单
use std::path::{Path, PathBuf};

use windows::core::{BOOL, Interface, PCSTR, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, WPARAM, SIZE};
use windows::Win32::Graphics::Gdi::DeleteObject;
use windows::Win32::Graphics::Imaging::{
    IWICBitmap, IWICImagingFactory, WICBitmapUsePremultipliedAlpha,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    CMINVOKECOMMANDINFO, CSIDL_DRIVES, FOLDERID_Desktop, FOLDERID_PublicDesktop, IContextMenu,
    IShellItem, IShellItemImageFactory, KF_FLAG_DEFAULT, SHCreateItemFromIDList,
    SHCreateItemFromParsingName, SHDefExtractIconW, SHGetFileInfoW, SHGetFolderLocation,
    SHGetKnownFolderPath, SHParseDisplayName, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_PIDL,
    SHFILEINFOW, SIIGBF_ICONONLY, ShellExecuteW,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, DestroyIcon, DestroyMenu, HICON, MB_ICONERROR, MB_OK, MessageBoxW,
    SW_SHOWNORMAL, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
};
use windows::Win32::UI::WindowsAndMessaging::HMENU;
use windows::core::w;

use crate::app::{ws, Item};

// ================= 系统桌面图标(listview)隐藏 / 恢复 =================

/// 找到承载桌面图标的 SysListView32(Progman 或 WorkerW 下的 SHELLDLL_DefView)
pub fn find_desktop_listview() -> Option<HWND> {
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, FindWindowExW, GetClassNameW};
    let mut found: Option<HWND> = None;
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let mut buf = [0u16; 64];
        let n = unsafe { GetClassNameW(hwnd, &mut buf) };
        let cls = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        if cls == "Progman" || cls == "WorkerW" {
            if let Ok(defview) =
                unsafe { FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), None) }
            {
                if !defview.is_invalid() {
                    if let Ok(lv) = unsafe {
                        FindWindowExW(Some(defview), None, w!("SysListView32"), None)
                    } {
                        if !lv.is_invalid() {
                            unsafe { &mut *(lparam.0 as *mut Option<HWND>) }.get_or_insert(lv);
                            return BOOL(0);
                        }
                    }
                }
            }
        }
        BOOL(1)
    }
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut found as *mut _ as isize));
    }
    found
}

pub fn hide_listview(lv: HWND) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
        let _ = ShowWindow(lv, SW_HIDE);
    }
}

pub fn show_listview(lv: HWND) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOW};
        let _ = ShowWindow(lv, SW_SHOW);
    }
}

// ================= COM 初始化 =================

pub fn init_com() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

pub fn create_wic() -> windows::core::Result<IWICImagingFactory> {
    unsafe {
        CoCreateInstance(
            &windows::Win32::Graphics::Imaging::CLSID_WICImagingFactory,
            None,
            CLSCTX_INPROC_SERVER,
        )
    }
}

// ================= 枚举桌面 =================

fn known_folder(id: &windows::core::GUID) -> Option<PathBuf> {
    unsafe {
        let pw = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        if pw.is_null() {
            return None;
        }
        let mut len = 0usize;
        while *pw.0.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(pw.0, len);
        let s = String::from_utf16_lossy(slice);
        CoTaskMemFree(Some(pw.0 as *const core::ffi::c_void));
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    }
}

fn desktop_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = known_folder(&FOLDERID_Desktop) {
        v.push(p);
    }
    if let Some(p) = known_folder(&FOLDERID_PublicDesktop) {
        v.push(p);
    }
    v
}

/// 桌面目录 mtime 的最大值,用于检测新建/删除/改名
pub fn desktop_dirs_mtime() -> Option<std::time::SystemTime> {
    let mut max: Option<std::time::SystemTime> = None;
    for d in desktop_dirs() {
        if let Ok(md) = std::fs::metadata(&d) {
            if let Ok(t) = md.modified() {
                if max.map(|m| t > m).unwrap_or(true) {
                    max = Some(t);
                }
            }
        }
    }
    max
}

const SHORTCUT_EXTS: [&str; 3] = ["lnk", "url", "appref-ms"];

fn is_shortcut(path: &Path) -> bool {
    path.extension()
        .map(|e| {
            let e = e.to_string_lossy().to_lowercase();
            SHORTCUT_EXTS.contains(&e.as_str())
        })
        .unwrap_or(false)
}

/// 显示名:去掉 .lnk/.url 扩展
fn display_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let lower = name.to_lowercase();
    for ext in [".lnk", ".url"] {
        if lower.ends_with(ext) && name.len() > ext.len() {
            return name[..name.len() - ext.len()].to_string();
        }
    }
    name
}

pub fn enumerate_items() -> Vec<Item> {
    let mut items = Vec::new();
    for dir in desktop_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for ent in rd.flatten() {
            let path = ent.path();
            let name = match ent.file_name().into_string() {
                Ok(n) => n,
                Err(_) => continue,
            };
            if name.eq_ignore_ascii_case("desktop.ini") {
                continue;
            }
            let is_dir = ent.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let rule: u8 = if is_dir {
                0
            } else if is_shortcut(&path) {
                2
            } else {
                1
            };
            let key = path.to_string_lossy().to_string();
            if items.iter().any(|i: &Item| i.key == key) {
                continue;
            }
            items.push(Item {
                key,
                name: display_name(&path),
                rule,
                launch: None,
            });
        }
    }
    items
}

pub fn push_builtin_items(items: &mut Vec<Item>) {
    fn b(key: &str, name: &str, exe: &str, args: &str) -> Item {
        Item {
            key: key.to_string(),
            name: name.to_string(),
            rule: 3,
            launch: Some((exe.to_string(), args.to_string())),
        }
    }
    items.push(b(
        "@builtin:recycle",
        "回收站",
        "explorer.exe",
        "shell:RecycleBinFolder",
    ));
    items.push(b(
        "@builtin:thispc",
        "此电脑",
        "explorer.exe",
        "::{20D04FE0-3C15-11D2-8E5F-00C04FB68F76}",
    ));
    items.push(b(
        "@builtin:network",
        "网络",
        "explorer.exe",
        "shell:NetworkPlacesFolder",
    ));
    items.push(b("@builtin:control", "控制面板", "control.exe", ""));
    items.push(b("@builtin:settings", "设置", "ms-settings:", ""));
}

// ================= 图标提取 =================

enum IconSrc {
    Pidl(&'static str),
    Path(String),
}

fn builtin_icon_src(key: &str) -> Option<IconSrc> {
    match key {
        "@builtin:recycle" => Some(IconSrc::Pidl("shell:RecycleBinFolder")),
        "@builtin:thispc" => Some(IconSrc::Pidl("::{20D04FE0-3C15-11D2-8E5F-00C04FB68F76}")),
        "@builtin:network" => Some(IconSrc::Pidl("shell:NetworkPlacesFolder")),
        "@builtin:control" => Some(IconSrc::Pidl("shell:ControlPanelFolder")),
        "@builtin:settings" => Some(IconSrc::Path(
            "C:\\Windows\\ImmersiveControlPanel\\SystemSettings.exe".into(),
        )),
        _ => None,
    }
}

fn icon_from_hbitmap(wic: &IWICImagingFactory, hbmp: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<IWICBitmap> {
    unsafe {
        let r = wic
            .CreateBitmapFromHBITMAP(hbmp, Default::default(), WICBitmapUsePremultipliedAlpha)
            .ok();
        let _ = DeleteObject(hbmp.into());
        r
    }
}

fn icon_from_hicon(wic: &IWICImagingFactory, hicon: HICON) -> Option<IWICBitmap> {
    unsafe {
        let r = wic.CreateBitmapFromHICON(hicon).ok();
        let _ = DestroyIcon(hicon);
        r
    }
}

fn icon_via_item_factory(wic: &IWICImagingFactory, src: &IconSrc) -> Option<IWICBitmap> {
    unsafe {
        let item: Option<IShellItem> = match src {
            IconSrc::Pidl(s) => {
                let w = ws(s);
                let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
                if SHParseDisplayName(PCWSTR(w.as_ptr()), None, &mut pidl, 0, None).is_err()
                    || pidl.is_null()
                {
                    return None;
                }
                let r = SHCreateItemFromIDList(pidl).ok();
                windows::Win32::UI::Shell::ILFree(Some(pidl));
                r
            }
            IconSrc::Path(p) => {
                let w = ws(p);
                SHCreateItemFromParsingName(PCWSTR(w.as_ptr()), None).ok()
            }
        };
        let item = item?;
        let factory: IShellItemImageFactory = item.cast().ok()?;
        let hbmp = factory
            .GetImage(SIZE { cx: 96, cy: 96 }, SIIGBF_ICONONLY)
            .ok()?;
        icon_from_hbitmap(wic, hbmp)
    }
}

/// PIDL → 图标:IShellItemImageFactory::GetImage 主路径,SHGetFileInfo(SHGFI_PIDL) 兜底。
/// 消费(释放)pidl。
fn icon_from_pidl(wic: &IWICImagingFactory, pidl: *mut ITEMIDLIST) -> Option<IWICBitmap> {
    unsafe {
        crate::panel::dlog("icon_parse pidl ok");
        let mut out: Option<IWICBitmap> = None;
        if let Ok(item) = SHCreateItemFromIDList::<IShellItem>(pidl) {
            crate::panel::dlog("icon_parse item ok");
            if let Ok(f) = item.cast::<IShellItemImageFactory>() {
                match f.GetImage(SIZE { cx: 96, cy: 96 }, SIIGBF_ICONONLY) {
                    Ok(hbmp) => {
                        crate::panel::dlog("icon_parse GetImage ok");
                        out = icon_from_hbitmap(wic, hbmp);
                    }
                    Err(e) => crate::panel::dlog(&format!("icon_parse GetImage FAIL {e}")),
                }
            } else {
                crate::panel::dlog("icon_parse cast factory FAIL");
            }
        } else {
            crate::panel::dlog("icon_parse SHCreateItemFromIDList FAIL");
        }
        if out.is_none() {
            let mut sfi = SHFILEINFOW::default();
            let r = SHGetFileInfoW(
                PCWSTR(pidl as *const u16),
                windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
                Some(&mut sfi),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_PIDL | SHGFI_ICON | SHGFI_LARGEICON,
            );
            crate::panel::dlog(&format!("icon_parse SHGetFileInfo r={}", r as isize));
            if r != 0 && !sfi.hIcon.is_invalid() {
                out = icon_from_hicon(wic, sfi.hIcon);
            }
        }
        windows::Win32::UI::Shell::ILFree(Some(pidl));
        out
    }
}

/// CLSID/shell 解析串取图标。此电脑的 "::{CLSID}" 字符串形式两条经典 API 均不接受
/// (SHGetFileInfo r=0,SHParseDisplayName E_INVALIDARG),改走权威 API
/// SHGetFolderLocation(CSIDL_DRIVES) 直取 My Computer PIDL;其余串走 SHParseDisplayName。
fn icon_from_parse(wic: &IWICImagingFactory, parse: &str) -> Option<IWICBitmap> {
    unsafe {
        crate::panel::dlog(&format!("icon_parse begin {parse}"));
        if parse.contains("20D04FE0-3C15-11D2-8E5F-00C04FB68F76") {
            let got: Option<IWICBitmap> = match SHGetFolderLocation(None, CSIDL_DRIVES as i32, None, 0)
            {
                Ok(pidl) if !pidl.is_null() => {
                    crate::panel::dlog("icon_parse CSIDL_DRIVES pidl ok");
                    icon_from_pidl(wic, pidl)
                }
                Ok(_) => {
                    crate::panel::dlog("icon_parse CSIDL_DRIVES pidl NULL");
                    None
                }
                Err(e) => {
                    crate::panel::dlog(&format!("icon_parse CSIDL_DRIVES FAIL {e}"));
                    None
                }
            };
            if got.is_some() {
                return got;
            }
            crate::panel::dlog("icon_parse CSIDL_DRIVES miss, fallback string parse");
        }
        let w = ws(parse);
        let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        if let Err(e) = SHParseDisplayName(PCWSTR(w.as_ptr()), None, &mut pidl, 0, None) {
            crate::panel::dlog(&format!("icon_parse SHParseDisplayName FAIL {e}"));
            return None;
        }
        if pidl.is_null() {
            crate::panel::dlog("icon_parse pidl NULL");
            return None;
        }
        icon_from_pidl(wic, pidl)
    }
}

fn icon_via_def_extract(wic: &IWICImagingFactory, path: &str) -> Option<IWICBitmap> {
    unsafe {
        let w = ws(path);
        let mut large = HICON::default();
        let mut small = HICON::default();
        let hr = SHDefExtractIconW(
            PCWSTR(w.as_ptr()),
            0,
            0,
            Some(&mut large),
            Some(&mut small),
            1,
        );
        if !small.is_invalid() {
            let _ = DestroyIcon(small);
        }
        if hr.is_ok() && !large.is_invalid() {
            return icon_from_hicon(wic, large);
        }
        if !large.is_invalid() {
            let _ = DestroyIcon(large);
        }
        None
    }
}

fn icon_via_shgetfileinfo(wic: &IWICImagingFactory, path: &str) -> Option<IWICBitmap> {
    unsafe {
        let w = ws(path);
        let mut sfi = SHFILEINFOW::default();
        let r = SHGetFileInfoW(
            PCWSTR(w.as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
            Some(&mut sfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if r != 0 && !sfi.hIcon.is_invalid() {
            return icon_from_hicon(wic, sfi.hIcon);
        }
        None
    }
}

pub fn load_icon(wic: &IWICImagingFactory, key: &str) -> Option<IWICBitmap> {
    if let Some(src) = builtin_icon_src(key) {
        match &src {
            IconSrc::Pidl(ps) => {
                if let Some(i) = icon_from_parse(wic, ps) {
                    return Some(i);
                }
            }
            IconSrc::Path(p) => {
                if let Some(i) = icon_via_item_factory(wic, &src) {
                    return Some(i);
                }
                if let Some(i) = icon_via_def_extract(wic, p) {
                    return Some(i);
                }
            }
        }
        return None;
    }
    if let Some(i) = icon_via_item_factory(wic, &IconSrc::Path(key.to_string())) {
        return Some(i);
    }
    if let Some(i) = icon_via_def_extract(wic, key) {
        return Some(i);
    }
    icon_via_shgetfileinfo(wic, key)
}

// ================= 打开 =================

pub fn open_item(hwnd: Option<HWND>, item: &crate::app::Item) {
    crate::panel::dlog(&format!("open begin key={}", item.key));
    unsafe {
        let (file, args) = match &item.launch {
            Some((f, a)) => (ws(f), ws(a)),
            None => (ws(&item.key), Vec::new()),
        };
        // 空 Vec 的 as_ptr() 是悬垂指针,绝不能直接传给 ShellExecuteW(lstrlenW 会崩)!
        let params = if args.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(args.as_ptr())
        };
        let _ = ShellExecuteW(
            hwnd,
            w!("open"),
            PCWSTR(file.as_ptr()),
            params,
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
        let _: Option<HINSTANCE> = None;
    }
    crate::panel::dlog("open done");
}

// ================= 右键菜单(shell 完整菜单) =================

fn force_foreground(hwnd: HWND) {
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            keybd_event, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_MENU,
        };
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_EXTENDEDKEY, 0);
        let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
    }
}

/// 条目的 PIDL(原生右键菜单入口):
/// - 文件/文件夹/快捷方式:完整路径
/// - 内置项:解析串;此电脑的 "::{CLSID}" 串解析不了 → SHGetFolderLocation(CSIDL_DRIVES)
fn item_pidl(item: &crate::app::Item) -> Option<*mut ITEMIDLIST> {
    unsafe {
        if !item.is_builtin() {
            let w = ws(&item.key);
            let p = windows::Win32::UI::Shell::ILCreateFromPathW(PCWSTR(w.as_ptr()));
            return if p.is_null() { None } else { Some(p) };
        }
        match builtin_icon_src(&item.key)? {
            IconSrc::Pidl(s) => {
                if s.contains("20D04FE0-3C15-11D2-8E5F-00C04FB68F76") {
                    SHGetFolderLocation(None, CSIDL_DRIVES as i32, None, 0).ok()
                } else {
                    let w = ws(&s);
                    let mut p: *mut ITEMIDLIST = std::ptr::null_mut();
                    if SHParseDisplayName(PCWSTR(w.as_ptr()), None, &mut p, 0, None).is_err()
                        || p.is_null()
                    {
                        None
                    } else {
                        Some(p)
                    }
                }
            }
            IconSrc::Path(pth) => {
                let w = ws(&pth);
                let p = windows::Win32::UI::Shell::ILCreateFromPathW(PCWSTR(w.as_ptr()));
                if p.is_null() {
                    None
                } else {
                    Some(p)
                }
            }
        }
    }
}

/// 条目右键:与资源管理器一致的 shell 原生菜单(IContextMenu 全量 verb,
/// 含第三方扩展、发送到、属性等);菜单被取消也算已展示,不再追加兜底菜单。
pub fn show_context_menu(hwnd: HWND, item: &crate::app::Item, x: i32, y: i32, cmd_open: usize) {
    unsafe {
        let menu: HMENU = match CreatePopupMenu() {
            Ok(m) => m,
            Err(_) => return,
        };
        let mut shown = false;
        let mut src = "none";

        if let Some(pidl) = item_pidl(item) {
            if let Ok(desk) = windows::Win32::UI::Shell::SHGetDesktopFolder() {
                let arr = [pidl as *const ITEMIDLIST];
                let cm: Option<IContextMenu> = desk.GetUIObjectOf(hwnd, &arr, None).ok();
                if let Some(cm) = cm {
                    let hr = cm.QueryContextMenu(menu, 0, 1, 0x7FFF, 0);
                    if hr.is_ok() {
                        force_foreground(hwnd);
                        let cmd = TrackPopupMenu(
                            menu,
                            TPM_RETURNCMD | TPM_RIGHTBUTTON,
                            x,
                            y,
                            Some(0),
                            hwnd,
                            None,
                        )
                        .0;
                        let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                            Some(hwnd),
                            WM_NULL,
                            WPARAM(0),
                            LPARAM(0),
                        );
                        shown = true;
                        src = "shell";
                        if cmd > 0 {
                            let mut cmi = CMINVOKECOMMANDINFO {
                                cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                                fMask: 0,
                                hwnd,
                                lpVerb: PCSTR::null(),
                                lpParameters: PCSTR::null(),
                                lpDirectory: PCSTR::null(),
                                nShow: SW_SHOWNORMAL.0,
                                dwHotKey: 0,
                                hIcon: Default::default(),
                            };
                            // lpVerb = MAKEINTRESOURCEA(cmd - idCmdFirst),idCmdFirst=1
                            cmi.lpVerb = PCSTR(((cmd - 1) as usize) as *const u8);
                            let _ = cm.InvokeCommand(&cmi);
                        }
                    } else {
                        crate::panel::dlog(&format!("ctx query FAIL {hr}"));
                    }
                } else {
                    crate::panel::dlog("ctx GetUIObjectOf FAIL");
                }
            }
            windows::Win32::UI::Shell::ILFree(Some(pidl));
        } else {
            crate::panel::dlog("ctx no pidl");
        }

        if !shown {
            // 简化菜单:打开
            let _ = windows::Win32::UI::WindowsAndMessaging::AppendMenuW(
                menu,
                windows::Win32::UI::WindowsAndMessaging::MF_STRING,
                cmd_open,
                w!("打开(&O)"),
            );
            force_foreground(hwnd);
            let cmd = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                x,
                y,
                Some(0),
                hwnd,
                None,
            )
            .0;
            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(hwnd),
                WM_NULL,
                WPARAM(0),
                LPARAM(0),
            );
            if cmd == cmd_open as i32 {
                open_item(Some(hwnd), item);
            }
            src = "simple";
        }

        crate::panel::dlog(&format!(
            "ctx menu src={src} builtin={} key={}",
            item.is_builtin(),
            item.key
        ));
        let _ = DestroyMenu(menu);
    }
}

pub fn show_error(msg: &str) {
    unsafe {
        let t = ws(msg);
        let c = ws("rdesktop");
        MessageBoxW(None, PCWSTR(t.as_ptr()), PCWSTR(c.as_ptr()), MB_OK | MB_ICONERROR);
    }
}
