# rdesktop — Windows Desktop Group Manager (Rust + Win32 API)

![rdesktop](icon_preview.png)

Replaces the system desktop icon layer with **semi-transparent, rounded, soft-shadow group panels**. Hides the original `SysListView32` desktop icon window and re-presents desktop content in categorized panels with drag-and-drop grouping, marquee & multi-select, per-panel layer/title/visibility control, and full registry persistence.

| Group | Content |
|---|---|
| Folders | All folders on the desktop |
| Files | Non-shortcut files (documents, images, etc.) |
| Shortcuts | `.lnk` / `.url` / `.appref-ms` |
| Quick Actions | Recycle Bin, This PC, Network, Control Panel, Settings |

## Screenshots

| Folders panel | Files panel (with scrollbar) |
|---|---|
| ![Folders panel](screenshots/panel-folders.png) | ![Files panel](screenshots/panel-files.png) |

| Shortcuts panel | Settings window |
|---|---|
| ![Shortcuts panel](screenshots/panel-shortcuts.png) | ![Settings window](screenshots/settings.png) |

> All screenshots use test data — no real files. See [screenshots/](screenshots/) for more.

## Features

### Core

- **Panel-based desktop**: hides the system `SysListView32` icon layer; renders desktop items in four semi-transparent, rounded panels with soft shadows, drawn via Direct2D + `UpdateLayeredWindow`.
- **Drag to re-group**: drag any icon from one panel to another (target panel highlights with a blue border; the ghost icon center tracks the cursor precisely). Result is persisted.
- **In-panel reordering**: drag icons within the same panel to any cell (including empty slots). Blue drop-target highlight. Manual order persists (`mo` + `oi` keys) and **overrides auto-sorting**.
- **Native context menu**: right-click any icon for the **full Explorer context menu** (`IContextMenu` — Open With, Send To, Properties, third-party extensions, etc.). Built-in items (Recycle Bin / This PC / Network / Control Panel / Settings) use the same native menu.
- **Marquee & multi-select**: drag on empty area for rubber-band selection; hold **Ctrl** to toggle individual items; hold **Shift** for range selection. Double-click opens all selected items (up to 16). Dragging a multi-selection moves all items to the target panel.

### Panel management

- **Free position**: drag by title bar — self-implemented movement (no system `HTCAPTION` modal loop), screen-coordinate delta method (no bounce/jitter). Click title to raise panel above siblings; stacking order is remembered (`zo` key, persists across Win+D and restarts).
- **Free resize**: drag any edge or corner (hot zones align with the visible rounded edge, hover shows direction-specific resize cursor). Live content re-layout during resize. Minimum size enforced.
- **Per-panel settings** (right-click menu + Settings dialog):
  - **Layer**: Low (above desktop, below apps — default) or High (topmost)
  - **Show title**: title text on/off; off = narrow drag strip (still draggable)
  - **Hide panel**: hides a single panel; restore via tray menu "Show panel: …"
  - **Delete panel**: removes user-created panels
  - **New panel…** / **Rename panel…** / **Title alignment ▸ Left/Center/Right**
  - App-level options (same as tray menu): Settings / Refresh / Show panels ✓ / Show system icons ✓ / Auto-start ✓ / Exit & restore
- **Vertical scrollbar**: when the panel height is insufficient, a thin scrollbar appears — mouse wheel, thumb drag, and track click all work.
- **`+N` overflow chip**: when the panel is too small for all items, a chip appears — click to auto-expand.

### System integration

- **Win+D resistant** (multi-layer protection): event-driven z-re-anchoring (`SetWinEventHook`), idempotent anchoring with anchor blacklist & fallback chain, 150ms × 12 correction window, `SC_MINIMIZE` interception + immediate `SW_RESTORE`, 2-second timer backstop. Steady-state = zero actions, zero log growth.
- **Tray icon**: Settings…, Refresh, Show/Hide panels, Show/Hide system icons, **Auto-start on boot** (writes `HKCU\...\Run`), Exit & restore desktop.
- **Desktop monitoring**: file/folder create/delete/rename on the desktop auto-refreshes every 2 seconds (with Auto-tidy on).
- **Registry persistence**: all state (positions, sizes, titles, layers, stacking order, settings, item assignments, manual orders) stored in `HKCU\Software\rdesktop`. Legacy `config.cfg` is auto-migrated on first run.

## Settings

| Setting | Description |
|---|---|
| **Frosted glass** | DWM blur behind panel rounded-rect region |
| **Corner radius (px)** | Direct numeric input (0–64), instant redraw |
| **Auto-tidy** | On: auto-group by type, sort by name, auto-refresh on desktop change; Off: keep manual order |
| **Show panel titles** | Batch default for all panels (per-panel toggle in right-click menu) |
| **Grid snap** | Off / 8 / 16 / 32 px; snap panel position on drag release |
| **Column gap / Row gap (px)** | Numeric input (col 0–48, row 0–40); instant re-layout (manually sized panels excluded) |
| **Layer (per panel)** | Low (default) / High (topmost), per-panel combo in settings |

## Build

```bash
cargo build --release
```

**Requirements**: Windows 10+, Rust stable. Dependencies: [`windows`](https://crates.io/crates/windows) 0.62 (Win32 bindings), [`windows-numerics`](https://crates.io/crates/windows-numerics) 0.3.

The executable is at `target\release\rdesktop.exe`.

## Architecture

```
src/main.rs     Entry: DPI awareness, single instance, hide system icons, message loop, cleanup
src/app.rs      App state: group model, layout algorithm, registry I/O, hit testing
src/desktop.rs  Shell interaction: hide/restore SysListView32, enumerate desktop,
                icon extraction (IShellItemImageFactory → WIC), open, native context menu (IContextMenu)
src/render.rs   Direct2D/DirectWrite/WIC pipeline: rounded rect, transparency, shadows,
                icons, text (trimming) → premultiplied-alpha bitmap → UpdateLayeredWindow
src/regstore.rs Registry persistence helpers (HKCU\Software\rdesktop)
src/panel.rs    Window procedures (hit/selection/drag/dblclick/context menu/tray/timers),
                z-order management (anchored above Progman/WorkerW, below normal apps),
                drag ghost window, scrollbar state
src/settings.rs Settings dialog: frosted glass, radius, auto-tidy, grid, gaps,
                per-panel layer/hide/title checkboxes
```

### Key implementation details

1. **Desktop icon replacement**: `EnumWindows` → `Progman/WorkerW → SHELLDLL_DefView → SysListView32` → `ShowWindow(SW_HIDE)`; `SW_SHOW` on exit.
2. **Transparency + rounded corners**: `WS_EX_LAYERED` + `UpdateLayeredWindow` (AC_SRC_ALPHA); every pixel painted by Direct2D into a 32-bit premultiplied-alpha DIB.
3. **Desktop layering**: panels are `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`; idempotent z-re-anchoring places them between the shell (Progman/WorkerW) and the lowest visible app window.
4. **Native context menu**: `SHBindToParent` for canonical parent+child PIDL binding → `GetUIObjectOf(IID_IContextMenu)` → `QueryContextMenu` → `TrackPopupMenu(TPM_RETURNCMD)` → `InvokeCommand`.
5. **Drag & drop**: `SetCapture` + threshold; ghost window (`WS_EX_TOPMOST | WS_EX_TRANSPARENT`) tracks cursor; drop target via `PtInRect`.

## License

MIT
