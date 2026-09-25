// 面板窗口:注册类、窗口过程(命中/选中/拖拽/右键/菜单)、z序管理、托盘
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{
    HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, TRUE, WPARAM,
};
use windows::Win32::UI::Shell::{
    NIIF_INFO, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use std::sync::atomic::AtomicIsize;
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::UI::Accessibility::{SetWinEventHook, WINEVENTPROC};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::Shell::Shell_NotifyIconW;
use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::{
    ws, App, IDM_AUTORUN, IDM_EXIT, IDM_OPEN, IDM_REFRESH, IDM_SETTINGS, IDM_TOGGLE_GROUPS,
    IDM_TOGGLE_ORIG, BOTTOM_PAD, ICON_SZ, ICON_TOP, MARGIN, PAD, TIMER_ZORDER,
    HOVER_TITLE, IDM_LOCK, IDM_LANG, IDM_PHIDE, IDM_PDEL, IDM_PALGN, IDM_PNEW, IDM_PRENAME, IDM_PSHOW, IDM_PTITLE,
    WM_OPEN_SETTINGS, WM_TRAY, WM_Z_PAUSE, WM_Z_REZ, DRAG_THRESHOLD,
};
use crate::desktop;

pub const PANEL_CLASS: PCWSTR = w!("RdGroupPanel");
const TIMER_SETTLE_ID: usize = 7;

/// WinEvent 投递目标:任一面板句柄(同线程消息队列)
static Z_SINK: AtomicIsize = AtomicIsize::new(0);

/// 锚点黑名单:SetWindowPos(…, after=该窗) 返回过 E_ACCESSDENIED 的窗
/// (如某些受保护/提权进程的窗口)。不再拿它当插入锚点,walk 也不再因它判 displaced,
/// 否则每次 ensure 都会失败重试、永不收敛。
static ANCHOR_DENY: std::sync::Mutex<Vec<isize>> = std::sync::Mutex::new(Vec::new());

fn anchor_denied(hwnd: HWND) -> bool {
    ANCHOR_DENY
        .lock()
        .map(|s| s.contains(&(hwnd.0 as isize)))
        .unwrap_or(false)
}

fn anchor_mark_denied(hwnd: HWND) {
    if let Ok(mut s) = ANCHOR_DENY.lock() {
        let v = hwnd.0 as isize;
        if !s.contains(&v) {
            s.push(v);
        }
    }
}

unsafe extern "system" fn winevent_cb(
    _hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    idobject: i32,
    _idchild: i32,
    _thread: u32,
    _time: u32,
) {
    use windows::Win32::UI::WindowsAndMessaging::{
        EVENT_OBJECT_HIDE, EVENT_OBJECT_REORDER, EVENT_OBJECT_SHOW, EVENT_SYSTEM_FOREGROUND,
        EVENT_SYSTEM_MINIMIZESTART,
    };
    let relevant = event == EVENT_SYSTEM_FOREGROUND
        || event == EVENT_SYSTEM_MINIMIZESTART
        || event == EVENT_OBJECT_SHOW
        || event == EVENT_OBJECT_HIDE
        || event == EVENT_OBJECT_REORDER;
    if !relevant {
        return;
    }
    if idobject != 0 {
        // 子对象级事件与桌面 z 序无关:csrss 等进程的 CLIENT 子窗会不停
        // 重排(idobject=-4),会把纠偏循环淹死。仅放行桌面窗自身
        // (壳在 Win+D 抬壁纸窗时可能带非0对象ID)。
        if event == EVENT_OBJECT_REORDER || event == EVENT_OBJECT_SHOW {
            let cls = class_name(hwnd);
            if cls != "Progman" && cls != "WorkerW" {
                return;
            }
        } else {
            return;
        }
    }
    dlog(&format!("evt obj={} event={} hwnd={}", idobject, event, hwnd.0 as isize));
    let sink = Z_SINK.load(std::sync::atomic::Ordering::SeqCst);
    if sink != 0 {
        let _ = PostMessageW(
            Some(HWND(sink as *mut core::ffi::c_void)),
            WM_Z_REZ,
            WPARAM(0),
            LPARAM(0),
        );
    }
}

/// 安装 WinEvent 钩子:前台切换 / 最小化 瞬间触发 z 序纠正(2秒定时器仅作兜底)
pub fn install_z_hooks() {
    unsafe {
        let cb: WINEVENTPROC = Some(winevent_cb);
        // 覆盖 EVENT_SYSTEM_FOREGROUND(3) .. EVENT_SYSTEM_MINIMIZESTART(22)
        let _ret = SetWinEventHook(
            windows::Win32::UI::WindowsAndMessaging::EVENT_SYSTEM_FOREGROUND,
            windows::Win32::UI::WindowsAndMessaging::EVENT_SYSTEM_MINIMIZESTART,
            None,
            cb,
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::WINEVENT_OUTOFCONTEXT,
        );
        // 第二个钩子:窗口级 z 序重排(如 Win+D 时 shell 把壁纸窗抬到顶部)
        let _ret2 = SetWinEventHook(
            windows::Win32::UI::WindowsAndMessaging::EVENT_OBJECT_SHOW,
            windows::Win32::UI::WindowsAndMessaging::EVENT_OBJECT_REORDER,
            None,
            cb,
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::WINEVENT_OUTOFCONTEXT,
        );
        dlog(&format!(
            "z hooks installed ret={} reorder={}",
            !_ret.is_invalid(),
            !_ret2.is_invalid()
        ));
    }
}

/// 工具图标:exe 资源 ID=1(build.rs 嵌入 assets/rdesktop.ico),失败回退系统图标
pub fn tool_icon() -> HICON {
    unsafe {
        if let Ok(hm) = GetModuleHandleW(None) {
            if let Ok(ic) = LoadIconW(Some(HINSTANCE(hm.0)), PCWSTR(1usize as *const u16)) {
                return ic;
            }
        }
        LoadIconW(None, IDI_APPLICATION).unwrap_or_default()
    }
}

// ================= 窗口类 =================

pub unsafe fn register_panel_class(hinst: windows::Win32::Foundation::HINSTANCE) -> bool {
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_DBLCLKS,
        lpfnWndProc: Some(panel_wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: tool_icon(),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        hbrBackground: Default::default(),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: PANEL_CLASS,
        hIconSm: tool_icon(),
    };
    RegisterClassExW(&wc) != 0
}

// ================= 创建窗口 =================

/// 为指定面板创建窗口(启动批量创建与"新建面板"共用)
pub fn create_panel_window(app: &mut App, gi: usize, hinst: windows::Win32::Foundation::HINSTANCE) {
    if gi >= app.groups.len() {
        return;
    }
    unsafe {
        let ptr = app as *mut App;
        let title = ws(&app.groups[gi].title);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            PANEL_CLASS,
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            0,
            0,
            100,
            100,
            None,
            None,
            Some(hinst),
            Some(ptr as *const core::ffi::c_void),
        );
        match hwnd {
            Ok(h) => {
                app.groups[gi].hwnd = h;
                if app.groups[gi].hidden {
                    let _ = ShowWindow(h, SW_HIDE);
                } else {
                    let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
                }
            }
            Err(_) => app.groups[gi].hwnd = HWND::default(),
        }
    }
}

pub unsafe fn create_windows(app: &mut App, hinst: windows::Win32::Foundation::HINSTANCE) {
    let n_panels = app.groups.len();
    for gi in 0..n_panels {
        create_panel_window(app, gi, hinst);
    }
    // 幽灵窗口(拖拽用),置顶 + 对鼠标透明
    let ptr = app as *mut App;
    let ghost = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_TOPMOST,
        PANEL_CLASS,
        w!("ghost"),
        WS_POPUP,
        0,
        0,
        10,
        10,
        None,
        None,
        Some(hinst),
        Some(ptr as *const core::ffi::c_void),
    );
    app.ghost = ghost.unwrap_or_default();
    // WinEvent 投递用的 sink:取第一块面板
    if let Some(g0) = app.groups.first() {
        if !g0.hwnd.is_invalid() {
            Z_SINK.store(g0.hwnd.0 as isize, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

// ================= z 序:面板永远位于所有应用窗口之下、桌面之上 =================

fn class_name(hwnd: HWND) -> String {
    unsafe {
        let mut buf = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}

/// 一次 EnumWindows(自顶向下)得到的锚点 + 面板簇状态
pub struct ZAnchor {
    pub shell: Option<HWND>,
    pub prev: Option<HWND>,
    /// prev 之下、shell 之上按 z 序实际出现的低层面板 gi(自顶向下)
    pub cluster: Vec<usize>,
    /// 面板簇下方(与 shell 之间)又出现了普通窗口 —— 簇未贴住锚点
    pub displaced: bool,
    pub prev_n: i32,
    pub first_panel_n: i32,
    pub shell_n: i32,
    /// 自顶向下最近访问的普通窗口(至多16个)——插入锚点回退链:
    /// prev 被系统拒绝(E_ACCESSDENIED)时,依次尝试更上方的窗口
    pub chain: Vec<HWND>,
}

/// 自顶向下找:桌面窗口(Progman/WorkerW)、其上方紧邻的普通窗口、以及夹在中间的面板簇
fn walk_anchor(app: &App) -> ZAnchor {
    unsafe {
        struct ZWalk<'a> {
            app: &'a App,
            prev: Option<HWND>,
            shell: Option<HWND>,
            cluster: Vec<usize>,
            displaced: bool,
            seen_panel: bool,
            n: i32,           // EnumWindows 全局序号(含跳过的窗口)
            prev_n: i32,
            first_panel_n: i32,
            shell_n: i32,
            chain: Vec<HWND>,
        }
        unsafe extern "system" fn z_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let st = &mut *(lparam.0 as *mut ZWalk);
            st.n += 1;
            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }
            if st.app.ghost == hwnd {
                return BOOL(1);
            }
            if let Some(gi) = st.app.groups.iter().position(|g| g.hwnd == hwnd) {
                if !st.app.groups[gi].z_top {
                    if !st.seen_panel {
                        st.first_panel_n = st.n;
                    }
                    st.cluster.push(gi);
                    st.seen_panel = true;
                }
                return BOOL(1); // 最高层带面板独立维护,不参与簇判断
            }
            let cls = class_name(hwnd);
            if cls == "Progman" || cls == "WorkerW" {
                st.shell = Some(hwnd);
                st.shell_n = st.n;
                return BOOL(0); // 到桌面层,停止
            }
            if st.seen_panel && !anchor_denied(hwnd) {
                st.displaced = true; // 面板簇和桌面之间夹进了普通窗口(锚点黑名单窗除外)
            }
            st.prev = Some(hwnd);
            st.prev_n = st.n;
            st.chain.push(hwnd);
            if st.chain.len() > 16 {
                st.chain.remove(0);
            }
            BOOL(1)
        }
        let mut walk = ZWalk {
            app,
            prev: None,
            shell: None,
            cluster: Vec::new(),
            displaced: false,
            seen_panel: false,
            n: 0,
            chain: Vec::new(),
            prev_n: -1,
            first_panel_n: -1,
            shell_n: -1,
        };
        let _ = EnumWindows(Some(z_cb), LPARAM(&mut walk as *mut ZWalk as isize));
        ZAnchor {
            shell: walk.shell,
            prev: walk.prev,
            cluster: walk.cluster,
            displaced: walk.displaced,
            prev_n: walk.prev_n,
            first_panel_n: walk.first_panel_n,
            shell_n: walk.shell_n,
            chain: walk.chain,
        }
    }
}

/// 兼容旧签名:桌面窗口与其上方紧邻的普通窗口
pub fn compute_anchor(app: &App) -> (Option<HWND>, Option<HWND>) {
    let a = walk_anchor(app);
    (a.shell, a.prev)
}

/// 边缘缩放热区(客户区坐标,落在阴影带内):返回 HT* 作为区域标识,0=非热区
fn edge_zone(gi: usize, app: &App, x: i32, y: i32) -> u32 {
    if app.locked {
        return 0;
    }
    let g = &app.groups[gi];
    let cw = g.panel_w + 2 * MARGIN;
    let chh = g.panel_h + 2 * MARGIN;
    const EDGE: i32 = 9;
    if x < -3 || y < -3 || x > cw + 3 || y > chh + 3 {
        return 0;
    }
    // 热区贴可见圆角边缘:从窗口最外缘一直覆盖到面板边框再往内 EDGE 像素
    // (对齐到 MARGIN 处的可见轮廓,原实现落在阴影带里难以发现)。
    // 标题隐藏时顶部只保留阴影带作缩放热区,把细拖动条(14px)让给移动。
    let on_l = x <= MARGIN + EDGE;
    let on_r = x >= cw - MARGIN - EDGE;
    let on_t = if app.settings.show_title {
        y <= MARGIN + EDGE
    } else {
        y <= EDGE
    };
    let on_b = y >= chh - MARGIN - EDGE;
    if !(on_l || on_r || on_t || on_b) {
        return 0;
    }
    if on_l && on_t {
        HTTOPLEFT
    } else if on_r && on_t {
        HTTOPRIGHT
    } else if on_l && on_b {
        HTBOTTOMLEFT
    } else if on_r && on_b {
        HTBOTTOMRIGHT
    } else if on_l {
        HTLEFT
    } else if on_r {
        HTRIGHT
    } else if on_t {
        HTTOP
    } else {
        HTBOTTOM
    }
}

fn zone_cursor(zone: u32) -> windows::core::PCWSTR {
    match zone {
        x if x == HTLEFT || x == HTRIGHT => IDC_SIZEWE,
        x if x == HTTOP || x == HTBOTTOM => IDC_SIZENS,
        x if x == HTTOPLEFT || x == HTBOTTOMRIGHT => IDC_SIZENWSE,
        _ => IDC_SIZENESW,
    }
}

/// 手动缩放主逻辑:屏幕坐标增量(与真实/投递消息同一路径)
fn do_resize(app: &mut App, hwnd: HWND, p: POINT) {
    let Some(mut rs) = app.rs.take() else { return };
    let (gi, zone) = (rs.gi, rs.zone);
    // 增量法,统一屏幕坐标:左/上边缘缩放时窗口原点在光标下移动,
    // 客户区坐标混帧会抖动/回弹(见 lparam_screen 注释)
    let sp = {
        let mut s = p;
        unsafe {
            let _ = ClientToScreen(hwnd, &mut s);
        }
        s
    };
    let dx = sp.x - rs.start_px;
    let dy = sp.y - rs.start_py;
    rs.start_px = sp.x;
    rs.start_py = sp.y;
    let min_w = (2 * PAD + app.cell_w()) + 2 * MARGIN;
    let min_h = (app.grid_top(rs.gi) + app.cell_h() + BOTTOM_PAD) + 2 * MARGIN;
    let (l0, t0, r0, b0) = (rs.win_l, rs.win_t, rs.win_r, rs.win_b);
    let (mut l, mut t, mut r, mut b) = (l0, t0, r0, b0);
    if zone == HTLEFT || zone == HTTOPLEFT || zone == HTBOTTOMLEFT {
        l = (l0 + dx).min(r - min_w);
    }
    if zone == HTRIGHT || zone == HTTOPRIGHT || zone == HTBOTTOMRIGHT {
        r = (r0 + dx).max(l + min_w);
    }
    if zone == HTTOP || zone == HTTOPLEFT || zone == HTTOPRIGHT {
        t = (t0 + dy).min(b - min_h);
    }
    if zone == HTBOTTOM || zone == HTBOTTOMLEFT || zone == HTBOTTOMRIGHT {
        b = (b0 + dy).max(t + min_h);
    }
    let (w, h) = (r - l, b - t);
    unsafe {
        let _ = SetWindowPos(hwnd, None, l, t, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
    }
    {
        let g = &mut app.groups[gi];
        g.gx = l + MARGIN;
        g.gy = t + MARGIN;
        g.user_size = true;
        g.manual_w = w - 2 * MARGIN;
        g.manual_h = h - 2 * MARGIN;
    }
    rs.win_l = l;
    rs.win_t = t;
    rs.win_r = r;
    rs.win_b = b;
    app.rs = Some(rs);
    app.render_panel(gi);
}

/// 启动/续期 settle 连续校正窗(150ms ×12):shell 的 z 洗牌(含壁纸窗跳变)
/// 往往在首个事件之后才完成,需要在事件流静默后继续校正一段窗口
fn arm_settle(app: &mut App) {
    if !app.groups_visible {
        return;
    }
    if let Some(h) = app.groups.first().map(|g| g.hwnd) {
        if !h.is_invalid() {
            app.settle_left = 12;
            unsafe {
                let _ = SetTimer(Some(h), TIMER_SETTLE_ID, 150, None);
            }
        }
    }
}

pub fn ensure_z_order(app: &App) {
    unsafe {
        // 一次 EnumWindows 得锚点 + 簇状态;簇已正确时完全不动 —— 幂等是关键:
        // 以前每次重跑“倒序插到锚点下”都会挪动4个面板,触发 REORDER → WM_Z_REZ
        // → 再 ensure,形成永不收敛的自激循环,真实 Win+D 事件被防抖丢弃。
        let a = walk_anchor(app);
        // 已隐藏面板:窗口不可见、不参与簇期望(否则 cluster 永远对不上 → 空转重建)
        let low: Vec<usize> = (0..app.groups.len())
            .filter(|&gi| {
                !app.groups[gi].z_top && !app.groups[gi].hidden && !app.groups[gi].hwnd.is_invalid()
            })
            .collect();
        // 期望叠放序:用户偏好(点标题提层)优先,未记录的按组号补在末尾
        let want = app.want_order(&low);
        let cluster_ok =
            a.shell.is_some() && !a.displaced && a.cluster == want;
        let mut acted = false;
        if !cluster_ok {
            let (pv, pc, pt) = match a.prev {
                Some(p) => {
                    let ex = GetWindowLongPtrW(p, GWL_EXSTYLE);
                    (
                        format!("0x{:x}", p.0 as usize),
                        class_name(p),
                        ex & (WS_EX_TOPMOST.0 as isize) != 0,
                    )
                }
                None => ("none".into(), "-".into(), false),
            };
            dlog(&format!(
                "EN disp={} want={:?} clu={:?} prev={pv}({pc}) prev_top={pt} pn={} fn={} sn={}",
                a.displaced, want, a.cluster, a.prev_n, a.first_panel_n, a.shell_n
            ));
        }
        // 顶层带:需要置顶的提升;被外部置顶的低层面板取消置顶(位置统一在下方处理)
        for gi in 0..app.groups.len() {
            let hw = app.groups[gi].hwnd;
            if hw.is_invalid() || app.groups[gi].hidden {
                continue;
            }
            let ex = GetWindowLongPtrW(hw, GWL_EXSTYLE);
            let is_top = ex & (WS_EX_TOPMOST.0 as isize) != 0;
            if app.groups[gi].z_top {
                if !is_top {
                    let r = SetWindowPos(
                        hw,
                        Some(HWND_TOPMOST),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                    match r {
                        Ok(_) => {
                            acted = true;
                            dlog(&format!("promote gi={gi} OK"));
                        }
                        Err(e) => dlog(&format!("promote gi={gi} ERR {e}")),
                    }
                }
            } else if is_top {
                let r = SetWindowPos(
                    hw,
                    Some(HWND_NOTOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
                match r {
                    Ok(_) => dlog(&format!("demote gi={gi} OK")),
                    Err(e) => dlog(&format!("demote gi={gi} ERR {e}")),
                }
            }
        }
        // 低层重插:按用户偏好序(want)自顶向下链式插入 —— 最上层最先归位。
        // 簇已正确时完全不动(幂等)。锚点回退链:黑名单与 TOPMOST 带窗跳过
        // (跨带插入只会落在普通带顶端,形成无效定点)。
        if a.shell.is_some() && !cluster_ok {
            // 死锁逃逸:同一失败形态连续出现 → 先散开(全部 HWND_TOP)再重建,
            // 打破"插入成功但落点不变"的定点(快速 Win+D 后出现过)
            {
                use std::sync::atomic::{AtomicU32, Ordering};
                static LAST_PAT: AtomicU32 = AtomicU32::new(u32::MAX);
                static FAIL_CNT: AtomicU32 = AtomicU32::new(0);
                let pat = {
                    let mut v: u32 = 0;
                    for &g in &a.cluster {
                        v = v.wrapping_mul(31).wrapping_add(g as u32 + 1);
                    }
                    v
                };
                if LAST_PAT.swap(pat, Ordering::SeqCst) == pat {
                    let n = FAIL_CNT.fetch_add(1, Ordering::SeqCst) + 1;
                    if n == 4 {
                        dlog(&format!(
                            "z ESCAPE: same fail pattern x{n}, scatter-bottom+rebuild clu={:?}",
                            a.cluster
                        ));
                        // HWND_BOTTOM(而非 HWND_TOP):散到底再由重建拉起,
                        // 不会短暂盖住新打开的窗口
                        for &gi in want.iter().rev() {
                            let hw = app.groups[gi].hwnd;
                            if hw.is_invalid() {
                                continue;
                            }
                            let _ = SetWindowPos(
                                hw,
                                Some(HWND_BOTTOM),
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                        }
                        FAIL_CNT.store(0, Ordering::SeqCst);
                    }
                } else {
                    FAIL_CNT.store(0, Ordering::SeqCst);
                }
            }
            let mut after: Option<HWND> = None;
            for &gi in want.iter() {
                let hw = app.groups[gi].hwnd;
                if hw.is_invalid() {
                    continue;
                }
                let mut done = false;
                let mut cands: Vec<Option<HWND>> = Vec::new();
                if let Some(h) = after {
                    cands.push(Some(h));
                }
                // 回退链:低→高;非 TOPMOST 才作为插入锚(末尾兜底 HWND_TOP)
                for cand in a.chain.iter().rev() {
                    let ex = GetWindowLongPtrW(*cand, GWL_EXSTYLE);
                    if ex & (WS_EX_TOPMOST.0 as isize) != 0 || anchor_denied(*cand) {
                        continue;
                    }
                    cands.push(Some(*cand));
                }
                cands.push(Some(HWND_TOP));
                for (k, cand) in cands.iter().enumerate() {
                    let r = SetWindowPos(
                        hw,
                        *cand,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                    match r {
                        Ok(_) => {
                            acted = true;
                            done = true;
                            after = Some(hw);
                            if k > 0 {
                                dlog(&format!(
                                    "  ins gi={gi} via fb[{k}] after={:?}",
                                    cand.map(|h| h.0 as usize)
                                ));
                            } else {
                                dlog(&format!(
                                    "  ins gi={gi} after={:?}",
                                    cand.map(|h| h.0 as usize)
                                ));
                            }
                            break;
                        }
                        Err(e) => {
                            if let Some(h) = cand {
                                anchor_mark_denied(*h);
                            }
                            dlog(&format!(
                                "anchor deny gi={gi} after={:?}: {e}",
                                cand.map(|h| h.0 as usize)
                            ));
                        }
                    }
                }
                if !done {
                    dlog(&format!(
                        "ins gi={gi} no usable anchor (chain={})",
                        a.chain.len()
                    ));
                }
            }
            // Win+D 后 shell 抬升壁纸层(Progman)到普通带顶,与面板的 HWND_TOP
            // 兜底形成震荡。此步把 shell 压到最底面板之下,一次性终结竞争:
            // 无论 shell 刚才把 Progman 抬到哪儿,这里强制归位到面板之下。
            if let Some(sh) = a.shell {
                if let Some(&last_gi) = want.last() {
                    let lh = app.groups[last_gi].hwnd;
                    if !lh.is_invalid() {
                        let _ = SetWindowPos(
                            sh,
                            Some(lh),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                    }
                }
            }
        }
        if acted {
            let post = walk_anchor(app);
            dlog(&format!(
                "  post clu={:?} disp={} fn={} pn={} sn={}",
                post.cluster, post.displaced, post.first_panel_n, post.prev_n, post.shell_n
            ));
        }
    }
}

// ================= 命中 =================

pub fn dlog(msg: &str) {
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

fn lparam_pt(lparam: LPARAM) -> POINT {
    POINT {
        x: (lparam.0 as u32 & 0xFFFF) as i16 as i32,
        y: ((lparam.0 as u32 >> 16) & 0xFFFF) as i16 as i32,
    }
}

/// 消息客户区坐标 → 屏幕坐标。拖拽/缩放增量必须用屏幕坐标求差:
/// 窗口每步都会在光标下移动,客户区坐标随之“换帧”,帧间求差会混入
/// 窗口自身位移(上一步位移被反向扣掉),表现为拖拽抖动、回弹。
/// 屏幕坐标不依赖窗口位置,真实输入与投递消息走同一路径。
fn lparam_screen(hwnd: HWND, lparam: LPARAM) -> POINT {
    let mut s = lparam_pt(lparam);
    unsafe {
        let _ = ClientToScreen(hwnd, &mut s);
    }
    s
}

fn find_group(app: &App, hwnd: HWND) -> Option<usize> {
    app.groups.iter().position(|g| g.hwnd == hwnd)
}

/// 悬停在哪个分组面板上(屏幕坐标)
fn group_at_point(app: &App, sp: POINT) -> Option<usize> {
    unsafe {
        for gi in 0..app.groups.len() {
            let hw = app.groups[gi].hwnd;
            if hw.is_invalid() || !IsWindowVisible(hw).as_bool() {
                continue;
            }
            let mut r = RECT::default();
            if GetWindowRect(hw, &mut r).is_ok()
                && sp.x >= r.left
                && sp.x < r.right
                && sp.y >= r.top
                && sp.y < r.bottom
            {
                return Some(gi);
            }
        }
    }
    None
}

fn set_cursor_sizeall() {
    unsafe {
        if let Ok(c) = LoadCursorW(None, IDC_SIZEALL) {
            SetCursor(Some(c));
        }
    }
}

// ================= 拖拽状态 =================

unsafe fn begin_or_update_drag(app: &mut App, src_gi: usize, pt: POINT) {
    if !app.mouse.dragging {
        let dx = pt.x - app.mouse.down_pt.x;
        let dy = pt.y - app.mouse.down_pt.y;
        if dx * dx + dy * dy < DRAG_THRESHOLD * DRAG_THRESHOLD {
            return;
        }
        dlog(&format!("DRAG start src={src_gi} pt=({},{})", pt.x, pt.y));
        app.mouse.dragging = true;
        app.mouse.src = Some(src_gi);
        app.mouse.hover_cell = None;
        if let Some(key_item) = app.mouse.down {
            app.ghost_item = Some(key_item);
            let _ = ShowWindow(app.ghost, SW_SHOWNOACTIVATE);
            app.render_ghost();
        }
        set_cursor_sizeall();
    }
    // 幽灵跟随 + 目标判定:用“源面板原点 + 消息客户坐标”推算屏幕位置
    // (不依赖 GetCursorPos —— 真实输入与投递消息走同一路径)
    {
        let gsrc = &app.groups[src_gi];
        let sp = POINT {
            x: gsrc.gx - MARGIN + pt.x,
            y: gsrc.gy - MARGIN + pt.y,
        };
        // 幽灵内的"图标中心"精确对准鼠标:
        // 窗口原点 = 光标 - 图标中心在幽灵窗口内的偏移
        let gw = app.cell_w();
        let gxs = sp.x - (MARGIN + gw / 2);
        let gys = sp.y - (MARGIN + ICON_TOP + ICON_SZ / 2);
        let _ = SetWindowPos(
            app.ghost,
            Some(HWND_TOPMOST),
            gxs,
            gys,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let new_target = group_at_point(app, sp);
        if new_target != app.mouse.target {
            let old = app.mouse.target;
            app.mouse.target = new_target;
            // 重绘受影响面板
            let src = app.mouse.src.unwrap_or(0);
            app.render_panel(src);
            if let Some(t) = old {
                app.render_panel(t);
            }
            if let Some(t) = new_target {
                app.render_panel(t);
            }
        }
        // 组内拖拽:追踪落点格(高亮 + 插入位置);pt 即源面板客户坐标
        if new_target == app.mouse.src {
            let hc = app.cell_index(src_gi, pt.x, pt.y);
            if hc != app.mouse.hover_cell {
                app.mouse.hover_cell = hc;
                app.render_panel(src_gi);
            }
        } else if app.mouse.hover_cell.take().is_some() {
            app.render_panel(src_gi);
        }
    }
}

unsafe fn end_drag(app: &mut App) {
    let src = app.mouse.src;
    let target = app.mouse.target;
    let down = app.mouse.down;
    let dragging = app.mouse.dragging;
    let hover_cell = app.mouse.hover_cell;
    if dragging {
        unsafe {
            let _ = ShowWindow(app.ghost, SW_HIDE);
        }
        if let (Some(s), Some(t), Some((gs, idx))) = (src, target, down) {
            dlog(&format!("DROP s={s} t={t} idx={idx}"));
            if s == gs && t != s {
                let mut moving = app.sel_items(s);
                if !moving.contains(&idx) {
                    moving.push(idx);
                    moving.sort_unstable();
                }
                if moving.len() <= 1 {
                    app.move_item(s, idx, t);
                } else {
                    for &i in moving.iter().rev() {
                        app.move_item(s, i, t);
                    }
                }
                app.clear_sel();
            } else if s == gs {
                // 组内:插入到落点格(手动排序/调换位置)
                if let Some(cell) = hover_cell {
                    if cell != idx {
                        dlog(&format!("reorder g={s} idx={idx} -> cell={cell}"));
                        app.reorder_in_group(s, idx, cell);
                        app.render_panel(s);
                    }
                }
            }
        }
    }
    app.mouse = Default::default();
    app.ghost_item = None;
    // 重绘受影响面板
    if let Some(s) = src {
        app.render_panel(s);
    }
    if let Some(t) = target {
        app.render_panel(t);
    }
}

pub fn cursor_pos() -> Option<POINT> {
    let mut pt = POINT::default();
    unsafe { GetCursorPos(&mut pt).ok()? };
    Some(pt)
}

// ================= 菜单 =================

/// 面板右键菜单:新建 / 重命名 / 标题对齐(单选) / 关闭(仅自建面板)
pub fn show_panel_menu(app: &mut App, gi: usize, hwnd: HWND, x: i32, y: i32) {
    unsafe {
        if gi >= app.groups.len() {
            return;
        }
        let Ok(menu) = CreatePopupMenu() else { return };
        let _ = AppendMenuW(menu, MF_STRING, IDM_PNEW, PCWSTR(ws(crate::lang::t("new_panel")).as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, IDM_PRENAME + gi, PCWSTR(ws(crate::lang::t("rename_panel")).as_ptr()));
        let Ok(sub) = CreatePopupMenu() else {
            let _ = DestroyMenu(menu);
            return;
        };
        let cur = app.groups[gi].title_align;
        let align_labels = if crate::lang::is_en() {
            ["Left(&L)", "Center(&C)", "Right(&R)"]
        } else {
            ["居左(&L)", "居中(&C)", "居右(&R)"]
        };
        for (i, label) in align_labels.iter().enumerate() {
            let f = if cur as usize == i {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            let t = ws(label);
            let _ = AppendMenuW(sub, f, IDM_PALGN + gi * 4 + i, PCWSTR(t.as_ptr()));
        }
        let _ = AppendMenuW(menu, MF_POPUP, sub.0 as usize, PCWSTR(ws(crate::lang::t("title_align")).as_ptr()));
        let ts = if app.groups[gi].title_show {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        let _ = AppendMenuW(menu, MF_STRING | ts, IDM_PTITLE + gi, PCWSTR(ws(crate::lang::t("show_title")).as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, IDM_PHIDE + gi, PCWSTR(ws(crate::lang::t("hide_panel")).as_ptr()));
        let lk1 = if app.locked { MF_CHECKED } else { MF_UNCHECKED };
        let _ = AppendMenuW(menu, MF_STRING | lk1, IDM_LOCK, PCWSTR(ws(crate::lang::t("lock_layout")).as_ptr()));
        if gi >= 4 {
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(menu, MF_STRING, IDM_PDEL + gi, PCWSTR(ws(crate::lang::t("delete_panel")).as_ptr()));
        }
        // 应用级选项(与托盘菜单一致;选中后经 WM_COMMAND 统一分发)
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, IDM_SETTINGS, PCWSTR(ws(crate::lang::t("settings")).as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, IDM_REFRESH, PCWSTR(ws(crate::lang::t("refresh")).as_ptr()));
        let vis = if app.groups_visible {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        let _ = AppendMenuW(menu, MF_STRING | vis, IDM_TOGGLE_GROUPS, PCWSTR(ws(crate::lang::t("show_panels")).as_ptr()));
        let org = if app.show_original {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        let _ = AppendMenuW(menu, MF_STRING | org, IDM_TOGGLE_ORIG, PCWSTR(ws(crate::lang::t("show_sys_icons")).as_ptr()));
        let ar = if desktop::autorun_enabled() {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        let _ = AppendMenuW(menu, MF_STRING | ar, IDM_AUTORUN, PCWSTR(ws(crate::lang::t("autostart")).as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, IDM_LANG, PCWSTR(ws(crate::lang::t("lang_switch")).as_ptr()));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, IDM_EXIT, PCWSTR(ws(crate::lang::t("exit")).as_ptr()));
        keybd_alt();
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, x, y, Some(0), hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu); // 含子菜单
    }
}

pub fn show_tray_menu(app: &mut App) {
    unsafe {
        let hwnd = app.groups[0].hwnd;
        let Ok(menu) = CreatePopupMenu() else { return };
        let _ = AppendMenuW(menu, MF_STRING, IDM_SETTINGS, PCWSTR(ws(crate::lang::t("settings")).as_ptr()));
        let mut any_hidden = false;
        for (gi, g) in app.groups.iter().enumerate() {
            if g.hidden {
                if !any_hidden {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                    any_hidden = true;
                }
                let t = ws(&format!("{}{}", crate::lang::t("show_panel"), g.title));
                let _ = AppendMenuW(menu, MF_STRING, IDM_PSHOW + gi, PCWSTR(t.as_ptr()));
            }
        }
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, IDM_REFRESH, PCWSTR(ws(crate::lang::t("refresh")).as_ptr()));
        let vis = if app.groups_visible { MF_CHECKED } else { MF_UNCHECKED };
        let _ = AppendMenuW(menu, MF_STRING | vis, IDM_TOGGLE_GROUPS, PCWSTR(ws(crate::lang::t("show_panels")).as_ptr()));
        let org = if app.show_original { MF_CHECKED } else { MF_UNCHECKED };
        let _ = AppendMenuW(menu, MF_STRING | org, IDM_TOGGLE_ORIG, PCWSTR(ws(crate::lang::t("show_sys_icons")).as_ptr()));
        let ar = if desktop::autorun_enabled() {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        let _ = AppendMenuW(menu, MF_STRING | ar, IDM_AUTORUN, PCWSTR(ws(crate::lang::t("autostart")).as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, IDM_LANG, PCWSTR(ws(crate::lang::t("lang_switch")).as_ptr()));
        let lk = if app.locked { MF_CHECKED } else { MF_UNCHECKED };
        let _ = AppendMenuW(menu, MF_STRING | lk, IDM_LOCK, PCWSTR(ws(crate::lang::t("lock_layout")).as_ptr()));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, IDM_EXIT, PCWSTR(ws(crate::lang::t("exit")).as_ptr()));

        // 托盘菜单需要前台才能正确收起
        keybd_alt();
        let _ = SetForegroundWindow(hwnd);
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            Some(0),
            hwnd,
            None,
        )
        .0 as usize;
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);

        // 选中项统一转成 WM_COMMAND,由窗口过程分发(菜单内不做业务,
        // 也让命令路径可被直接驱动)
        let _ = PostMessageW(Some(hwnd), WM_COMMAND, WPARAM(cmd), LPARAM(0));
    }
}

fn keybd_alt() {
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            keybd_event, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_MENU,
        };
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_EXTENDEDKEY, 0);
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
    }
}

// ================= 托盘 =================

pub fn tray_add(app: &mut App) {
    unsafe {
        let icon = tool_icon();
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = app.groups[0].hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_INFO;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = icon;
        copy_wstr(&mut nid.szTip, &format!("rdesktop · {}", crate::lang::t("tray_tip")));
        copy_wstr(&mut nid.szInfo, crate::lang::t("tray_info"));
        copy_wstr(&mut nid.szInfoTitle, "rdesktop");
        nid.dwInfoFlags = NIIF_INFO;
        if Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
            app.tray_added = true;
        }
    }
}

pub fn tray_delete(app: &App) {
    if !app.tray_added {
        return;
    }
    unsafe {
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = app.groups[0].hwnd;
        nid.uID = 1;
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

fn copy_wstr(dst: &mut [u16], s: &str) {
    let w: Vec<u16> = s.encode_utf16().collect();
    let n = w.len().min(dst.len() - 1);
    dst[..n].copy_from_slice(&w[..n]);
    dst[n] = 0;
    for c in dst.iter_mut().skip(n + 1) {
        *c = 0;
    }
}

// ================= 窗口过程 =================

unsafe extern "system" fn panel_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // 取 App 指针
    let app_ptr: *mut App = if msg == WM_NCCREATE {
        let cs = &*(lparam.0 as *const CREATESTRUCTW);
        let p = cs.lpCreateParams as *mut App;
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, p as isize);
        p
    } else {
        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App
    };
    if app_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let app = &mut *app_ptr;

    match msg {
        // 转发 DefWindowProc:窗口文本在此步存入(否则标题永远为空)
        WM_NCCREATE => return DefWindowProcW(hwnd, msg, wparam, lparam),

        WM_PAINT => {
            let _ = ValidateRect(Some(hwnd), None);
            return LRESULT(0);
        }

        WM_NCHITTEST => {
            // NCHITTEST 给的是屏幕坐标;热区/标题判定必须按窗口本地(客户区)
            // 坐标,否则面板偏离屏幕左上角后光标提示全部失效。
            let mut p = lparam_pt(lparam);
            let _ = ScreenToClient(hwnd, &mut p);
            let gi = match find_group(app, hwnd) {
                Some(g) => g,
                None => {
                    app.hover = 0;
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
            };
            // 尺寸先取,避免与 hover 写入的可变借用冲突
            let (pw, ph) = {
                let g = &app.groups[gi];
                (g.panel_w, g.panel_h)
            };
            // 客户区原点 = 面板左上角 - MARGIN
            let th = app.title_h(gi);
            let px = p.x - MARGIN;
            let py = p.y - MARGIN;
            // 1) 边缘/角落 = 缩放热区(自实现拖拽,不走系统 sizing 循环)
            let zone = edge_zone(gi, app, p.x, p.y);
            if zone != 0 {
                if app.hover != zone {
                    dlog(&format!("hover -> edge 0x{zone:x}"));
                    app.hover = zone;
                }
                if let Ok(c) = LoadCursorW(None, zone_cursor(zone)) {
                    SetCursor(Some(c));
                }
                return LRESULT(HTCLIENT as isize);
            }
            if px >= 0 && py >= 0 && px < pw && py < ph {
                if py < th && !app.locked {
                    // 标题栏 = 移动热区(光标由 WM_SETCURSOR 统一给出)
                    if app.hover != HOVER_TITLE {
                        dlog("hover -> TITLE");
                        app.hover = HOVER_TITLE;
                    }
                    if let Ok(c) = LoadCursorW(None, IDC_SIZEALL) {
                        SetCursor(Some(c));
                    }
                    return LRESULT(HTCLIENT as isize);
                }
                if app.hover != 0 {
                    dlog("hover ->0");
                    app.hover = 0;
                }
                return LRESULT(HTCLIENT as isize);
            }
            if app.hover != 0 {
                dlog("hover ->0");
                app.hover = 0;
            }
            // 阴影区域:吞掉(桌面层无可交互对象)
            return LRESULT(HTCLIENT as isize);
        }

        WM_MOVE => {
            // 面板(可能被标题栏拖动)位置同步
            if let Some(gi) = find_group(app, hwnd) {
                let mut r = RECT::default();
                if GetWindowRect(hwnd, &mut r).is_ok() {
                    let g = &mut app.groups[gi];
                    g.gx = r.left + MARGIN;
                    g.gy = r.top + MARGIN;
                }
            }
            return LRESULT(0);
        }
        WM_EXITSIZEMOVE => {
            // 对齐:松手后位置吸附到网格
            if let Some(gi) = find_group(app, hwnd) {
                let grid = app.settings.snap_grid;
                if grid > 0 {
                    let (gx, gy) = {
                        let g = &app.groups[gi];
                        (g.gx, g.gy)
                    };
                    let nx = ((gx + grid / 2) / grid) * grid;
                    let ny = ((gy + grid / 2) / grid) * grid;
                    if nx != gx || ny != gy {
                        {
                            let g = &mut app.groups[gi];
                            g.gx = nx;
                            g.gy = ny;
                        }
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            nx - MARGIN,
                            ny - MARGIN,
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                }
                app.save_config();
            }
            return LRESULT(0);
        }

        WM_LBUTTONDOWN => {
            let p = lparam_pt(lparam);
            dlog(&format!("LBD-enter hwnd={} p=({},{})", hwnd.0 as isize, p.x, p.y));
            if let Some(gi) = find_group(app, hwnd) {
                // a) 边缘热区:开始手动缩放
                let zone = edge_zone(gi, app, p.x, p.y);
                if zone != 0 {
                    let sp = lparam_screen(hwnd, lparam);
                    let mut wr = RECT::default();
                    let _ = GetWindowRect(hwnd, &mut wr);
                    app.rs = Some(crate::app::ResizeState {
                        gi,
                        zone,
                        start_px: sp.x,
                        start_py: sp.y,
                        win_l: wr.left,
                        win_t: wr.top,
                        win_r: wr.right,
                        win_b: wr.bottom,
                    });
                    let _ = SetCapture(hwnd);
                    dlog(&format!("resize start gi={gi} zone=0x{zone:x}"));
                    return LRESULT(0);
                }
                // b) 点击标题:提层 + 开始自实现移动(不走系统 HTCAPTION 模态循环,
                //    避免与 WS_EX_NOACTIVATE / 定时器 SetWindowPos 干涉导致"拖不动")
                let mut title_started = false;
                let th = app.title_h(gi);
                {
                    let g = &app.groups[gi];
                    let px = p.x - MARGIN;
                    let py = p.y - MARGIN;
                    if px >= 0 && py >= 0 && px < g.panel_w && py < th {
                        title_started = true;
                        let exv = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                        if exv & (WS_EX_TOPMOST.0 as isize) != 0 {
                            // 已是最高层:在置顶带内提到最前
                            let _ = SetWindowPos(
                                hwnd,
                                Some(HWND_TOPMOST),
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                        } else if let Some(a) = compute_anchor(app).1 {
                            // 最低层:提到本组最上层(仍低于所有应用)
                            let _ = SetWindowPos(
                                hwnd,
                                Some(a),
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                            // 记住叠放偏好:纠偏时保序,否则会被升序重建压回
                            app.raise_pref(gi);
                            dlog(&format!("title raise gi={gi} pref={:?}", app.z_pref));
                        }
                        // 锁定时跳过 mv 启动(不可拖动面板)
                        if app.locked {
                            return LRESULT(0);
                        }
                        let mut wr = RECT::default();
                        let _ = GetWindowRect(hwnd, &mut wr);
                        let sp = lparam_screen(hwnd, lparam);
                        app.mv = Some(crate::app::MoveState {
                            gi,
                            start_px: sp.x,
                            start_py: sp.y,
                            win_l: wr.left,
                            win_t: wr.top,
                        });
                        let _ = SetCapture(hwnd);
                        dlog(&format!("mv start gi={gi} p=({},{}) win=({},{})", p.x, p.y, wr.left, wr.top));
                    }
                }
                if title_started {
                    return LRESULT(0);
                }
                // b2) 滚动条:轨道点击跳转 / 滑块拖动
                if app.sb_hit(gi, p.x, p.y) {
                    let (tx, ty, tw, th_track) = app.scroll_track(gi);
                    let (_, thumb_y, _, thumb_h) = app.scroll_thumb(gi);
                    let max = app.max_scroll(gi);
                    let mut sy = app.groups[gi].scroll_y;
                    let in_thumb = p.y >= thumb_y && p.y <= thumb_y + thumb_h;
                    if !in_thumb && th_track > thumb_h && max > 0 {
                        // 轨道点击:滑块中心跳到点击处
                        let span = (th_track - thumb_h).max(1);
                        let rel = (p.y - thumb_y - thumb_h / 2).clamp(0, span);
                        sy = ((rel * max) / span).clamp(0, max);
                        app.groups[gi].scroll_y = sy;
                        app.render_panel(gi);
                    }
                    app.sb = Some(crate::app::SbState {
                        gi,
                        start_y: p.y,
                        start_scroll: sy,
                    });
                    let _ = SetCapture(hwnd);
                    dlog(&format!("sb down gi={gi} thumb={in_thumb}"));
                    return LRESULT(0);
                }
                // c) 溢出条 "+N":展开到能放下全部条目
                if app.chip_at(gi, p.x, p.y) {
                    app.expand_to_fit(gi);
                    app.render_panel(gi);
                    dlog(&format!("chip expand gi={gi}"));
                    return LRESULT(0);
                }
                let hit = app.item_at(gi, p.x, p.y);
                dlog(&format!("LBD g={gi} client=({},{}) hit={:?}", p.x, p.y, hit));
                if let Some(idx) = app.item_at(gi, p.x, p.y) {
                    let ctrl = wparam.0 & 0x8 != 0;
                    let shift = wparam.0 & 0x4 != 0;
                    if ctrl || shift {
                        if shift {
                            let anchor = app
                                .selected
                                .or_else(|| app.sel_multi.last().copied())
                                .filter(|(g, _)| *g == gi)
                                .map(|(_, a)| a);
                            if let Some(a) = anchor {
                                let lo = a.min(idx);
                                let hi = a.max(idx);
                                app.sel_multi = (lo..=hi).map(|i| (gi, i)).collect();
                            } else {
                                app.selected = Some((gi, idx));
                            }
                        } else if app.is_sel(gi, idx) {
                            app.sel_multi.retain(|&k| k != (gi, idx));
                            if app.selected == Some((gi, idx)) {
                                app.selected = None;
                            }
                        } else {
                            app.sel_multi.push((gi, idx));
                            if app.selected.is_none() {
                                app.selected = Some((gi, idx));
                            }
                        }
                        dlog(&format!(
                            "multi n={} ctrl={ctrl} shift={shift}",
                            app.sel_multi.len()
                        ));
                        app.mouse.down = Some((gi, idx));
                        app.mouse.down_pt = p;
                        let _ = SetCapture(hwnd);
                        app.render_panel(gi);
                    } else if !app.is_sel(gi, idx) {
                        dlog(&format!("DOWN g={gi} idx={idx} (single)"));
                        let old = app.selected;
                        app.clear_sel();
                        app.selected = Some((gi, idx));
                        app.mouse.down = Some((gi, idx));
                        app.mouse.down_pt = p;
                        let _ = SetCapture(hwnd);
                        if let Some((ogi, _)) = old {
                            if ogi != gi {
                                app.render_panel(ogi);
                            }
                        }
                        app.render_panel(gi);
                    } else {
                        // 点击已选条目:保留多选(整组拖动)
                        dlog(&format!("DOWN g={gi} idx={idx} (keep-multi)"));
                        app.mouse.down = Some((gi, idx));
                        app.mouse.down_pt = p;
                        let _ = SetCapture(hwnd);
                        app.render_panel(gi);
                    }
                } else {
                    // 空白:清选 + 框选起点
                    let old = app.selected.take();
                    let had_multi = !app.sel_multi.is_empty();
                    app.sel_multi.clear();
                    app.mouse.down = None;
                    app.marquee = Some(crate::app::Marquee {
                        gi,
                        x0: p.x,
                        y0: p.y,
                        x1: p.x,
                        y1: p.y,
                        active: false,
                    });
                    let _ = SetCapture(hwnd);
                    if old.is_some() || had_multi {
                        if let Some((ogi, _)) = old {
                            if ogi != gi {
                                app.render_panel(ogi);
                            }
                        }
                        app.render_panel(gi);
                    }
                }
            }
            return LRESULT(0);
        }

        WM_MOUSEMOVE => {
            if app.rs.is_some() {
                let p = lparam_pt(lparam);
                do_resize(app, hwnd, p);
                return LRESULT(0);
            }
            if app.mv.is_some() {
                let sp = lparam_screen(hwnd, lparam);
                let mut mv = app.mv.take().unwrap();
                // 屏幕坐标增量(见 lparam_screen 注释:客户区坐标混帧会抖动/回弹)
                let dx = sp.x - mv.start_px;
                let dy = sp.y - mv.start_py;
                mv.start_px = sp.x;
                mv.start_py = sp.y;
                mv.win_l += dx;
                mv.win_t += dy;
                let (nl, nt) = (mv.win_l, mv.win_t);
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        nl,
                        nt,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                {
                    let g = &mut app.groups[mv.gi];
                    g.gx = nl + MARGIN;
                    g.gy = nt + MARGIN;
                }
                app.mv = Some(mv);
                return LRESULT(0);
            }
            if let Some(sb) = app.sb.take() {
                let p = lparam_pt(lparam);
                let (_, _, _, th_track) = app.scroll_track(sb.gi);
                let (_, _, _, thumb_h) = app.scroll_thumb(sb.gi);
                let max = app.max_scroll(sb.gi);
                let span = (th_track - thumb_h).max(1);
                let dy = p.y - sb.start_y;
                let target = sb.start_scroll + dy * max.max(1) / span;
                let new = target.clamp(0, max);
                if new != app.groups[sb.gi].scroll_y {
                    app.groups[sb.gi].scroll_y = new;
                    app.render_panel(sb.gi);
                }
                app.sb = Some(sb);
                return LRESULT(0);
            }
            if let Some(mut mq) = app.marquee.take() {
                let p = lparam_pt(lparam);
                if !mq.active {
                    let dx = p.x - mq.x0;
                    let dy = p.y - mq.y0;
                    if dx * dx + dy * dy > 25 {
                        mq.active = true;
                    }
                }
                if mq.active {
                    mq.x1 = p.x;
                    mq.y1 = p.y;
                    app.render_panel(mq.gi);
                }
                app.marquee = Some(mq);
                return LRESULT(0);
            }
            if app.mouse.down.is_some() {
                let p = lparam_pt(lparam);
                if let Some(gi) = find_group(app, hwnd) {
                    begin_or_update_drag(app, gi, p);
                }
            }
            return LRESULT(0);
        }

        WM_LBUTTONUP => {
            if let Some(rs) = app.rs.take() {
                let _ = ReleaseCapture();
                dlog(&format!(
                    "resize end gi={} {}x{}",
                    rs.gi,
                    app.groups[rs.gi].panel_w,
                    app.groups[rs.gi].panel_h
                ));
                app.save_config();
                return LRESULT(0);
            }
            if let Some(mv) = app.mv.take() {
                let _ = ReleaseCapture();
                // 手动拖放的位置必须跨重启保留:置 user_pos,
                // 否则启动时 place_groups 会把面板重新流式排版(位置丢失)
                app.groups[mv.gi].user_pos = true;
                let grid = app.settings.snap_grid;
                if grid > 0 {
                    let (gx, gy) = {
                        let g = &app.groups[mv.gi];
                        (g.gx, g.gy)
                    };
                    let nx = ((gx + grid / 2) / grid) * grid;
                    let ny = ((gy + grid / 2) / grid) * grid;
                    if nx != gx || ny != gy {
                        {
                            let g = &mut app.groups[mv.gi];
                            g.gx = nx;
                            g.gy = ny;
                        }
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            nx - MARGIN,
                            ny - MARGIN,
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                }
                dlog(&format!("move end gi={}", mv.gi));
                app.save_config();
                return LRESULT(0);
            }
            if let Some(sb) = app.sb.take() {
                let _ = ReleaseCapture();
                dlog(&format!("sb up gi={} y={}", sb.gi, app.groups[sb.gi].scroll_y));
            }
            if let Some(mq) = app.marquee.take() {
                let _ = ReleaseCapture();
                if mq.active {
                    let (x0, x1) = (mq.x0.min(mq.x1), mq.x0.max(mq.x1));
                    let (y0, y1) = (mq.y0.min(mq.y1), mq.y0.max(mq.y1));
                    let g = &app.groups[mq.gi];
                    let cw = app.cell_w();
                    let ch = app.cell_h();
                    let sy = g.scroll_y;
                    let mut hits: Vec<(usize, usize)> = Vec::new();
                    for idx in 0..g.items.len() {
                        let col = (idx as i32) % g.cols.max(1);
                        let row = (idx as i32) / g.cols.max(1);
                        let cx = MARGIN + PAD + col * cw;
                        let cy = MARGIN + app.grid_top(mq.gi) + row * ch - sy;
                        if cx < x1 && cx + cw > x0 && cy < y1 && cy + ch > y0 {
                            hits.push((mq.gi, idx));
                        }
                    }
                    hits.sort_unstable();
                    app.sel_multi = hits;
                    app.selected = app.sel_multi.first().copied();
                    dlog(&format!("marquee sel n={}", app.sel_multi.len()));
                    app.render_panel(mq.gi);
                }
            }
            if app.mouse.down.is_some() {
                end_drag(app);
                let _ = ReleaseCapture();
            }
            return LRESULT(0);
        }

        WM_LBUTTONDBLCLK => {
            dlog("DBLCLK");
            let p = lparam_pt(lparam);
            if let Some(gi) = find_group(app, hwnd) {
                if let Some(idx) = app.item_at(gi, p.x, p.y) {
                    if app.is_sel(gi, idx) {
                        let mut targets = app
                            .sel_items(gi)
                            .iter()
                            .map(|i| (gi, *i))
                            .collect::<Vec<_>>();
                        for (g2, i2) in &app.sel_multi {
                            if !targets.contains(&(*g2, *i2)) {
                                targets.push((*g2, *i2));
                            }
                        }
                        if let Some(pos) = targets.iter().position(|t| *t == (gi, idx)) {
                            targets.swap(0, pos);
                        }
                        for (g2, i2) in targets.iter().take(16) {
                            if let Some(it) = app.groups[*g2].items.get(*i2) {
                                desktop::open_item(Some(hwnd), it);
                            }
                        }
                    } else {
                        let item = app.groups[gi].items[idx].clone();
                        desktop::open_item(Some(hwnd), &item);
                    }
                }
            }
            return LRESULT(0);
        }

        WM_RBUTTONDOWN => {
            dlog("RBUTTON");
            let p = lparam_pt(lparam);
            if let Some(gi) = find_group(app, hwnd) {
                if let Some(idx) = app.item_at(gi, p.x, p.y) {
                    let old = app.selected;
                    app.selected = Some((gi, idx));
                    if let Some((ogi, _)) = old {
                        if ogi != gi {
                            app.render_panel(ogi);
                        }
                    }
                    app.render_panel(gi);
                    let item = app.groups[gi].items[idx].clone();
                    let mut cp = POINT::default();
                    let _ = GetCursorPos(&mut cp);
                    desktop::show_context_menu(hwnd, &item, cp.x, cp.y, IDM_OPEN);
                } else {
                    // 空白/标题右键:面板菜单(面板管理 + 应用选项,含"设置")
                    let mut cp = POINT::default();
                    let _ = GetCursorPos(&mut cp);
                    show_panel_menu(app, gi, hwnd, cp.x, cp.y);
                }
            }
            return LRESULT(0);
        }

        WM_SETCURSOR => {
            // 优先级:缩放中(对应边角光标)> 移动中/图标拖拽(移动光标)>
            //         悬停热区:标题=移动光标、边缘=缩放光标 > 默认箭头
            // (必须在此给出光标,否则 DefWindowProc 会把 NCHITTEST 里设置的光标覆盖回箭头)
            if let Some(rs) = &app.rs {
                if let Ok(c) = LoadCursorW(None, zone_cursor(rs.zone)) {
                    SetCursor(Some(c));
                }
                return LRESULT(TRUE.0 as isize);
            }
            if app.mv.is_some() || app.mouse.dragging {
                set_cursor_sizeall();
                return LRESULT(TRUE.0 as isize);
            }
            if app.hover == HOVER_TITLE && !app.locked {
                if let Ok(c) = LoadCursorW(None, IDC_SIZEALL) {
                    SetCursor(Some(c));
                }
                return LRESULT(TRUE.0 as isize);
            }
            if app.hover != 0 {
                if let Ok(c) = LoadCursorW(None, zone_cursor(app.hover)) {
                    SetCursor(Some(c));
                }
                return LRESULT(TRUE.0 as isize);
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }

        WM_SYSCOMMAND => {
            // 拒绝最小化/最大化:面板必须常驻桌面层(如 Windows+D 触发的最小化)
            let cmd = wparam.0 & 0xFFF0;
            if cmd == SC_MINIMIZE as usize || cmd == SC_MAXIMIZE as usize {
                return LRESULT(0);
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }

        WM_TIMER => {
            if wparam.0 == TIMER_SETTLE_ID {
                // 连续校正窗:每次 ensure,预算用尽后停表(z_pause 时整窗暂停)
                if app.groups_visible && !app.z_pause {
                    ensure_z_order(app);
                }
                app.settle_left = app.settle_left.saturating_sub(1);
                if app.settle_left == 0 {
                    if let Some(h) = app.groups.first().map(|g| g.hwnd) {
                        if !h.is_invalid() {
                            unsafe {
                                let _ = KillTimer(Some(h), TIMER_SETTLE_ID);
                            }
                        }
                    }
                    dlog("settle window done");
                }
                return LRESULT(0);
            }
            if wparam.0 == 8 {
                // 延迟重渲染:Win+D/隐藏恢复后重提交所有面板 ULW 像素
                let _ = KillTimer(Some(hwnd), 8);
                app.render_all();
                return LRESULT(0);
            }
            if wparam.0 == TIMER_ZORDER {
                // 兜底:若仍被最小化(某些 ShowDesktop 路径),立即还原
                for gi2 in 0..app.groups.len() {
                    let hw2 = app.groups[gi2].hwnd;
                    if !hw2.is_invalid() && IsIconic(hw2).as_bool() {
                        let _ = ShowWindow(hw2, SW_RESTORE);
                        app.render_panel(gi2);
                        dlog("timer IsIconic -> SW_RESTORE + render");
                    }
                }
                // 保持桌面图标隐藏(explore 重启自愈)+ z序(测试可暂停)
                if !app.z_pause {
                if !app.show_original {
                    let lv = desktop::find_desktop_listview().unwrap_or(app.desktop_lv);
                    if !lv.is_invalid() {
                        if IsWindowVisible(lv).as_bool() {
                            desktop::hide_listview(lv);
                        }
                        app.desktop_lv = lv;
                    }
                }
                if app.groups_visible {
                    ensure_z_order(app);
                }
                }
                // 桌面目录内容变化自动刷新(新建/删除/改名) —— 仅在"自动整理"开启时
                if !app.mouse.dragging && app.settings.auto_tidy {
                    if let Some(mt) = desktop::desktop_dirs_mtime() {
                        if app.last_mtime != Some(mt) {
                            let first = app.last_mtime.is_none();
                            app.last_mtime = Some(mt);
                            if !first {
                                app.refresh_from_disk();
                            }
                        }
                    }
                }
            }
            return LRESULT(0);
        }

        WM_TRAY => {
            let ev = lparam.0 as u32;
            match ev {
                WM_RBUTTONUP | WM_CONTEXTMENU => show_tray_menu(app),
                WM_LBUTTONDBLCLK => {
                    // 双击托盘:打开设置
                    let _ = PostMessageW(Some(hwnd), WM_OPEN_SETTINGS, WPARAM(0), LPARAM(0));
                }
                _ => {}
            }
            return LRESULT(0);
        }

        WM_SIZE => {
            if wparam.0 == SIZE_MINIMIZED as usize {
                // 界面不允许被最小化隐藏(Windows+D、ShowWindow 直调等路径)。
                // 注意:SW_SHOWNOACTIVATE 不会退出最小化态,必须用 SW_RESTORE;
                // 面板是 WS_EX_NOACTIVATE,SW_RESTORE 也不会抢焦点。
                let _ = ShowWindow(hwnd, SW_RESTORE);
                dlog("WM_SIZE SIZE_MINIMIZED -> SW_RESTORE + repaint");
                // 还原后立刻重提 ULW 像素(还原周期可能清空分层内容)
                if let Some(gi) = find_group(app, hwnd) {
                    app.render_panel(gi);
                }
                // SW_RESTORE 可能把面板抬到普通应用之上,立即归位(幂等,无副作用)
                ensure_z_order(app);
            }
            return LRESULT(0);
        }

        WM_MOUSEWHEEL => {
            // lparam = 屏幕坐标:滚动光标下的面板(与焦点无关)
            let sp = lparam_pt(lparam);
            let delta = (((wparam.0 >> 16) & 0xFFFF) as u16 as i16) as i32;
            if delta == 0 {
                return LRESULT(0);
            }
            for gi in 0..app.groups.len() {
                let hw = app.groups[gi].hwnd;
                if hw.is_invalid() || app.groups[gi].hidden {
                    continue;
                }
                if !IsWindowVisible(hw).as_bool() {
                    continue;
                }
                let mut r = RECT::default();
                if GetWindowRect(hw, &mut r).is_err() {
                    continue;
                }
                if sp.x < r.left || sp.x >= r.right || sp.y < r.top || sp.y >= r.bottom {
                    continue;
                }
                let dy = -(delta) * app.cell_h() / 120; // 滚轮向下=正方向(显示下方内容)
                let max = app.max_scroll(gi);
                let new = (app.groups[gi].scroll_y + dy).clamp(0, max);
                if new != app.groups[gi].scroll_y {
                    app.groups[gi].scroll_y = new;
                    app.render_panel(gi);
                    dlog(&format!("wheel gi={gi} dy={dy} y={new}/{max}"));
                }
                return LRESULT(0);
            }
            return LRESULT(0);
        }

        WM_Z_REZ => {
            if app.z_pause {
                return LRESULT(0);
            }
            // 防抖:我们自己的 SetWindowPos 也会触发 REORDER,100ms 内合并。
            // 合并(丢弃)时必须把 settle 收尾窗推到“最后一个事件”之后 —— 否则
            // 尾部真实事件(如 Win+D 洗牌的最后一次)会被丢掉且无人再纠正。
            let now = std::time::Instant::now();
            if let Some(t) = app.rez_last {
                if t.elapsed() < std::time::Duration::from_millis(100) {
                    arm_settle(app);
                    return LRESULT(0);
                }
            }
            app.rez_last = Some(now);
            arm_settle(app);
            // 事件驱动:前台切换/最小化/壁纸窗抬升 → 立即纠偏 + 救回被最小化的面板
            if app.groups_visible {
                let mut restored: Vec<usize> = Vec::new();
                for gi in 0..app.groups.len() {
                    let hw = app.groups[gi].hwnd;
                    if !hw.is_invalid() && IsIconic(hw).as_bool() {
                        let _ = ShowWindow(hw, SW_RESTORE);
                        restored.push(gi);
                    }
                }
                ensure_z_order(app);
                // 最小化/还原周期后重新提交分层窗口像素,防止内容空白
                for gi in &restored {
                    app.render_panel(*gi);
                }
                dlog(&format!("rez: restored={:?} z re-anchored", restored));
            }
            return LRESULT(0);
        }

        WM_Z_PAUSE => {
            app.z_pause = !app.z_pause;
            dlog(&format!("z_pause -> {}", app.z_pause));
            return LRESULT(0);
        }

        WM_OPEN_SETTINGS => {
            crate::settings::open_settings(app);
            return LRESULT(0);
        }

        WM_ENDSESSION => {
            if wparam.0 != 0 {
                // 会话结束:立刻恢复桌面图标
                if !app.desktop_lv.is_invalid() {
                    desktop::show_listview(app.desktop_lv);
                }
            }
            return LRESULT(0);
        }

        WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            match id {
                // ---- 应用级选项(托盘/面板菜单共用) ----
                IDM_SETTINGS => {
                    let _ = PostMessageW(Some(hwnd), WM_OPEN_SETTINGS, WPARAM(0), LPARAM(0));
                }
                IDM_REFRESH => app.refresh_from_disk(),
                IDM_LOCK => {
                    app.locked = !app.locked;
                    app.save_config();
                }
                                IDM_TOGGLE_GROUPS => {
                    app.groups_visible = !app.groups_visible;
                    for gi in 0..app.groups.len() {
                        if !app.groups[gi].hwnd.is_invalid() {
                            let sw = if app.groups_visible && !app.groups[gi].hidden {
                                SW_RESTORE
                            } else {
                                SW_HIDE
                            };
                            let _ = ShowWindow(app.groups[gi].hwnd, sw);
                        }
                    }
                    if app.groups_visible {
                        app.render_all();
                        ensure_z_order(app);
                        // Win+D 恢复后 ULW 表面可能被清,延迟重提交
                        let _ = SetTimer(Some(hwnd), 8, 200, None);
                    }
                }
                IDM_TOGGLE_ORIG => {
                    app.show_original = !app.show_original;
                    if app.show_original {
                        if !app.desktop_lv.is_invalid() {
                            desktop::show_listview(app.desktop_lv);
                        }
                    } else if !app.desktop_lv.is_invalid() {
                        desktop::hide_listview(app.desktop_lv);
                    }
                }
                IDM_AUTORUN => {
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
                }
                IDM_EXIT => {
                    PostQuitMessage(0);
                }
                // ---- 面板管理 ----
                IDM_PNEW => {
                    dlog("pnew: begin");
                    let gi = app.new_panel();
                    dlog(&format!("pnew: created gi={gi}"));
                    if let Ok(hm) = GetModuleHandleW(None) {
                        create_panel_window(app, gi, HINSTANCE(hm.0));
                    }
                    dlog("pnew: window ok");
                    app.render_all();
                    dlog("pnew: rendered");
                    crate::settings::open_rename(app, gi);
                    dlog(&format!("panel new gi={gi}"));
                }
                x if (IDM_PRENAME..IDM_PDEL).contains(&x) => {
                    crate::settings::open_rename(app, x - IDM_PRENAME);
                }
                x if (IDM_PDEL..IDM_PALGN).contains(&x) => {
                    let gi = x - IDM_PDEL;
                    app.delete_panel(gi);
                    app.render_all();
                    dlog(&format!("panel delete gi={gi}"));
                }
                x if (IDM_PALGN..IDM_PALGN + 128).contains(&x) => {
                    let gi = (x - IDM_PALGN) / 4;
                    let a = ((x - IDM_PALGN) % 4) as u8;
                    if a <= 2 {
                        app.set_title_align(gi, a);
                        dlog(&format!("title align gi={gi} a={a}"));
                    }
                }
                x if (IDM_PTITLE..IDM_PTITLE + 100).contains(&x) => {
                    app.toggle_title_show(x - IDM_PTITLE);
                }
                x if (IDM_PHIDE..IDM_PHIDE + 100).contains(&x) => {
                    let gi = x - IDM_PHIDE;
                    if gi < app.groups.len() {
                        app.groups[gi].hidden = true;
                        app.save_config();
                        if !app.groups[gi].hwnd.is_invalid() {
                            let _ = ShowWindow(app.groups[gi].hwnd, SW_HIDE);
                        }
                        dlog(&format!("panel hide gi={gi}"));
                    }
                }
                x if (IDM_PSHOW..IDM_PSHOW + 100).contains(&x) => {
                    let gi = x - IDM_PSHOW;
                    if gi < app.groups.len() && app.groups[gi].hidden {
                        app.groups[gi].hidden = false;
                        app.save_config();
                        if app.groups_visible && !app.groups[gi].hwnd.is_invalid() {
                            let _ = ShowWindow(app.groups[gi].hwnd, SW_RESTORE);
                            app.render_panel(gi);
                            ensure_z_order(app);
                        }
                        dlog(&format!("panel show gi={gi}"));
                    }
                }
                _ => {}
            }
            return LRESULT(0);
        }

        WM_DESTROY => {
            // 正常外部关闭(taskkill/Alt+F4)与退出清理 -> 结束消息循环;
            // 仅 delete_panel 内部销毁自身窗口时不退出程序
            if app.exiting || !app.in_panel_destroy {
                PostQuitMessage(0);
            }
            return LRESULT(0);
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
