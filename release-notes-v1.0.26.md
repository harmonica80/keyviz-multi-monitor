# Keyviz v1.0.26 Test

- Settings toolbar preview uses the same presentational component as the real toolbar, including theme, visible tool order, palette, size slider, undo/redo and utility footer. Preview controls are inert.
- New Circular Menu Settings: two saved quick colors, default #ef2b2d (red) and #2d37d6 (blue). Legacy settings receive these defaults. Toolbar reset restores both colors.
- Quick colors are independent of the six drawing theme palettes and the full toolbar color palette. The native swatch display and selection actions use the same configured values.
- The radial hub is blank instead of showing a cross; clicking it still dismisses the menu. Cross-mark tool, Esc and right-click behavior are unchanged.
- Black custom swatches remain opaque through the existing native alpha conversion.
- Version metadata synchronized to 1.0.26. Stable Release is unchanged.

## Verification

Run frontend build, shortcut and preference regression checks; Windows CI is needed for native compilation and renderer fixtures because local MSVC link.exe is unavailable.

Real Windows 10/11 input, mixed-DPI, recording and settings synchronization still require hardware testing.
