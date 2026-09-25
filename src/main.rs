// rdesktop —— 桌面分组管理工具
// 隐藏系统桌面图标,用四块“半透明圆角面板”重新呈现:文件夹 / 文件 / 快捷方式 / 其他快捷功能。
// 支持在面板间拖拽图标、双击打开、右键 shell 菜单、拖动面板、托盘管理。
#![windows_subsystem = "windows"]

mod app;
mod desktop;
mod panel;
mod lang;
mod regstore;
mod render;
mod settings;

use std::sync::atomic::{AtomicIsize, Ordering};

use app::{ws, App, TIMER_ZORDER};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, SetTimer, TranslateMessage, MSG,
};

/// 桌面图标列表窗口句柄(崩溃时兜底恢复)
static DESKTOP_LV: AtomicIsize = AtomicIsize::new(0);

fn restore_desktop_icons() {
    let h = DESKTOP_LV.swap(0, Ordering::SeqCst);
    if h != 0 {
        desktop::show_listview(HWND(h as *mut core::ffi::c_void));
    }
}

fn dlog(msg: &str) {
    use std::io::Write;
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true)
        .open(std::env::temp_dir().join("rdesktop.log")) {
        let _ = writeln!(f, "[{}] {}", ms, msg);
    }
}

fn main() {
    crate::lang::load_persisted();
    // 崩溃兜底:无论如何都把系统桌面图标放回来,并记录 panic 位置
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true)
            .open(std::env::temp_dir().join("rdesktop.log")) {
            let _ = writeln!(f, "PANIC: {}", info);
        }
        restore_desktop_icons();
    }));

    if let Err(e) = run() {
        desktop::show_error(&e);
    }
    restore_desktop_icons();
}

fn run() -> Result<(), String> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    // 单实例
    let _mutex = unsafe {
        let name = ws("Local\\rdesktop-single-instance");
        match CreateMutexW(None, false, PCWSTR(name.as_ptr())) {
            Ok(m) => {
                if windows::Win32::Foundation::GetLastError().0 == 183 {
                    return Err("rdesktop 已在运行。".into());
                }
                m
            }
            Err(e) => return Err(format!("创建互斥量失败: {e}")),
        }
    };

    desktop::init_com();

    // 隐藏系统桌面图标(记住句柄以便恢复)
    if let Some(lv) = desktop::find_desktop_listview() {
        desktop::hide_listview(lv);
        DESKTOP_LV.store(lv.0 as isize, Ordering::SeqCst);
    }

    // 应用状态
    let mut app = Box::new(App::new()?);

    // 窗口类 + 面板窗口
    let hinst = unsafe {
        let hm = GetModuleHandleW(None).map_err(|e| format!("获取模块句柄失败: {e}"))?;
        windows::Win32::Foundation::HINSTANCE(hm.0)
    };
    if !unsafe { panel::register_panel_class(hinst) } {
        return Err("注册窗口类失败。".into());
    }
    if !unsafe { settings::register_class(hinst) } {
        return Err("注册设置窗口类失败。".into());
    }
    unsafe { panel::create_windows(&mut app, hinst) };

    // 首次呈现
    dlog("stage: first render");
    app.render_all();
    settings::apply_frosted(&mut app);
    dlog("stage: first render done");
    app.last_mtime = desktop::desktop_dirs_mtime();
    panel::ensure_z_order(&app);
    panel::tray_add(&mut app);

    // 事件驱动 z 维护:前台切换/最小化瞬间纠正(2秒定时器仅兜底)
    panel::install_z_hooks();

    // 定时器:z序维护 + 桌面变化自检
    unsafe {
        let ret = SetTimer(Some(app.groups[0].hwnd), TIMER_ZORDER, 2000, None);
        dlog(&format!("SetTimer ret={ret}"));
    }

    // 消息循环
    let mut msg = MSG::default();
    loop {
        let code = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
        if code <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    dlog("stage: message loop exit");
    app.exiting = true; // 后续 DestroyWindow 属于退出流程(WM_DESTROY 才 PostQuitMessage)
    // 清理:先删托盘,再销毁窗口(图标恢复由 main 收尾)
    panel::tray_delete(&app);
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, KillTimer};
        for gi in 0..app.groups.len() {
            let hw = app.groups[gi].hwnd;
            if !hw.is_invalid() {
                let _ = KillTimer(Some(hw), TIMER_ZORDER);
                let _ = DestroyWindow(hw);
            }
        }
        if !app.ghost.is_invalid() {
            let _ = DestroyWindow(app.ghost);
        }
    }
    Ok(())
}
