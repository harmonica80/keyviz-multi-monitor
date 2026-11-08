// Include in native_drawing::platform alongside drawing_radial.rs.
// Panel order is red, black, undo, redo, toolbar (hover slots 13..=17).
unsafe fn draw_radial_styled(
    dc: HDC,
    menu: &RadialMenu,
    tools: &[NativeTool],
    style: &str,
    panel: &[RECT; 5],
    can_undo: bool,
    can_redo: bool,
    toolbar_hidden: bool,
) {
    radial_styled_renderer::draw(
        dc,
        menu,
        tools,
        style,
        panel,
        can_undo,
        can_redo,
        toolbar_hidden,
    );
}

mod radial_styled_renderer {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Palette {
        sectors: [u32; 6],
        hub: u32,
        divider: u32,
        panel: u32,
    }

    fn palette(style: &str) -> Palette {
        match style {
            "gray" => Palette {
                sectors: [0xffe1e4e8; 6],
                hub: 0xffc9ced5,
                divider: 0xff9ba3ae,
                panel: 0xffe1e4e8,
            },
            "colorful" => Palette {
                sectors: [
                    0xffffe0df, 0xffffedcb, 0xffe8f2cf, 0xffd9f1ed, 0xffdce8ff, 0xffecdfff,
                ],
                hub: 0xffffffff,
                divider: 0xffb5bbc7,
                panel: 0xfffffaf5,
            },
            "blue" => Palette {
                sectors: [0xffe2edfb; 6],
                hub: 0xffbed4f2,
                divider: 0xff7e9fc8,
                panel: 0xffe2edfb,
            },
            "classic" => Palette {
                sectors: [0xffe4e8ed; 6],
                hub: 0xffc9cfd6,
                divider: 0xff7f8995,
                panel: 0xffe4e8ed,
            },
            "minimal" => Palette {
                sectors: [0xffffffff; 6],
                hub: 0xffffffff,
                divider: 0xff202020,
                panel: 0xffffffff,
            },
            // Unknown persisted values deliberately use the new white default.
            _ => Palette {
                sectors: [0xffffffff; 6],
                hub: 0xffffffff,
                divider: 0xffc7cbd1,
                panel: 0xffffffff,
            },
        }
    }

    const HOVER: u32 = 0xffcde5ff;
    // Raised silver bevel: inset shadow, broad highlight, then a darker outer lip.
    const SILVER_BANDS: [u32; 8] = [
        0xffcbd1d7, 0xffeef1f4, 0xffffffff, 0xfff5f7f9, 0xffe4e8ed, 0xffd6dce3, 0xffc1c9d2,
        0xffaeb9c5,
    ];

    fn sector_fill(colors: Palette, index: usize, hovered: bool) -> u32 {
        if hovered {
            HOVER
        } else {
            colors.sectors[index % 6]
        }
    }

    fn panel_enabled(index: usize, can_undo: bool, can_redo: bool) -> bool {
        match index {
            2 => can_undo,
            3 => can_redo,
            _ => true,
        }
    }

    fn point(menu: &RadialMenu, angle: f64, radius: f64) -> WinPoint {
        WinPoint {
            x: menu.center.x + (angle.cos() * radius).round() as i32,
            y: menu.center.y + (angle.sin() * radius).round() as i32,
        }
    }

    fn sector_points(menu: &RadialMenu, index: usize) -> Vec<WinPoint> {
        let inner = menu.radius * if index < 6 { 0.22 } else { 0.61 };
        let outer = menu.radius * if index < 6 { 0.61 } else { 1.0 };
        sector_band(menu, index, inner, outer)
    }

    fn sector_band(menu: &RadialMenu, index: usize, inner: f64, outer: f64) -> Vec<WinPoint> {
        let (start, sweep) = radial_sector(index);
        (0..=32)
            .map(|step| point(menu, start + sweep * step as f64 / 32.0, outer))
            .chain(
                (0..=32)
                    .rev()
                    .map(|step| point(menu, start + sweep * step as f64 / 32.0, inner)),
            )
            .collect()
    }

    unsafe fn fill(graphics: *mut c_void, points: &[WinPoint], color: u32) {
        let mut brush = std::ptr::null_mut();
        if GdipCreateSolidFill(color, &mut brush) == 0 {
            GdipFillPolygonI(graphics, brush, points.as_ptr(), points.len() as i32, 0);
            GdipDeleteBrush(brush);
        }
    }

    pub(super) unsafe fn draw(
        dc: HDC,
        menu: &RadialMenu,
        tools: &[NativeTool],
        style: &str,
        panel: &[RECT; 5],
        can_undo: bool,
        can_redo: bool,
        toolbar_hidden: bool,
    ) {
        if !menu.radius.is_finite() || menu.radius <= 0.0 {
            return;
        }
        let colors = palette(style);
        // These are geometry supplied by input handling, not independently laid out buttons.
        with_gdiplus_graphics(dc, false, |graphics| {
            let pen = create_gdiplus_pen(colors.divider, 1);
            for index in 0..=RADIAL_CLEAR_SLOT {
                let points = sector_points(menu, index);
                let hovered = menu.hovered == Some(index) && radial_action(index, tools).is_some();
                fill(graphics, &points, sector_fill(colors, index, hovered));
                if style == "classic" && !hovered {
                    let inner = menu.radius * if index < 6 { 0.22 } else { 0.61 };
                    let thickness = menu.radius * 0.39;
                    for (band, color) in SILVER_BANDS.iter().enumerate() {
                        let low = inner + thickness * band as f64 / 8.0;
                        let high = inner + thickness * (band + 1) as f64 / 8.0;
                        fill(graphics, &sector_band(menu, index, low, high), *color);
                    }
                }
                if let Some(pen) = pen {
                    GdipDrawPolygonI(graphics, pen, points.as_ptr(), points.len() as i32);
                }
            }
            let hub: Vec<_> = (0..96)
                .map(|step| {
                    point(
                        menu,
                        step as f64 * std::f64::consts::TAU / 96.0,
                        menu.radius * 0.22,
                    )
                })
                .collect();
            fill(graphics, &hub, colors.hub);
            if let Some(pen) = pen {
                GdipDrawPolygonI(graphics, pen, hub.as_ptr(), hub.len() as i32);
            }
            for (index, rect) in panel.iter().enumerate() {
                if rect.right <= rect.left || rect.bottom <= rect.top {
                    continue;
                }
                let points = [
                    WinPoint {
                        x: rect.left,
                        y: rect.top,
                    },
                    WinPoint {
                        x: rect.right - 1,
                        y: rect.top,
                    },
                    WinPoint {
                        x: rect.right - 1,
                        y: rect.bottom - 1,
                    },
                    WinPoint {
                        x: rect.left,
                        y: rect.bottom - 1,
                    },
                ];
                let hovered =
                    menu.hovered == Some(13 + index) && panel_enabled(index, can_undo, can_redo);
                fill(
                    graphics,
                    &points,
                    if hovered { HOVER } else { colors.panel },
                );
                if style == "classic" && !hovered {
                    let height = rect.bottom - rect.top - 1;
                    for (band, color) in SILVER_BANDS.iter().enumerate() {
                        let top = rect.top + height * band as i32 / 8;
                        let bottom = rect.top + height * (band + 1) as i32 / 8;
                        let stripe = [
                            WinPoint {
                                x: rect.left,
                                y: top,
                            },
                            WinPoint {
                                x: rect.right - 1,
                                y: top,
                            },
                            WinPoint {
                                x: rect.right - 1,
                                y: bottom,
                            },
                            WinPoint {
                                x: rect.left,
                                y: bottom,
                            },
                        ];
                        fill(graphics, &stripe, *color);
                    }
                }
                if let Some(pen) = pen {
                    GdipDrawPolygonI(graphics, pen, points.as_ptr(), points.len() as i32);
                }
            }
            if let Some(pen) = pen {
                GdipDeletePen(pen);
            }
        });
        for index in 0..=RADIAL_CLEAR_SLOT {
            let Some(action) = radial_action(index, tools) else {
                continue;
            };
            let (start, sweep) = radial_sector(index);
            let position = point(
                menu,
                start + sweep / 2.0,
                menu.radius * if index < 6 { 0.415 } else { 0.805 },
            );
            let center = Point {
                x: position.x,
                y: position.y,
            };
            match action {
                RadialAction::Tool(tool) => draw_radial_icon(
                    dc,
                    tool,
                    center,
                    menu.radius / 156.0,
                    menu.hovered == Some(index),
                ),
                // The reused clear helper has a red hover; keep its neutral icon on blue fill.
                RadialAction::Clear => {
                    draw_radial_clear_icon(dc, center, menu.radius / 156.0, false)
                }
            }
        }
        draw_radial_icon(
            dc,
            NativeTool::CrossMark,
            menu.center,
            menu.radius / 240.0,
            false,
        );
        for (index, rect) in panel.iter().enumerate() {
            if rect.right <= rect.left || rect.bottom <= rect.top {
                continue;
            }
            draw_panel_icon(
                dc,
                menu.radius,
                rect,
                index,
                panel_enabled(index, can_undo, can_redo),
                toolbar_hidden,
            );
        }
    }

    unsafe fn draw_panel_icon(
        dc: HDC,
        radius: f64,
        rect: &RECT,
        index: usize,
        enabled: bool,
        toolbar_hidden: bool,
    ) {
        let center = Point {
            x: rect.left + (rect.right - rect.left) / 2,
            y: rect.top + (rect.bottom - rect.top) / 2,
        };
        let scale = (radius / 78.0)
            .min((rect.right - rect.left) as f64 / 28.0)
            .min((rect.bottom - rect.top) as f64 / 28.0);
        let p = |x: f64, y: f64| Point {
            x: center.x + (x * scale).round() as i32,
            y: center.y + (y * scale).round() as i32,
        };
        let color = if enabled {
            COLORREF(0x181818)
        } else {
            COLORREF(0xa5a5a5)
        };
        let line = |a: (f64, f64), b: (f64, f64)| {
            draw_antialiased_shape(
                dc,
                &NativeTool::Line,
                p(a.0, a.1),
                p(b.0, b.1),
                color,
                scale.round().max(1.0) as i32,
                0.0,
            );
        };
        match index {
            0 | 1 => {
                with_gdiplus_graphics(dc, false, |graphics| {
                    let mut brush = std::ptr::null_mut();
                    let color = if index == 0 { 0xffe53935 } else { 0xff181818 };
                    if GdipCreateSolidFill(color, &mut brush) == 0 {
                        let size = (12.0 * scale).round().max(1.0) as i32;
                        GdipFillEllipseI(
                            graphics,
                            brush,
                            center.x - size / 2,
                            center.y - size / 2,
                            size,
                            size,
                        );
                        GdipDeleteBrush(brush);
                    }
                });
            }
            2 | 3 => {
                let mirror = if index == 2 { 1.0 } else { -1.0 };
                let path = [
                    (-7.0, -2.0),
                    (-2.0, -5.0),
                    (4.0, -5.0),
                    (7.0, -2.0),
                    (7.0, 3.0),
                    (4.0, 6.0),
                    (0.0, 6.0),
                ];
                for pair in path.windows(2) {
                    line(
                        (pair[0].0 * mirror, pair[0].1),
                        (pair[1].0 * mirror, pair[1].1),
                    );
                }
                line((-7.0 * mirror, -2.0), (-7.0 * mirror, -7.0));
                line((-7.0 * mirror, -2.0), (-2.0 * mirror, -1.0));
            }
            4 => {
                for (a, b) in [
                    ((-9.0, -6.0), (9.0, -6.0)),
                    ((9.0, -6.0), (9.0, 6.0)),
                    ((9.0, 6.0), (-9.0, 6.0)),
                    ((-9.0, 6.0), (-9.0, -6.0)),
                    ((-5.0, -2.0), (-5.0, 2.0)),
                    ((-1.0, -2.0), (-1.0, 2.0)),
                    ((3.0, 0.0), (7.0, 0.0)),
                ] {
                    line(a, b);
                }
                if toolbar_hidden {
                    line((5.0, -2.0), (5.0, 2.0));
                }
            }
            _ => {}
        }
    }

    #[cfg(test)]
    mod hover_tests {
        use super::*;

        #[test]
        fn original_is_white_with_fine_gray_not_minimal_black() {
            let original = palette("original");
            assert_eq!(original.sectors, [0xffffffff; 6]);
            assert_eq!(original.hub, 0xffffffff);
            assert_eq!(original.divider, 0xffc7cbd1);
            assert_eq!(palette("minimal").divider, 0xff202020);
            assert_eq!(palette(""), original);
            assert_eq!(palette("unknown"), original);
            assert_eq!(palette("legacy"), original);
        }

        #[test]
        fn classic_silver_has_highlight_and_shadow_depth() {
            assert_eq!(palette("classic").sectors, [0xffe4e8ed; 6]);
            assert_eq!(SILVER_BANDS[2], 0xffffffff);
            assert_ne!(SILVER_BANDS[0], SILVER_BANDS[2]);
            assert_ne!(SILVER_BANDS[2], SILVER_BANDS[7]);
            for color in SILVER_BANDS {
                let red = (color >> 16) & 255;
                let blue = color & 255;
                assert!(blue >= red && blue - red <= 24);
            }
        }

        #[test]
        fn six_themes_are_distinct_and_hover_is_always_blue() {
            let styles = ["original", "gray", "colorful", "blue", "classic", "minimal"];
            for (index, style) in styles.iter().enumerate() {
                let colors = palette(style);
                for other in &styles[index + 1..] {
                    assert_ne!(colors, palette(other));
                }
                for slot in 0..=RADIAL_CLEAR_SLOT {
                    assert_eq!(sector_fill(colors, slot, true), HOVER);
                    assert_eq!(sector_fill(colors, slot, false), colors.sectors[slot % 6]);
                }
            }
        }

        #[test]
        fn geometry_stays_aligned_to_existing_two_ring_hit_regions() {
            for radius in [78.0, 117.0, 156.0] {
                let menu = RadialMenu {
                    center: Point { x: -400, y: 250 },
                    radius,
                    hovered: None,
                    panel: [RECT::default(); 5],
                };
                for index in 0..=RADIAL_CLEAR_SLOT {
                    let (start, sweep) = radial_sector(index);
                    let position = point(
                        &menu,
                        start + sweep / 2.0,
                        radius * if index < 6 { 0.415 } else { 0.805 },
                    );
                    assert_eq!(
                        menu.hit(Point {
                            x: position.x,
                            y: position.y
                        }),
                        Some(index)
                    );
                    let vertices = sector_points(&menu, index);
                    assert_eq!(vertices.len(), 66);
                    for vertex in vertices {
                        let distance = ((vertex.x - menu.center.x) as f64)
                            .hypot((vertex.y - menu.center.y) as f64);
                        let (inner, outer) = if index < 6 { (0.22, 0.61) } else { (0.61, 1.0) };
                        assert!(distance >= radius * inner - 1.0);
                        assert!(distance <= radius * outer + 1.0);
                    }
                }
            }
        }

        #[test]
        fn undo_redo_disabled_independently_without_disabling_other_buttons() {
            for (undo, redo) in [(false, false), (false, true), (true, false), (true, true)] {
                assert_eq!(panel_enabled(2, undo, redo), undo);
                assert_eq!(panel_enabled(3, undo, redo), redo);
                for index in [0, 1, 4] {
                    assert!(panel_enabled(index, undo, redo));
                }
            }
        }

        #[test]
        fn native_six_theme_previews_keep_transparency_and_action_states() {
            unsafe {
                let mut surface = OverlaySurface {
                    display_hwnd: HWND(0),
                    input_hwnd: HWND(0),
                    bounds: RECT {
                        left: 0,
                        top: 0,
                        right: 360,
                        bottom: 360,
                    },
                    canvas: None,
                    display_ready: false,
                    last_update_error: None,
                    frame_stats: None,
                };
                assert!(ensure_overlay_canvas(&mut surface));
                let canvas = surface.canvas.as_ref().unwrap();
                let panel = [
                    RECT {
                        left: 270,
                        top: 140,
                        right: 300,
                        bottom: 170,
                    },
                    RECT {
                        left: 304,
                        top: 140,
                        right: 334,
                        bottom: 170,
                    },
                    RECT {
                        left: 270,
                        top: 174,
                        right: 300,
                        bottom: 204,
                    },
                    RECT {
                        left: 304,
                        top: 174,
                        right: 334,
                        bottom: 204,
                    },
                    RECT {
                        left: 270,
                        top: 208,
                        right: 334,
                        bottom: 238,
                    },
                ];
                let mut menu = RadialMenu {
                    center: Point { x: 180, y: 180 },
                    radius: 78.0,
                    hovered: None,
                    panel,
                };
                let pixels = std::slice::from_raw_parts_mut(canvas.bits as *mut u32, 360 * 360);
                let mut snapshots = Vec::new();
                for style in ["original", "gray", "colorful", "blue", "classic", "minimal"] {
                    // Both images show blue hover, disabled redo, and the toolbar toggle state.
                    for (variant, hovered, toolbar_hidden) in [
                        ("ring", 1, true),
                        ("panel", 15, false),
                        ("disabled", 16, true),
                    ] {
                        pixels.fill(0);
                        menu.hovered = Some(hovered);
                        draw_radial_styled(
                            HDC(canvas.memory_dc.0),
                            &menu,
                            &RADIAL_TOOLS,
                            style,
                            &panel,
                            true,
                            false,
                            toolbar_hidden,
                        );
                        assert!(GdiFlush().as_bool());
                        assert_eq!(pixels[0], 0);
                        assert_eq!(pixels[180 * 360 + 260], 0);
                        assert_eq!(pixels[359 * 360 + 359], 0);
                        let painted = pixels.iter().filter(|pixel| **pixel != 0).count();
                        assert!((22_000..28_000).contains(&painted), "{style}: {painted}");
                        let undo_bg = pixels[177 * 360 + 273] & 0xffffff;
                        let redo_bg = pixels[177 * 360 + 307] & 0xffffff;
                        if hovered == 15 {
                            assert_eq!(undo_bg, HOVER & 0xffffff);
                        } else if style != "classic" {
                            assert_eq!(undo_bg, palette(style).panel & 0xffffff);
                        }
                        if style == "classic" {
                            assert_ne!(redo_bg, HOVER & 0xffffff);
                            assert_ne!(redo_bg, pixels[200 * 360 + 307] & 0xffffff);
                        } else {
                            assert_eq!(redo_bg, palette(style).panel & 0xffffff);
                        }
                        if hovered == 1 {
                            let (start, sweep) = radial_sector(1);
                            let sample = point(&menu, start + sweep / 2.0, menu.radius * 0.54);
                            assert_eq!(
                                pixels[(sample.y * 360 + sample.x) as usize] & 0xffffff,
                                HOVER & 0xffffff
                            );
                        }
                        if variant == "ring" {
                            snapshots.push(pixels.to_vec());
                        }
                        if let Some(directory) =
                            std::env::var_os("KEYVIZ_RADIAL_STYLES_PREVIEW_DIR")
                        {
                            let directory = std::path::PathBuf::from(directory);
                            std::fs::create_dir_all(&directory).unwrap();
                            let bytes = std::slice::from_raw_parts(
                                canvas.bits as *const u8,
                                pixels.len() * 4,
                            );
                            let mut bmp = Vec::with_capacity(54 + bytes.len());
                            bmp.extend_from_slice(b"BM");
                            bmp.extend_from_slice(&(54u32 + bytes.len() as u32).to_le_bytes());
                            bmp.extend_from_slice(&[0; 4]);
                            bmp.extend_from_slice(&54u32.to_le_bytes());
                            bmp.extend_from_slice(&40u32.to_le_bytes());
                            bmp.extend_from_slice(&360i32.to_le_bytes());
                            bmp.extend_from_slice(&(-360i32).to_le_bytes());
                            bmp.extend_from_slice(&1u16.to_le_bytes());
                            bmp.extend_from_slice(&32u16.to_le_bytes());
                            bmp.extend_from_slice(&[0; 24]);
                            bmp.extend_from_slice(bytes);
                            std::fs::write(
                                directory.join(format!("radial-{style}-{variant}.bmp")),
                                bmp,
                            )
                            .unwrap();
                        }
                    }
                }
                for (index, snapshot) in snapshots.iter().enumerate() {
                    for other in &snapshots[index + 1..] {
                        assert_ne!(snapshot, other);
                    }
                }
                release_overlay_canvas(&mut surface.canvas);
            }
        }
    }
}
