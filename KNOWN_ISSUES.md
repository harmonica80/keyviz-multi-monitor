# Known Issues

## 2026-08-12: PixPin pinned screenshot turns the drawing screen black

Status: fix implemented; Windows build and PixPin runtime verification pending.

### Reproduction

1. Start Keyviz and open the screen-drawing toolbar.
2. Use PixPin to capture part of the screen.
3. Pin the captured image as an always-on-top sticky image.
4. The screen outside the pinned image may turn completely black while the Keyviz drawing toolbar remains visible.
5. Press `Esc` to close the Keyviz drawing toolbar; the screen immediately returns to normal.

### Current observations

- The problem occurs while the Keyviz drawing overlay is active.
- The PixPin pinned image remains visible above the black area.
- Closing the Keyviz drawing overlay restores the desktop, so the black area is not permanently written to the screen.
- Screenshot: [2026-08-12-pixpin-black-overlay.png](docs/issues/2026-08-12-pixpin-black-overlay.png)

### Investigation priorities

- Check the interaction between Keyviz's per-monitor layered/topmost windows and PixPin's pinned topmost window.
- Verify whether the per-pixel alpha surface is being presented as opaque black after the window z-order changes.
- Compare one-monitor and multi-monitor behavior, including mixed DPI scaling.
- Trace `WM_WINDOWPOSCHANGED`, topmost reassertion, `UpdateLayeredWindow`, and overlay recreation around the moment PixPin pins the image.
- Confirm whether the issue depends on which application enters topmost mode first.

### Implemented fix

- Removed the low-alpha black input sentinel from the visible layered canvas.
- The visible per-monitor canvas now keeps unused pixels fully transparent.
- Added a separate per-monitor input window with `WS_EX_NOREDIRECTIONBITMAP`, so drawing tools can still receive mouse, wheel, keyboard, and cursor events without adding a visible DWM surface.
- The drawing display and input windows are created, raised, hidden, and destroyed together.
