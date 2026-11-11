# Keyviz v1.0.25 - Unified Drawing Themes (Test)

## Changes

- One theme selector for the drawing toolbar and native circular menu; replaces the six old menu-only styles.
- 01 Pearl (default), 02 Graphite, 03 Ocean, 04 Butter Yellow, 05 Bright Orange, 06 Sand.
- Pearl and Butter Yellow use #fcfbfc surfaces; Bright Orange uses #fce4ec light pink surfaces with orange accents.
- Shared JSON palette consumed by React and Rust; theme-aware normal, selected, disabled, clear and utility icons.
- Rounded opaque toolbar window and circular selected-tool buttons; rounded radial action buttons and existing soft radial shadow. No blur or new layered toolbar transparency.
- Old menu style IDs fall back to Pearl. Tool order, visibility, hover preferences, shortcuts and drawing colors are preserved.
- Version metadata synchronized to 1.0.25.

## Verification

- Frontend build and shortcut/theme preference regression tests run locally.
- Windows-native compilation and renderer tests require GitHub Actions because local MSVC link.exe is unavailable.
- Real-device testing is still required: theme switching while the toolbar is hidden, all utility actions, mixed-DPI monitors, recording, and Windows 10/11 window clipping/shadows.
- This is a test build only. Stable Release is not updated.
