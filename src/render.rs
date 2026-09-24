// 渲染:把每个分组面板画进 32 位预乘 alpha 位图,再用 UpdateLayeredWindow 呈现
// 半透明 + 圆角 + 柔和阴影全部由像素绘制完成。
use windows::core::Result;
use windows::Win32::Foundation::{POINT, RECT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT, ID2D1RenderTarget,
};
use windows_numerics::Vector2;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, IDWriteTextLayout,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BI_RGB,
    BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS, HGDIOBJ, AC_SRC_ALPHA,
    AC_SRC_OVER,
};
use windows::Win32::Graphics::Imaging::{
    GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SetWindowPos, UpdateLayeredWindow, SWP_NOACTIVATE, SWP_NOZORDER, ULW_ALPHA,
};

use crate::app::{App, Surface, ICON_SZ, ICON_TOP, MARGIN, PAD};
use windows::Win32::Graphics::Direct2D::D2D1_ANTIALIAS_MODE_PER_PRIMITIVE;

fn color(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

fn inflate(rect: D2D_RECT_F, d: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.left - d,
        top: rect.top - d,
        right: rect.right + d,
        bottom: rect.bottom + d,
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

// ---------------- 位图表面管理 ----------------

impl App {
    fn ensure_surface(surf: &mut Option<Surface>, w: i32, h: i32) -> (*mut u8, windows::Win32::Graphics::Gdi::HDC) {
        let need_new = match surf {
            Some(s) => s.w != w || s.h != h,
            None => true,
        };
        if need_new {
            if let Some(s) = surf.take() {
                unsafe {
                    let _ = SelectObject(s.dc, s.old);
                    let _ = DeleteObject(s.dib.into());
                    let _ = DeleteDC(s.dc);
                }
            }
            unsafe {
                let dc = CreateCompatibleDC(None);
                if dc.is_invalid() {
                    return (std::ptr::null_mut(), Default::default());
                }
                let mut bi: BITMAPINFO = std::mem::zeroed();
                bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                bi.bmiHeader.biWidth = w;
                bi.bmiHeader.biHeight = -h; // 自上而下
                bi.bmiHeader.biPlanes = 1;
                bi.bmiHeader.biBitCount = 32;
                bi.bmiHeader.biCompression = BI_RGB.0;
                bi.bmiHeader.biSizeImage = (w * h * 4) as u32;
                let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
                let dib = match CreateDIBSection(
                    Some(dc),
                    &bi,
                    DIB_RGB_COLORS,
                    &mut bits,
                    None,
                    0,
                ) {
                    Ok(d) => d,
                    Err(_) => {
                        let _ = DeleteDC(dc);
                        return (std::ptr::null_mut(), Default::default());
                    }
                };
                let old = SelectObject(dc, HGDIOBJ(dib.0));
                *surf = Some(Surface { dc, dib, old, bits: bits as *mut u8, w, h });
            }
        }
        let s = surf.as_ref().unwrap();
        (s.bits, s.dc)
    }

    pub fn render_panel(&mut self, gi: usize) {
        if self.groups[gi].hwnd.is_invalid() {
            return;
        }
        // 1) 布局 + 窗口尺寸/位置
        self.layout_group(gi);
        let (gx, gy, pw, ph) = {
            let g = &self.groups[gi];
            (g.gx, g.gy, g.panel_w, g.panel_h)
        };
        let bw = pw + 2 * MARGIN;
        let bh = ph + 2 * MARGIN;
        let hwnd = self.groups[gi].hwnd;
        unsafe {
            let mut cur = RECT::default();
            let want = RECT {
                left: gx - MARGIN,
                top: gy - MARGIN,
                right: gx - MARGIN + bw,
                bottom: gy - MARGIN + bh,
            };
            let mismatch = GetWindowRect(hwnd, &mut cur).is_err()
                || cur.left != want.left
                || cur.top != want.top
                || cur.right - cur.left != bw
                || cur.bottom - cur.top != bh;
            if mismatch {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    gx - MARGIN,
                    gy - MARGIN,
                    bw,
                    bh,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            let g = &mut self.groups[gi];
            g.win_w = bw;
            g.win_h = bh;
        }
        let (bits, hdc_src) = {
            let surf = &mut self.surfaces[gi];
            Self::ensure_surface(surf, bw, bh)
        };
        if bits.is_null() {
            return;
        }
        unsafe {
            std::ptr::write_bytes(bits, 0, (bw * bh * 4) as usize);
            if let Err(e) = self.paint_panel(gi, bits, bw, bh) {
                dlog(&format!("paint_panel({gi}) err: {e}"));
                return;
            }
            {
                let total = (bw * bh) as usize;
                let mut nonzero = 0usize;
                let mut i = 0usize;
                while i < total {
                    if unsafe { *bits.add(i * 4 + 3) } != 0 { nonzero += 1; }
                    i += 1;
                }
                dlog(&format!("paint({gi}) pixels={total} visible={nonzero}"));
            }
            let pt_dst = POINT { x: gx - MARGIN, y: gy - MARGIN };
            let sz = SIZE { cx: bw, cy: bh };
            let pt0 = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            match UpdateLayeredWindow(
                hwnd,
                None,
                Some(&pt_dst),
                Some(&sz),
                Some(hdc_src),
                Some(&pt0),
                Default::default(),
                Some(&blend),
                ULW_ALPHA,
            ) {
                Ok(_) => dlog(&format!("ULW({gi}) ok dst=({},{}), sz=({},{}), bits={:p}", pt_dst.x, pt_dst.y, bw, bh, bits)),
                Err(e) => dlog(&format!("ULW({gi}) err: {e}")),
            }
        }
    }

    pub fn render_all(&mut self) {
        for gi in 0..self.groups.len() {
            self.render_panel(gi);
        }
    }

    // ---------------- 拖拽幽灵窗口 ----------------

    pub fn render_ghost(&mut self) {
        let Some((src_gi, idx)) = self.ghost_item else { return };
        let hwnd = self.ghost;
        if hwnd.is_invalid() {
            return;
        }
        let label: String = self.groups[src_gi].items[idx].name.clone();
        let icon_key: String = self.groups[src_gi].items[idx].key.clone();
        let gw = self.cell_w();
        let gh = ICON_TOP + ICON_SZ + 30;
        let bw = gw + 2 * MARGIN;
        let bh = gh + 2 * MARGIN;

        let (bits, hdc_src) = {
            let surf = &mut self.ghost_surface;
            Self::ensure_surface(surf, bw, bh)
        };
        if bits.is_null() {
            return;
        }
        unsafe {
            std::ptr::write_bytes(bits, 0, (bw * bh * 4) as usize);
            let _ = self.paint_ghost(&icon_key, &label, bits, bw, bh, gw, gh);
            let sz = SIZE { cx: bw, cy: bh };
            let pt0 = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            // 位置:采用当前窗口位置,仅刷新内容
            let mut wr = RECT::default();
            let _ = GetWindowRect(hwnd, &mut wr);
            let pt_dst = POINT { x: wr.left, y: wr.top };
            let _ = UpdateLayeredWindow(
                hwnd,
                None,
                Some(&pt_dst),
                Some(&sz),
                Some(hdc_src),
                Some(&pt0),
                Default::default(),
                Some(&blend),
                ULW_ALPHA,
            );
        }
    }

    // ---------------- 绘制主体 ----------------

    fn make_rt(&self, bits_w: i32, bits_h: i32) -> Result<(windows::Win32::Graphics::Imaging::IWICBitmap, ID2D1RenderTarget)> {
        unsafe {
            let wicbmp = self.wic.CreateBitmap(
                bits_w as u32,
                bits_h as u32,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapCacheOnLoad,
            )?;
            let mut props: D2D1_RENDER_TARGET_PROPERTIES = std::mem::zeroed();
            props.pixelFormat.alphaMode = D2D1_ALPHA_MODE_PREMULTIPLIED;
            let rt = self.d2d.CreateWicBitmapRenderTarget(&wicbmp, &props)?;
            Ok((wicbmp, rt))
        }
    }

    fn flush_rt(
        &self,
        wicbmp: &windows::Win32::Graphics::Imaging::IWICBitmap,
        rt: &ID2D1RenderTarget,
        bits: *mut u8,
        w: i32,
        h: i32,
    ) -> Result<()> {
        unsafe {
            rt.EndDraw(None, None)?;
            let slice = std::slice::from_raw_parts_mut(bits, (w * h * 4) as usize);
            wicbmp.CopyPixels(std::ptr::null(), (w * 4) as u32, slice)?;
        }
        Ok(())
    }

    unsafe fn draw_text_layout(
        rt: &ID2D1RenderTarget,
        dwrite: &windows::Win32::Graphics::DirectWrite::IDWriteFactory,
        fmt: &windows::Win32::Graphics::DirectWrite::IDWriteTextFormat,
        text: &str,
        x: f32,
        y: f32,
        max_w: f32,
        max_h: f32,
        brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
        shadow: bool,
        shadow_brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
    ) -> Result<()> {
        let wt: Vec<u16> = text.encode_utf16().collect();
        if wt.is_empty() {
            return Ok(());
        }
        let layout: IDWriteTextLayout = dwrite.CreateTextLayout(&wt, fmt, max_w, max_h)?;
        let _ = layout.SetTrimming(
            &DWRITE_TRIMMING {
                granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                delimiter: 0,
                delimiterCount: 0,
            },
            None,
        );
        if shadow {
            rt.DrawTextLayout(
                Vector2 { X: x + 1.0, Y: y + 1.0 },
                &layout,
                shadow_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
        }
        rt.DrawTextLayout(
            Vector2 { X: x, Y: y },
            &layout,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
        );
        Ok(())
    }

    fn paint_panel(&self, gi: usize, bits: *mut u8, bw: i32, bh: i32) -> Result<()> {
        let g = &self.groups[gi];
        let radius = self.settings.radius.max(0) as f32;
        let cw = self.cell_w();
        let ch = self.cell_h();
        let (wicbmp, rt) = self.make_rt(bw, bh)?;
        unsafe {
            let _ = rt.BeginDraw();
            let clear = color(0.0, 0.0, 0.0, 0.0);
            rt.Clear(Some(&clear));

            let panel = D2D_RECT_F {
                left: MARGIN as f32,
                top: MARGIN as f32,
                right: (MARGIN + g.panel_w) as f32,
                bottom: (MARGIN + g.panel_h) as f32,
            };

            // 柔和阴影:多圈低透明描边近似
            for i in 0..8 {
                let grow = 3.0 + i as f32 * 2.0;
                let rr = D2D1_ROUNDED_RECT {
                    rect: inflate(panel, grow),
                    radiusX: radius + grow,
                    radiusY: radius + grow,
                };
                let geo = self.d2d.CreateRoundedRectangleGeometry(&rr)?;
                let a = 0.055 * (1.0 - i as f32 / 8.0);
                let pen = rt.CreateSolidColorBrush(&color(0.0, 0.0, 0.0, a), None)?;
                rt.DrawGeometry(&geo, &pen, 3.0, None);
            }

            // 面板本体:半透明深色玻璃
            let rr = D2D1_ROUNDED_RECT { rect: panel, radiusX: radius, radiusY: radius };
            let geo = self.d2d.CreateRoundedRectangleGeometry(&rr)?;
            let fill = rt.CreateSolidColorBrush(&color(0.11, 0.12, 0.145, 0.80), None)?;
            rt.FillGeometry(&geo, &fill, None);

            // 拖放高亮
            if self.mouse.dragging && self.mouse.target == Some(gi) {
                let hl = rt.CreateSolidColorBrush(&color(0.30, 0.62, 1.0, 0.10), None)?;
                rt.FillGeometry(&geo, &hl, None);
            }

            let border = rt.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.18), None)?;
            rt.DrawGeometry(&geo, &border, 1.25, None);
            if self.mouse.dragging && self.mouse.target == Some(gi) {
                let acc = rt.CreateSolidColorBrush(&color(0.35, 0.65, 1.0, 0.95), None)?;
                rt.DrawGeometry(&geo, &acc, 2.5, None);
            }

            // 标题
            let title = format!("{} · {}", g.title, g.items.len());
            let title_brush = rt.CreateSolidColorBrush(&color(0.96, 0.97, 0.99, 1.0), None)?;
            let shadow_brush = rt.CreateSolidColorBrush(&color(0.0, 0.0, 0.0, 0.55), None)?;
            if g.title_show {
                // 标题:按面板设置 左/中/右 对齐
                let wt2: Vec<u16> = title.encode_utf16().collect();
                if !wt2.is_empty() {
                    let talign = match g.title_align {
                        0 => DWRITE_TEXT_ALIGNMENT_LEADING,
                        2 => DWRITE_TEXT_ALIGNMENT_TRAILING,
                        _ => DWRITE_TEXT_ALIGNMENT_CENTER,
                    };
                    let tl: IDWriteTextLayout = self.dwrite.CreateTextLayout(
                        &wt2,
                        &self.fmt_title,
                        (g.panel_w - 2 * PAD) as f32,
                        24.0,
                    )?;
                    let _ = tl.SetTrimming(
                        &DWRITE_TRIMMING {
                            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                            delimiter: 0,
                            delimiterCount: 0,
                        },
                        None,
                    );
                    let _ = tl.SetTextAlignment(talign);
                    rt.DrawTextLayout(
                        Vector2 {
                            X: (MARGIN + PAD) as f32,
                            Y: (MARGIN + 10) as f32,
                        },
                        &tl,
                        &title_brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    );
                }
            }

            if g.items.is_empty() {
                let hint_brush = rt.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.45), None)?;
                Self::draw_text_layout(
                    &rt,
                    &self.dwrite,
                    &self.fmt_hint,
                    crate::lang::t("drag_hint"),
                    (MARGIN + PAD) as f32,
                    (MARGIN + self.grid_top(gi) + 20) as f32,
                    (g.panel_w - 2 * PAD) as f32,
                    30.0,
                    &hint_brush,
                    false,
                    &shadow_brush,
                )?;
            }

            // 图标网格(滚动偏移 + 只在可视行区内绘制)
            {
                let gt0 = self.grid_top(gi);
                let clip = D2D_RECT_F {
                    left: (MARGIN + 2) as f32,
                    top: (MARGIN + gt0) as f32,
                    right: (MARGIN + g.panel_w - 2) as f32,
                    bottom: (MARGIN + gt0 + g.rows.max(1) * ch) as f32,
                };
                let _ = rt.PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            }
            let label_brush = rt.CreateSolidColorBrush(&color(0.93, 0.94, 0.96, 0.97), None)?;
            for idx in 0..g.items.len() {
                let col = (idx as i32) % g.cols;
                let row = (idx as i32) / g.cols;
                // 不截断:滚动后需绘制溢出行,由 PushAxisAlignedClip 裁剪可见区
                let cx = (MARGIN + PAD + col * cw) as f32;
                let sy = g.scroll_y as f32;
                let cy = (MARGIN + self.grid_top(gi) + row * ch) as f32 - sy;
                let is_selected = self.is_sel(gi, idx);
                let is_drag_src = self.mouse.dragging
                    && self.mouse.src == Some(gi)
                    && self.mouse.down == Some((gi, idx));

                if is_selected {
                    let sel = D2D_RECT_F {
                        left: cx + 2.0,
                        top: cy,
                        right: cx + cw as f32 - 2.0,
                        bottom: cy + ICON_TOP as f32 + ICON_SZ as f32 + 26.0,
                    };
                    let rr = D2D1_ROUNDED_RECT { rect: sel, radiusX: 8.0, radiusY: 8.0 };
                    let geo = self.d2d.CreateRoundedRectangleGeometry(&rr)?;
                    let b = rt.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.16), None)?;
                    rt.FillGeometry(&geo, &b, None);
                }

                // 图标(必要时补进缓存)
                let key = &g.items[idx].key;
                let icon_x = cx + (cw - ICON_SZ) as f32 / 2.0;
                let icon_y = cy + ICON_TOP as f32;
                let opacity = if is_drag_src { 0.35 } else { 1.0 };
                // 缓存缺失则由调用方保证已填充;此处仅读取
                let cached = self.icon_cache.get(key).cloned().flatten().or_else(|| {
                    crate::desktop::load_icon(&self.wic, key)
                });
                if let Some(wicb) = cached {
                    if let Ok(bmp) = rt.CreateBitmapFromWicBitmap(&wicb, None) {
                        let dest = D2D_RECT_F {
                            left: icon_x,
                            top: icon_y,
                            right: icon_x + ICON_SZ as f32,
                            bottom: icon_y + ICON_SZ as f32,
                        };
                        let _ = rt.DrawBitmap(
                            &bmp,
                            Some(&dest),
                            opacity,
                            D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                            None,
                        );
                    }
                } else {
                    // 占位:圆角方块 + 首字母
                    let ph = D2D_RECT_F {
                        left: icon_x,
                        top: icon_y,
                        right: icon_x + ICON_SZ as f32,
                        bottom: icon_y + ICON_SZ as f32,
                    };
                    let rr = D2D1_ROUNDED_RECT { rect: ph, radiusX: 10.0, radiusY: 10.0 };
                    let geo = self.d2d.CreateRoundedRectangleGeometry(&rr)?;
                    let b = rt.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.14), None)?;
                    rt.FillGeometry(&geo, &b, None);
                    let ch = g.items[idx].name.chars().next().unwrap_or('?').to_string();
                    let lb = rt.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.85), None)?;
                    Self::draw_text_layout(
                        &rt,
                        &self.dwrite,
                        &self.fmt_title,
                        &ch,
                        icon_x,
                        icon_y + 10.0,
                        ICON_SZ as f32,
                        26.0,
                        &lb,
                        false,
                        &shadow_brush,
                    )?;
                }

                // 标签
                Self::draw_text_layout(
                    &rt,
                    &self.dwrite,
                    &self.fmt_label,
                    &g.items[idx].name,
                    cx + 4.0,
                    cy + ICON_TOP as f32 + ICON_SZ as f32 + 5.0,
                    (cw - 8) as f32,
                    20.0,
                    &label_brush,
                    true,
                    &shadow_brush,
                )?;
            }

            // 组内拖拽落点高亮(含空格)
            if self.mouse.dragging
                && self.mouse.src == Some(gi)
                && self.selected != self.mouse.hover_cell.map(|c| (gi, c))
            {
                if let Some(cell) = self.mouse.hover_cell {
                    let col = (cell as i32) % g.cols.max(1);
                    let row = (cell as i32) / g.cols.max(1);
                    if row < g.rows {
                        let hx = (MARGIN + PAD + col * cw + 2) as f32;
                        let hy = (MARGIN + self.grid_top(gi) + row * ch) as f32 - g.scroll_y as f32;
                        let hr = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: hx,
                                top: hy,
                                right: hx + (cw - 4) as f32,
                                bottom: hy + (ICON_TOP + ICON_SZ + 26) as f32,
                            },
                            radiusX: 8.0,
                            radiusY: 8.0,
                        };
                        let geo = self.d2d.CreateRoundedRectangleGeometry(&hr)?;
                        let b = rt.CreateSolidColorBrush(&color(0.35, 0.65, 1.0, 0.85), None)?;
                        rt.DrawGeometry(&geo, &b, 2.0, None);
                    }
                }
            }

            let _ = rt.PopAxisAlignedClip();

            // 框选矩形
            if let Some(mq) = &self.marquee {
                if mq.active && mq.gi == gi {
                    let rx = mq.x0.min(mq.x1) as f32;
                    let ry = mq.y0.min(mq.y1) as f32;
                    let rw = (mq.x0 - mq.x1).abs() as f32;
                    let rh = (mq.y0 - mq.y1).abs() as f32;
                    let rr = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F { left: rx, top: ry, right: rx + rw, bottom: ry + rh },
                        radiusX: 2.0,
                        radiusY: 2.0,
                    };
                    let geo = self.d2d.CreateRoundedRectangleGeometry(&rr)?;
                    let bf = rt.CreateSolidColorBrush(&color(0.35, 0.65, 1.0, 0.15), None)?;
                    rt.FillGeometry(&geo, &bf, None);
                    let bl = rt.CreateSolidColorBrush(&color(0.35, 0.65, 1.0, 0.8), None)?;
                    rt.DrawGeometry(&geo, &bl, 1.5, None);
                }
            }

            // 垂直滚动条(内容超高时的细轨道+滑块)
            if self.scroll_bar_visible(gi) {
                let (bx, by, bw, bh) = self.scroll_track(gi);
                let track = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: bx as f32,
                        top: by as f32,
                        right: (bx + bw) as f32,
                        bottom: (by + bh) as f32,
                    },
                    radiusX: 3.0,
                    radiusY: 3.0,
                };
                let geo = self.d2d.CreateRoundedRectangleGeometry(&track)?;
                let tb = rt.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.10), None)?;
                rt.FillGeometry(&geo, &tb, None);
                let (tx2, ty2, tw2, th2) = self.scroll_thumb(gi);
                let thumb = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: tx2 as f32,
                        top: ty2 as f32,
                        right: (tx2 + tw2) as f32,
                        bottom: (ty2 + th2) as f32,
                    },
                    radiusX: 3.0,
                    radiusY: 3.0,
                };
                let geo2 = self.d2d.CreateRoundedRectangleGeometry(&thumb)?;
                let tb2 = rt.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.34), None)?;
                rt.FillGeometry(&geo2, &tb2, None);
            }

            // 溢出指示 "+N"(手动改小后提示还有内容;点击展开)
            let cap = g.cols * g.rows;
            let n_total = g.items.len() as i32;
            if n_total > cap {
                let extra = n_total - cap;
                let (cx, cy, cw, chh) = self.chip_rect(gi);
                let rr = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: (MARGIN + cx) as f32,
                        top: (MARGIN + cy) as f32,
                        right: (MARGIN + cx + cw) as f32,
                        bottom: (MARGIN + cy + chh) as f32,
                    },
                    radiusX: 7.0,
                    radiusY: 7.0,
                };
                let geo = self.d2d.CreateRoundedRectangleGeometry(&rr)?;
                let b = rt.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.18), None)?;
                rt.FillGeometry(&geo, &b, None);
                let bn = rt.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.35), None)?;
                rt.DrawGeometry(&geo, &bn, 1.2, None);
                let tb = rt.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.97), None)?;
                let sb2 = rt.CreateSolidColorBrush(&color(0.0, 0.0, 0.0, 0.55), None)?;
                let txt = format!("+{}", extra);
                Self::draw_text_layout(
                    &rt,
                    &self.dwrite,
                    &self.fmt_hint,
                    &txt,
                    (MARGIN + cx) as f32,
                    (MARGIN + cy + 1) as f32,
                    cw as f32,
                    (chh - 2) as f32,
                    &tb,
                    true,
                    &sb2,
                )?;
            }

            self.flush_rt(&wicbmp, &rt, bits, bw, bh)?;
        }
        Ok(())
    }

    fn paint_ghost(
        &self,
        icon_key: &str,
        label: &str,
        bits: *mut u8,
        bw: i32,
        bh: i32,
        gw: i32,
        gh: i32,
    ) -> Result<()> {
        let (wicbmp, rt) = self.make_rt(bw, bh)?;
        unsafe {
            let _ = rt.BeginDraw();
            let clear = color(0.0, 0.0, 0.0, 0.0);
            rt.Clear(Some(&clear));
            let panel = D2D_RECT_F {
                left: MARGIN as f32,
                top: MARGIN as f32,
                right: (MARGIN + gw) as f32,
                bottom: (MARGIN + gh) as f32,
            };
            let rr = D2D1_ROUNDED_RECT { rect: panel, radiusX: 10.0, radiusY: 10.0 };
            let geo = self.d2d.CreateRoundedRectangleGeometry(&rr)?;
            let fill = rt.CreateSolidColorBrush(&color(0.13, 0.14, 0.17, 0.92), None)?;
            rt.FillGeometry(&geo, &fill, None);
            let border = rt.CreateSolidColorBrush(&color(0.35, 0.65, 1.0, 0.9), None)?;
            rt.DrawGeometry(&geo, &border, 1.5, None);

            let icon_x = (MARGIN + (gw - ICON_SZ) / 2) as f32;
            let icon_y = (MARGIN + ICON_TOP) as f32;
            let cached = self.icon_cache.get(icon_key).cloned().flatten().or_else(|| {
                crate::desktop::load_icon(&self.wic, icon_key)
            });
            if let Some(wicb) = cached {
                if let Ok(bmp) = rt.CreateBitmapFromWicBitmap(&wicb, None) {
                    let dest = D2D_RECT_F {
                        left: icon_x,
                        top: icon_y,
                        right: icon_x + ICON_SZ as f32,
                        bottom: icon_y + ICON_SZ as f32,
                    };
                    let _ = rt.DrawBitmap(
                        &bmp,
                        Some(&dest),
                        0.9,
                        D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                        None,
                    );
                }
            }
            let lb = rt.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.95), None)?;
            let sb = rt.CreateSolidColorBrush(&color(0.0, 0.0, 0.0, 0.6), None)?;
            Self::draw_text_layout(
                &rt,
                &self.dwrite,
                &self.fmt_label,
                label,
                (MARGIN + 4) as f32,
                (MARGIN + ICON_TOP + ICON_SZ + 5) as f32,
                (gw - 8) as f32,
                20.0,
                &lb,
                true,
                &sb,
            )?;
            self.flush_rt(&wicbmp, &rt, bits, bw, bh)?;
        }
        Ok(())
    }
}

// 图标缓存补全:渲染前调用
impl App {
    pub fn warm_icons(&mut self) {
        for gi in 0..4 {
            for i in 0..self.groups[gi].items.len() {
                let key = self.groups[gi].items[i].key.clone();
                if !self.icon_cache.contains_key(&key) {
                    let ic = crate::desktop::load_icon(&self.wic, &key);
                    self.icon_cache.insert(key, ic);
                }
            }
        }
    }
}
