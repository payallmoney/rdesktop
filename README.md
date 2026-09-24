<div align="center">

# rdesktop

**Windows 桌面分组管理工具** · **Windows Desktop Group Manager**

[中文](#中文) | [English](#english)

![Rust](https://img.shields.io/badge/rust-stable-orange)
![Platform](https://img.shields.io/badge/platform-Windows%2010%2B-blue)
![Version](https://img.shields.io/badge/version-1.0.0-green)
![License](https://img.shields.io/badge/license-MIT-yellow)

*半透明圆角分组面板 · 桌面图标接管 · 拖拽分组 · 框选多选*
*Semi-transparent rounded panels · Desktop icon takeover · Drag & drop · Marquee selection*

</div>

---

## 中文

### 简介

隐藏系统桌面图标(`SysListView32` 层),用**半透明、圆角、带柔和阴影的分组面板**重新呈现桌面内容。面板之间自由拖拽图标、框选多选、双击打开,所有状态持久化于注册表。

| 分组 | 内容 |
|---|---|
| 文件夹 | 桌面上的所有文件夹 |
| 文件 | 非快捷方式的普通文件 |
| 快捷方式 | `.lnk` / `.url` / `.appref-ms` |
| 快捷功能 | 回收站、此电脑、网络、控制面板、设置 |

### 截图

| 文件夹 | 文件(带滚动条) |
|---|---|
| ![文件夹面板](screenshots/panel-folders.png) | ![文件面板](screenshots/panel-files.png) |

| 快捷方式 | 设置窗口 |
|---|---|
| ![快捷方式面板](screenshots/panel-shortcuts.png) | ![设置窗口](screenshots/settings.png) |

> 以上截图均为测试数据,不含真实文件。更多见 [screenshots/](screenshots/)。

### 功能

**面板操作**

- **拖拽分组** — 面板间拖拽图标重新分组;组内拖拽排序
- **框选** — 空白处拖拽框选多个图标
- **Ctrl/Shift 多选** — Ctrl 点击切换选中,Shift 点击范围选择
- **双击打开** — 单个或全部选中项
- **右键菜单** — 与资源管理器一致的原生菜单(IContextMenu)
- **垂直滚动条** — 高度不够时自动出现,支持滚轮和滑块拖动
- **`+N` 溢出提示** — 放不下时显示,点击展开

**面板管理**

- **自由拖动** — 标题栏拖拽,屏幕坐标增量法(不抖不弹)
- **自由缩放** — 四边四角拖拽,光标方向反馈
- **每面板独立设置** — 图层(最低/最高)、标题显示、标题对齐、隐藏
- **新建/重命名/删除面板**
- **叠放偏好** — 点击标题提层的顺序被记住(跨 Win+D / 重启)

**系统集成**

- **Win+D 防护** — 事件驱动 + 幂等锚定 + 锚点黑名单回退链,面板不消失
- **开机自启动** — 托盘勾选,写入 HKCU Run
- **注册表持久化** — 全部状态存 `HKCU\Software\rdesktop`,无需管理员
- **托盘图标** — 设置 / 刷新 / 显示隐藏 / 自启动 / 退出恢复

**外观**

- 毛玻璃(DWM Blur)、圆角像素直输、行列间距像素直输

### 快速开始

```
git clone https://github.com/你的用户名/rdesktop.git
cd rdesktop
cargo build --release
target\release\rdesktop.exe
```

### 设置

| 配置项 | 说明 |
|---|---|
| 毛玻璃 | DWM 背景模糊 |
| 圆角(px) | 0–64,直接输入 |
| 自动整理 | 自动分组+排序+桌面变化刷新 |
| 显示面板标题 | 批量默认,每面板可单独覆盖 |
| 对齐 | 关闭/8/16/32px 网格吸附 |
| 列间距/行间距(px) | 0–48/0–40,直接输入 |
| 图层(每面板) | 最低层(默认)/最高层 |

### 架构

```
src/main.rs      入口:DPI、单实例、隐藏图标、消息循环
src/app.rs       状态:分组模型、布局、注册表读写、命中
src/desktop.rs   Shell 交互:图标枚举/提取/打开/原生菜单
src/render.rs    Direct2D 渲染:圆角/阴影/图标/文字/滚动条
src/regstore.rs  注册表持久化
src/panel.rs     窗口过程:交互/z序/托盘/滚动
src/settings.rs  设置对话框
```

---

## English

### Overview

Hides the system desktop icon layer (`SysListView32`) and re-presents desktop content in **semi-transparent, rounded panels** with soft shadows. Drag icons between panels, marquee-select, double-click to open — all state persisted in the registry.

### Features

**Panel operations**

- **Drag to re-group** — drag icons between panels; in-panel reorder
- **Marquee selection** — drag on empty area for rubber-band select
- **Ctrl/Shift multi-select** — toggle or range-select icons
- **Double-click open** — single or all selected items
- **Native context menu** — Explorer-identical IContextMenu
- **Vertical scrollbar** — auto-appears when height is insufficient
- **`+N` overflow chip** — click to expand panel

**Panel management**

- **Free position** — drag by title bar, screen-coordinate delta method
- **Free resize** — edge/corner drag with directional cursor feedback
- **Per-panel settings** — layer (low/high), title visibility, title alignment, hide/show
- **New / Rename / Delete panel**
- **Stacking preference** — click title to raise; order persists across Win+D and restarts

**System integration**

- **Win+D resistant** — event-driven + idempotent anchoring + anchor blacklist fallback chain
- **Auto-start on boot** — tray checkbox, writes HKCU Run
- **Registry persistence** — `HKCU\Software\rdesktop`, no admin required
- **Tray icon** — Settings / Refresh / Show-Hide / Auto-start / Exit & restore

**Appearance**

- DWM frosted glass, pixel-input corner radius, pixel-input column/row gaps
- Dark semi-transparent panels with soft shadows

### Quick Start

```
git clone https://github.com/YOUR_USERNAME/rdesktop.git
cd rdesktop
cargo build --release
target\release\rdesktop.exe
```

### Architecture

```
src/main.rs      Entry: DPI, single instance, hide icons, message loop
src/app.rs       State: group model, layout, registry I/O, hit testing
src/desktop.rs   Shell: enumerate, icon extraction, open, native context menu
src/render.rs    Direct2D pipeline: rounded rect, shadows, icons, text, scrollbar
src/regstore.rs  Registry persistence helpers
src/panel.rs     Window procedures: interaction, z-order, tray, scrollbar
src/settings.rs  Settings dialog
```

---

<div align="center">

**License: MIT** · Built with Rust + Win32 API + Direct2D

</div>
