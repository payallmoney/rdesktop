// 应用状态:四组模型、布局常量、配置读写
use std::collections::HashMap;
use std::path::PathBuf;

use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Direct2D::ID2D1Factory;
use windows::Win32::Graphics::DirectWrite::{IDWriteFactory, IDWriteTextFormat};
use windows::Win32::Graphics::Gdi::{HBITMAP, HDC, HGDIOBJ};
use windows::Win32::Graphics::Imaging::IWICImagingFactory;

// ---------- 布局常量(物理像素) ----------
pub const MARGIN: i32 = 26; // 阴影留白:窗口位图比面板四周各大这么多
pub const PAD: i32 = 16; // 面板内水平留白
pub const TITLE_H: i32 = 40; // 标题栏高度(标题栏可拖动面板)
pub const GRID_TOP: i32 = 46; // 图标网格起始 y(面板坐标)
// 单元内容尺寸(图标+文字块);格宽/格高 = 内容 + 可配置间距
// 默认间距(列8/行7)与旧常量 CELL_W=140 / CELL_H=86 等价,升级零视觉变化
pub const CONTENT_W: i32 = 132;
pub const CONTENT_H: i32 = 79;
pub const DEFAULT_COL_GAP: i32 = 8;
pub const DEFAULT_ROW_GAP: i32 = 7;
pub const ICON_SZ: i32 = 48;
pub const ICON_TOP: i32 = 6; // 图标在格子内的上边距
pub const BOTTOM_PAD: i32 = 14;
pub const RADIUS: f32 = 14.0; // 面板圆角半径
pub const GAP: i32 = 16; // 面板间距
pub const DRAG_THRESHOLD: i32 = 5;

pub const GROUP_TITLES: [&str; 4] = ["文件夹", "文件", "快捷方式", "其他快捷功能"];

// 托盘 / 菜单 ID
pub const WM_TRAY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;
pub const TIMER_ZORDER: usize = 1;
pub const IDM_REFRESH: usize = 1001;
pub const IDM_TOGGLE_GROUPS: usize = 1002;
pub const IDM_TOGGLE_ORIG: usize = 1003;
pub const IDM_EXIT: usize = 1004;
pub const IDM_OPEN: usize = 1101;
pub const IDM_SETTINGS: usize = 1010;
// 面板右键菜单(WM_COMMAND 分发,便于自动化)
pub const IDM_PNEW: usize = 1200; // 新建面板
pub const IDM_PRENAME: usize = 1210; // +gi  重命名
pub const IDM_PDEL: usize = 1300; // +gi  关闭面板(仅用户自建)
pub const IDM_PALGN: usize = 1400; // +gi*4+a  标题对齐 a:0左1中2右
pub const WM_OPEN_SETTINGS: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;
pub const WM_Z_PAUSE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 3; // 测试用:暂停 z 序维护
pub const WM_Z_REZ: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 5; // 事件驱动:立即纠正 z 序/救回最小化
/// hover 状态:0=无;HOVER_TITLE=标题(移动);其余为 HTLEFT/HTRIGHT/...(缩放热区)
pub const HOVER_TITLE: u32 = 0x1000;

fn min_h_const(ch: i32) -> i32 {
    GRID_TOP + ch + BOTTOM_PAD
}

pub fn ws(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[derive(Clone)]
pub struct Item {
    pub key: String,  // 完整路径,或 "@builtin:*"
    pub name: String, // 显示名(.lnk 已去掉扩展名)
    pub rule: u8,     // 按类型分组的默认组号
    pub launch: Option<(String, String)>, // (exe, args),仅内置项
}

impl Item {
    pub fn is_builtin(&self) -> bool {
        self.key.starts_with('@')
    }
}

pub struct Group {
    pub title: String,
    pub title_align: u8, // 标题对齐:0=左 1=中(默认) 2=右
    pub items: Vec<Item>,
    pub hwnd: HWND,
    pub gx: i32,
    pub gy: i32, // 面板左上角(屏幕坐标,不含阴影留白)
    pub cols: i32,
    pub rows: i32,
    pub panel_w: i32,
    pub panel_h: i32,
    pub win_w: i32,
    pub win_h: i32, // 当前窗口(位图)尺寸
    pub user_pos: bool, // 位置是否被用户拖动过
    pub z_top: bool, // 图层:false=最低层(桌面之上、应用之下,默认);true=最高层(置顶)
    pub user_size: bool, // 用户手动调整过大小
    pub manual_w: i32,
    pub manual_h: i32,
    pub manual_ord: bool, // 组内顺序已被用户手动排过(优先于自动排序)
}

pub struct Surface {
    pub dc: HDC,
    pub dib: HBITMAP,
    pub old: HGDIOBJ,
    pub bits: *mut u8,
    pub w: i32,
    pub h: i32,
}

/// 手动缩放状态(自实现的边缘拖拽,客户区坐标增量法)
#[derive(Default)]
pub struct ResizeState {
    pub gi: usize,
    pub zone: u32, // HTLEFT/HTRIGHT/... 作为区域标识
    pub start_px: i32,
    pub start_py: i32,
    pub win_l: i32,
    pub win_t: i32,
    pub win_r: i32,
    pub win_b: i32,
}

/// 手动移动状态(标题栏自实现拖动,替代系统 HTCAPTION 模态循环)
#[derive(Default)]
pub struct MoveState {
    pub gi: usize,
    pub start_px: i32,
    pub start_py: i32,
    pub win_l: i32,
    pub win_t: i32,
}

#[derive(Default)]
pub struct Mouse {
    pub down: Option<(usize, usize)>, // 按下的 (组, 项)
    pub down_pt: POINT,
    pub dragging: bool,
    pub src: Option<usize>,
    pub target: Option<usize>, // 拖拽悬停的目标组
    pub hover_cell: Option<usize>, // 组内拖拽悬停的目标格(可能为空格)
}

impl Group {
    pub fn empty(title: String) -> Group {
        Group {
            title,
            items: Vec::new(),
            hwnd: HWND::default(),
            gx: 0,
            gy: 0,
            cols: 2,
            rows: 1,
            panel_w: 0,
            panel_h: 0,
            win_w: 0,
            win_h: 0,
            user_pos: false,
            z_top: false,
            user_size: false,
            manual_w: 0,
            manual_h: 0,
            manual_ord: false,
            title_align: 1,
        }
    }
}

/// 用户可配置项(设置窗口)
#[derive(Clone, PartialEq)]
pub struct Settings {
    pub frosted: bool,   // 毛玻璃(DWM 背景模糊)
    pub radius: i32,     // 圆角半径 px(0=直角)
    pub auto_tidy: bool, // 自动整理(自动分组、按名称排序、自动刷新)
    pub snap_grid: i32,  // 对齐:拖动面板后吸附网格 px(0=关)

    pub col_gap: i32, // 列间距
    pub row_gap: i32, // 行间距
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            frosted: false,
            radius: 14,
            auto_tidy: true,
            snap_grid: 0,
            col_gap: DEFAULT_COL_GAP,
            row_gap: DEFAULT_ROW_GAP,
        }
    }
}

pub struct App {
    pub groups: Vec<Group>,

    // 渲染设施
    pub wic: IWICImagingFactory,
    pub d2d: ID2D1Factory,
    pub dwrite: IDWriteFactory,
    pub fmt_title: IDWriteTextFormat,
    pub fmt_label: IDWriteTextFormat,
    pub fmt_hint: IDWriteTextFormat,
    pub icon_cache: HashMap<String, Option<windows::Win32::Graphics::Imaging::IWICBitmap>>,
    pub surfaces: Vec<Option<Surface>>, // 0..4 四个面板
    pub ghost_surface: Option<Surface>,
    pub ghost: HWND,
    pub ghost_item: Option<(usize, usize)>,

    // 交互
    pub mouse: Mouse,
    pub selected: Option<(usize, usize)>,

    // 数据 / 配置
    pub settings: Settings,
    pub saved_order: HashMap<usize, Vec<String>>,
    pub overrides: HashMap<String, usize>,
    pub settings_hwnd: HWND,
    pub name_hwnd: HWND,
    pub name_target: Option<usize>,
    pub exiting: bool,
    pub in_panel_destroy: bool,
    pub in_size_move: bool,
    pub z_pause: bool,
    pub rez_last: Option<std::time::Instant>,
    pub settle_left: u32,
    pub hover: u32,
    pub rs: Option<ResizeState>,
    pub mv: Option<MoveState>,
    pub settings_font: windows::Win32::Graphics::Gdi::HFONT,
    pub config_path: PathBuf,
    pub desktop_lv: HWND,
    pub show_original: bool, // 是否显示系统原桌面图标
    pub groups_visible: bool,
    pub work: RECT,
    pub dpi: u32,
    pub tray_added: bool,
    pub last_mtime: Option<std::time::SystemTime>,
    pub scale: f32, // dpi / 96.0
}

impl App {
    pub fn scale_f(&self, v: f32) -> f32 {
        v * self.scale
    }

    /// 当前列宽 = 内容宽 + 列间距
    pub fn cell_w(&self) -> i32 {
        CONTENT_W + self.settings.col_gap.clamp(0, 48)
    }

    /// 当前行高 = 内容高 + 行间距
    pub fn cell_h(&self) -> i32 {
        CONTENT_H + self.settings.row_gap.clamp(0, 40)
    }

    pub fn new() -> Result<App, String> {
        use windows::core::w;
        use windows::Win32::Graphics::Direct2D::{D2D1CreateFactory, ID2D1Factory};
        use windows::Win32::Graphics::DirectWrite::{
            DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWriteCreateFactory,
            IDWriteFactory,
        };
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY, MONITORINFO,
        };
        use windows::Win32::UI::HiDpi::GetDpiForSystem;

        unsafe {
            // 渲染设施
            let wic = crate::desktop::create_wic().map_err(|e| format!("WIC 初始化失败: {e}"))?;
            let d2d: ID2D1Factory =
                D2D1CreateFactory(windows::Win32::Graphics::Direct2D::D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
                    .map_err(|e| format!("Direct2D 初始化失败: {e}"))?;
            let dwrite: IDWriteFactory =
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).map_err(|e| format!("DirectWrite 初始化失败: {e}"))?;

            let scale = GetDpiForSystem() as f32 / 96.0;
            let fmt_title = dwrite
                .CreateTextFormat(
                    w!("Microsoft YaHei UI"),
                    None::<&windows::Win32::Graphics::DirectWrite::IDWriteFontCollection>,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    13.5 * scale,
                    w!(""),
                )
                .map_err(|e| format!("字体创建失败: {e}"))?;
            let fmt_label = dwrite
                .CreateTextFormat(
                    w!("Microsoft YaHei UI"),
                    None::<&windows::Win32::Graphics::DirectWrite::IDWriteFontCollection>,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    12.0 * scale,
                    w!(""),
                )
                .map_err(|e| format!("字体创建失败: {e}"))?;
            // 图标下方名称:居中
            let _ = fmt_label.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
            let fmt_hint = dwrite
                .CreateTextFormat(
                    w!("Microsoft YaHei UI"),
                    None::<&windows::Win32::Graphics::DirectWrite::IDWriteFontCollection>,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    12.0 * scale,
                    w!(""),
                )
                .map_err(|e| format!("字体创建失败: {e}"))?;
            use windows::Win32::Graphics::DirectWrite::{
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
            };
            let _ = fmt_hint.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
            let _ = fmt_hint.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);

            // 工作区 + DPI
            let mon = MonitorFromPoint(
                windows::Win32::Foundation::POINT { x: 0, y: 0 },
                MONITOR_DEFAULTTOPRIMARY,
            );
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let work = if GetMonitorInfoW(mon, &mut mi).as_bool() {
                mi.rcWork
            } else {
                windows::Win32::Foundation::RECT { left: 0, top: 0, right: 1920, bottom: 1080 }
            };

            // 配置路径
            let cfg_dir = std::env::var("USERPROFILE")
                .map(std::path::PathBuf::from)
                .map(|p| p.join("AppData\\Roaming\\rdesktop"))
                .map_err(|_| "无法定位配置目录".to_string())?;
            let _ = std::fs::create_dir_all(&cfg_dir);
            let config_path = cfg_dir.join("config.cfg");

            let empty_groups: [Group; 4] = std::array::from_fn(|i| Group {
                title: GROUP_TITLES[i].to_string(),
                title_align: 1,
                items: Vec::new(),
                hwnd: HWND::default(),
                gx: 0,
                gy: 0,
                cols: 2,
                rows: 1,
                panel_w: 0,
                panel_h: 0,
                win_w: 0,
                win_h: 0,
                user_pos: false,
                z_top: false,
                user_size: false,
                manual_w: 0,
                manual_h: 0,
                manual_ord: false,
            });

            let mut app = App {
                settings: Settings::default(),
                saved_order: HashMap::new(),
                settings_hwnd: HWND::default(),
                name_hwnd: HWND::default(),
                name_target: None,
                exiting: false,
                in_panel_destroy: false,
                in_size_move: false,
                z_pause: false,
                rez_last: None,
                settle_left: 0,
                hover: 0,
                rs: None,
                mv: None,
                settings_font: Default::default(),
                groups: Vec::from(empty_groups),
                wic,
                d2d,
                dwrite,
                fmt_title,
                fmt_label,
                fmt_hint,
                icon_cache: Default::default(),
                surfaces: (0..4).map(|_| None).collect(),
                ghost_surface: None,
                ghost: HWND::default(),
                ghost_item: None,
                mouse: Default::default(),
                selected: None,
                overrides: Default::default(),
                config_path,
                desktop_lv: HWND::default(),
                show_original: false,
                groups_visible: true,
                work,
                dpi: GetDpiForSystem(),
                tray_added: false,
                last_mtime: None,
                scale,
            };
            {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true)
                    .open(std::env::temp_dir().join("rdesktop.log")) {
                    let _ = writeln!(f, "work=({},{})-({},{}) dpi={} scale={:.2} mon={:?}",
                        work.left, work.top, work.right, work.bottom, GetDpiForSystem(), scale, mon);
                }
            }
            app.load_config();
            app.refresh_items();
            app.place_groups();
            app.warm_icons();
            Ok(app)
        }
    }

    pub fn refresh_from_disk(&mut self) {
        self.refresh_items();
        self.warm_icons();
        self.render_all();
        crate::settings::apply_frosted(self);
    }

    // ---------- 单组布局 ----------
    pub fn layout_group(&mut self, gi: usize) {
        let cw = self.cell_w();
        let ch = self.cell_h();
        // 手动调整过大小:以手动尺寸为准(下限保证放下至少一格)
        if self.groups[gi].user_size {
            let min_w = 2 * PAD + cw;
            let min_h = GRID_TOP + ch + BOTTOM_PAD;
            let max_w = (self.work.right - self.work.left) - 4;
            let max_h = (self.work.bottom - self.work.top) - 4;
            let g = &mut self.groups[gi];
            let w = g.manual_w.clamp(min_w, max_w);
            let h = g.manual_h.clamp(min_h, max_h);
            g.panel_w = w;
            g.panel_h = h;
            g.cols = ((w - 2 * PAD) / cw).max(1);
            g.rows = ((h - GRID_TOP - BOTTOM_PAD) / ch).max(1);
            return;
        }
        let work = self.work;
        let n = self.groups[gi].items.len() as i32;
        let max_panel_h = (work.bottom - work.top) - 2 * MARGIN - 8;
        let rows_max = ((max_panel_h - GRID_TOP - BOTTOM_PAD) / ch).max(1);
        let mut cols = if n == 0 {
            2
        } else {
            (n + rows_max - 1) / rows_max
        };
        cols = cols.max(2);
        let cols_max = (((work.right - work.left) - 2 * MARGIN - 2 * PAD) / cw).max(2);
        cols = cols.min(cols_max);
        let rows = if n == 0 { 1 } else { (n + cols - 1) / cols };
        let g = &mut self.groups[gi];
        g.cols = cols;
        g.rows = rows;
        g.panel_w = PAD * 2 + cols * cw;
        g.panel_h = GRID_TOP + rows * ch + BOTTOM_PAD;
    }

    // ---------- 流式摆位(仅未保存过位置的组) ----------
    pub fn place_groups(&mut self) {
        let work = self.work;
        let mut x = work.left + 12;
        let mut y = work.top + 12;
        let mut row_h = 0i32;
        for gi in 0..self.groups.len() {
            self.layout_group(gi);
            let (w, h) = (self.groups[gi].panel_w, self.groups[gi].panel_h);
            if self.groups[gi].user_pos {
                // 已保存位置:参与行高统计但不重新摆
                row_h = row_h.max(h);
                continue;
            }
            if x + w > work.right - 8 {
                y += row_h + GAP;
                x = work.left + 12;
                row_h = 0;
            }
            self.groups[gi].gx = x;
            self.groups[gi].gy = y;
            x += w + GAP;
            row_h = row_h.max(h);
        }
        // 位置钳制在屏幕内
        for gi in 0..self.groups.len() {
            let g = &mut self.groups[gi];
            let max_x = work.right - g.panel_w - 4;
            let max_y = work.bottom - g.panel_h - 4;
            g.gx = g.gx.clamp(work.left + 2, max_x.max(work.left + 2));
            g.gy = g.gy.clamp(work.top + 2, max_y.max(work.top + 2));
        }
    }

    // ---------- 分组数据 ----------
    pub fn refresh_items(&mut self) {
        let mut items = crate::desktop::enumerate_items();
        crate::desktop::push_builtin_items(&mut items);
        // 应用覆盖
        let mut groups: Vec<Vec<Item>> = vec![Vec::new(); self.groups.len()];
        for it in items {
            let mut gi = self.overrides.get(&it.key).map(|&g| g).unwrap_or(it.rule as usize);
            gi = gi.min(groups.len().saturating_sub(1));
            groups[gi].push(it);
        }
        for gi in 0..self.groups.len() {
            let arr = &mut groups[gi];
            let use_saved = self.groups[gi].manual_ord
                || (!self.settings.auto_tidy && self.saved_order.contains_key(&gi));
            if use_saved {
                // 手动序优先:保存过的顺序在前,新条目按名称排在末尾
                let order = self.saved_order.get(&gi);
                if let Some(order) = order {
                    let pos =
                        |k: &String| order.iter().position(|x| x == k).unwrap_or(usize::MAX);
                    arr.sort_by(|a, b| (pos(&a.key), a.name.to_lowercase())
                        .cmp(&(pos(&b.key), b.name.to_lowercase())));
                } else {
                    arr.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                }
            } else if self.settings.auto_tidy {
                // 自动整理:按显示名排序
                arr.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            }
        }
        for gi in 0..self.groups.len() {
            self.groups[gi].items = std::mem::take(&mut groups[gi]);
        }
    }

    /// 把组内条目从 idx 移到 cell(格位,可为空格末尾)—— 手动排序/调换位置
    pub fn reorder_in_group(&mut self, gi: usize, idx: usize, cell: usize) {
        if idx >= self.groups[gi].items.len() || cell == idx {
            return;
        }
        let it = self.groups[gi].items.remove(idx);
        let pos = cell.min(self.groups[gi].items.len());
        self.groups[gi].items.insert(pos, it);
        self.groups[gi].manual_ord = true;
        self.sync_order(gi);
        // 选中跟随
        if self.selected == Some((gi, idx)) {
            self.selected = Some((gi, pos));
        }
        self.save_config();
    }

    /// 手动序组:把当前条目顺序写回 saved_order(供刷新/重启保持)
    fn sync_order(&mut self, gi: usize) {
        if self.groups[gi].manual_ord {
            let keys: Vec<String> = self.groups[gi].items.iter().map(|i| i.key.clone()).collect();
            self.saved_order.insert(gi, keys);
        }
    }

    /// 新建面板(空组,自动摆位),返回其 gi
    pub fn new_panel(&mut self) -> usize {
        let mut k = 1;
        let mut title = "新面板".to_string();
        while self.groups.iter().any(|g| g.title == title) {
            k += 1;
            title = format!("新面板{}", k);
        }
        self.groups.push(Group::empty(title));
        self.surfaces.push(None);
        self.place_groups();
        self.save_config();
        self.groups.len() - 1
    }

    pub fn rename_panel(&mut self, gi: usize, name: &str) {
        if gi >= self.groups.len() {
            return;
        }
        let t: String = name
            .chars()
            .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
            .collect();
        let t = t.trim().to_string();
        if t.is_empty() {
            return;
        }
        self.groups[gi].title = t;
        self.save_config();
        if gi < self.surfaces.len() {
            self.render_panel(gi);
        }
    }

    /// 关闭面板(仅用户自建;组内条目按默认规则回迁)
    pub fn delete_panel(&mut self, gi: usize) {
        if gi < 4 || gi >= self.groups.len() {
            return;
        }
        let keys: Vec<String> = self.groups[gi].items.iter().map(|i| i.key.clone()).collect();
        self.overrides.retain(|k, _| !keys.contains(k));
        // 销毁该面板窗口(WM_DESTROY 会因 exiting=false 而不会触发退出)
        let hwp = self.groups[gi].hwnd;
        if !hwp.is_invalid() {
            self.in_panel_destroy = true;
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwp);
            }
            self.in_panel_destroy = false;
        }
        self.groups.remove(gi);
        if gi < self.surfaces.len() {
            self.surfaces.remove(gi);
        }
        for v in self.overrides.values_mut() {
            if *v > gi {
                *v -= 1;
            }
        }
        let mut new_ord = std::collections::HashMap::new();
        for (k, order) in std::mem::take(&mut self.saved_order) {
            if k == gi {
                continue;
            }
            let nk = if k > gi { k - 1 } else { k };
            new_ord.insert(nk, order);
        }
        self.saved_order = new_ord;
        self.rs = None;
        self.mv = None;
        self.hover = 0;
        self.selected = None;
        self.refresh_items();
        self.place_groups();
        self.save_config();
    }

    pub fn set_title_align(&mut self, gi: usize, v: u8) {
        if gi >= self.groups.len() {
            return;
        }
        self.groups[gi].title_align = v.min(2);
        self.save_config();
        if gi < self.surfaces.len() {
            self.render_panel(gi);
        }
    }

    pub fn move_item(&mut self, from: usize, idx: usize, to: usize) {
        if from == to || idx >= self.groups[from].items.len() {
            return;
        }
        let it = self.groups[from].items.remove(idx);
        let key = it.key.clone();
        let rule = it.rule;
        self.groups[to].items.push(it);
        if self.settings.auto_tidy {
            self.groups[to]
                .items
                .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
        // 更新覆盖表:与默认规则一致则移除记录
        let default_g = rule as usize;
        if to == default_g {
            self.overrides.remove(&key);
        } else {
            self.overrides.insert(key, to);
        }
        self.sync_order(from);
        self.sync_order(to);
        self.save_config();
    }

    // ---------- 配置 ----------
    pub fn load_config(&mut self) {
        let Ok(text) = std::fs::read_to_string(&self.config_path) else {
            return;
        };
        let lines: Vec<String> = text.lines().map(str::to_string).collect();
        // 第一遍:面板列表(g 行)先落地,后续行的 gi 才有效
        for line in &lines {
            let parts: Vec<&str> = line.split('\t').collect();
            if let ["g", g, title] = parts.as_slice() {
                if let Ok(gi) = g.parse::<usize>() {
                    while self.groups.len() <= gi {
                        let idx = self.groups.len();
                        self.groups.push(Group::empty(format!(
                            "新面板{}",
                            idx.saturating_sub(3)
                        )));
                        self.surfaces.push(None);
                    }
                    self.groups[gi].title = (*title).to_string();
                }
            }
        }
        for line in &lines {
            let parts: Vec<&str> = line.split('\t').collect();
            match parts.as_slice() {
                ["m", g, key] => {
                    if let Ok(gi) = g.parse::<usize>() {
                        self.overrides.insert(key.to_string(), gi.min(3));
                    }
                }
                ["s", key, val] => {
                    let v = val.parse::<i32>().unwrap_or(0);
                    match *key {
                        "frosted" => self.settings.frosted = v != 0,
                        "radius" => self.settings.radius = v.clamp(0, 32),
                        "tidy" => self.settings.auto_tidy = v != 0,
                        "grid" => {
                            self.settings.snap_grid = if v <= 0 { 0 } else { v.clamp(4, 64) }
                        }
                        "colgap" => self.settings.col_gap = v.clamp(0, 48),
                        "rowgap" => self.settings.row_gap = v.clamp(0, 40),
                        _ => {}
                    }
                }
                ["mo", g] => {
                    if let Ok(gi) = g.parse::<usize>() {
                        if gi < self.groups.len() {
                            self.groups[gi].manual_ord = true;
                        }
                    }
                }
                ["oi", g, keys @ ..] => {
                    if let Ok(gi) = g.parse::<usize>() {
                        if gi < self.groups.len() {
                            self.saved_order.insert(
                                gi,
                                keys.iter().map(|k| k.to_string()).collect(),
                            );
                        }
                    }
                }
                ["z", g, v] => {
                    if let Ok(gi) = g.parse::<usize>() {
                        if gi < self.groups.len() {
                            self.groups[gi].z_top = v.parse::<i32>().unwrap_or(0) != 0;
                        }
                    }
                }
                ["q", g, w, h] => {
                    if let (Ok(gi), Ok(w), Ok(h)) =
                        (g.parse::<usize>(), w.parse::<i32>(), h.parse::<i32>())
                    {
                        if gi < self.groups.len() {
                            self.groups[gi].user_size = true;
                            self.groups[gi].manual_w = w;
                            self.groups[gi].manual_h = h;
                        }
                    }
                }
                ["ta", g, v] => {
                    if let Ok(gi) = g.parse::<usize>() {
                        if gi < self.groups.len() {
                            self.groups[gi].title_align = v.parse::<u8>().unwrap_or(1).min(2);
                        }
                    }
                }
                ["p", g, x, y, u] => {
                    if let (Ok(gi), Ok(x), Ok(y), Ok(u)) =
                        (g.parse::<usize>(), x.parse::<i32>(), y.parse::<i32>(), u.parse::<u8>())
                    {
                        if gi < self.groups.len() {
                            self.groups[gi].gx = x;
                            self.groups[gi].gy = y;
                            self.groups[gi].user_pos = u == 1;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub fn save_config(&self) {
        let mut out = String::from("rdesktop-config-1\n");
        // 面板列表与标题(置于最前,便于加载时先落地动态面板)
        for (gi, g) in self.groups.iter().enumerate() {
            let t: String = g.title.chars().filter(|c| !matches!(c, '\t' | '\n' | '\r')).collect();
            out.push_str(&format!("g\t{}\t{}\n", gi, t));
            out.push_str(&format!("ta\t{}\t{}\n", gi, g.title_align));
        }
        for gi in 0..self.groups.len() {
            for it in &self.groups[gi].items {
                if it.rule as usize != gi {
                    out.push_str(&format!("m\t{}\t{}\n", gi, it.key));
                }
            }
        }
        // 设置项
        out.push_str(&format!("s\tfrosted\t{}\n", i32::from(self.settings.frosted)));
        out.push_str(&format!("s\tradius\t{}\n", self.settings.radius));
        out.push_str(&format!("s\ttidy\t{}\n", i32::from(self.settings.auto_tidy)));
        out.push_str(&format!("s\tgrid\t{}\n", self.settings.snap_grid));
        out.push_str(&format!("s\tcolgap\t{}\n", self.settings.col_gap));
        out.push_str(&format!("s\trowgap\t{}\n", self.settings.row_gap));
        // 手动序标记 + 需要持久化的组顺序
        //  oi:手动排序过的组,或"自动整理关闭"时的全部组(保持旧版本行为)
        for gi in 0..self.groups.len() {
            if self.groups[gi].manual_ord {
                out.push_str(&format!("mo\t{}\n", gi));
            }
        }
        for gi in 0..self.groups.len() {
            let persist = self.groups[gi].manual_ord || !self.settings.auto_tidy;
            if !persist {
                continue;
            }
            let mut line = format!("oi\t{}\t", gi);
            for it in &self.groups[gi].items {
                line.push_str(&it.key);
                line.push('\t');
            }
            out.push_str(&line);
            out.push('\n');
        }
        for gi in 0..self.groups.len() {
            out.push_str(&format!("z\t{}\t{}\n", gi, i32::from(self.groups[gi].z_top)));
        }
        for gi in 0..self.groups.len() {
            let g = &self.groups[gi];
            if g.user_size {
                out.push_str(&format!("q\t{}\t{}\t{}\n", gi, g.panel_w, g.panel_h));
            }
        }
        for gi in 0..self.groups.len() {
            let g = &self.groups[gi];
            out.push_str(&format!(
                "p\t{}\t{}\t{}\t{}\n",
                gi,
                g.gx,
                g.gy,
                if g.user_pos { 1 } else { 0 }
            ));
        }
        let _ = std::fs::write(&self.config_path, out);
    }

    // ---------- 命中测试(客户区坐标) ----------
    /// 网格放不下的条目数
    pub fn overflow(&self, gi: usize) -> i32 {
        let g = &self.groups[gi];
        (g.items.len() as i32 - g.cols * g.rows).max(0)
    }

    /// 溢出指示条 "+N" 矩形(面板坐标)
    pub fn chip_rect(&self, gi: usize) -> (i32, i32, i32, i32) {
        let g = &self.groups[gi];
        (g.panel_w - PAD - 64, g.panel_h - 34, 64, 22)
    }

    pub fn chip_at(&self, gi: usize, x: i32, y: i32) -> bool {
        if self.overflow(gi) <= 0 {
            return false;
        }
        let (cx, cy, cw, chh) = self.chip_rect(gi);
        let px = x - MARGIN;
        let py = y - MARGIN;
        px >= cx && py >= cy && px <= cx + cw && py <= cy + chh
    }

    /// 点击 "+N":按当前列数增高到能放下全部条目(只放大)
    pub fn expand_to_fit(&mut self, gi: usize) {
        let ch = self.cell_h();
        let n = self.groups[gi].items.len() as i32;
        let cols = self.groups[gi].cols.max(1);
        let rows = (n + cols - 1) / cols;
        let need = GRID_TOP + rows * ch + BOTTOM_PAD;
        let max_h = (self.work.bottom - self.work.top) - 4;
        let cur = self.groups[gi].panel_h;
        let g = &mut self.groups[gi];
        g.user_size = true;
        g.manual_h = need.max(cur).clamp(min_h_const(ch), max_h);
        g.manual_w = g.panel_w;
        self.save_config();
    }

    /// 格位序号(不含"是否有条目"判断;超出条目的为空格,用于组内拖放落点)
    pub fn cell_index(&self, gi: usize, x: i32, y: i32) -> Option<usize> {
        let g = &self.groups[gi];
        let cw = self.cell_w();
        let ch = self.cell_h();
        let px = x - MARGIN;
        let py = y - MARGIN;
        if px < PAD || py < GRID_TOP {
            return None;
        }
        let col = (px - PAD) / cw;
        let row = (py - GRID_TOP) / ch;
        if col < 0 || col >= g.cols || row < 0 || row >= g.rows {
            return None;
        }
        Some((row * g.cols + col) as usize)
    }

    pub fn item_at(&self, gi: usize, x: i32, y: i32) -> Option<usize> {
        let g = &self.groups[gi];
        let cw = self.cell_w();
        let ch = self.cell_h();
        let px = x - MARGIN;
        let py = y - MARGIN;
        if px < PAD || py < GRID_TOP {
            return None;
        }
        let col = (px - PAD) / cw;
        let row = (py - GRID_TOP) / ch;
        if col < 0 || col >= g.cols || row < 0 || row >= g.rows {
            return None;
        }
        let inx = (px - PAD) % cw;
        let iny = (py - GRID_TOP) % ch;
        if inx > cw || iny > ch {
            return None;
        }
        let idx = (row * g.cols + col) as usize;
        if idx < g.items.len() {
            Some(idx)
        } else {
            None
        }
    }

    pub fn icon_rect(&self, gi: usize, idx: usize) -> (i32, i32) {
        let g = &self.groups[gi];
        let cw = self.cell_w();
        let ch = self.cell_h();
        let col = (idx as i32) % g.cols;
        let row = (idx as i32) / g.cols;
        (
            MARGIN + PAD + col * cw + (cw - ICON_SZ) / 2,
            MARGIN + GRID_TOP + row * ch + ICON_TOP,
        )
    }
}
