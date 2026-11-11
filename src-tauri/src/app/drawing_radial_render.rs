// Include in native_drawing::platform alongside drawing_radial.rs.
// Panel order is quick colors 1/2, undo, redo, toolbar, keyboard, settings (13..=19).
// Keep shadows out of the DC: the overlay converts every nonzero RGB to opaque.
pub(super) fn radial_shadow_alpha(menu: &RadialMenu, panel: &[RECT; 7], point: Point) -> u8 {
    if !menu.radius.is_finite() || menu.radius <= 0.0 {
        return 0;
    }
    let distance = |offset_y: f64| {
        let x = f64::from(point.x);
        let y = f64::from(point.y) - offset_y;
        let mut nearest =
            (x - f64::from(menu.center.x)).hypot(y - f64::from(menu.center.y)) - menu.radius;
        for rect in panel {
            if rect.right <= rect.left || rect.bottom <= rect.top {
                continue;
            }
            let left = f64::from(rect.left);
            let right = f64::from(rect.right - 1);
            let top = f64::from(rect.top);
            let bottom = f64::from(rect.bottom - 1);
            let dx = (left - x).max(x - right);
            let dy = (top - y).max(y - bottom);
            let signed = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0);
            nearest = nearest.min(signed);
        }
        nearest
    };
    if distance(0.0) <= 0.0 {
        return 0;
    }
    let feather = ((distance(3.0) + 6.0) / 12.0).clamp(0.0, 1.0);
    (60.0 * (1.0 - feather * feather * (3.0 - 2.0 * feather))).round() as u8
}

unsafe fn draw_radial_styled(
    dc: HDC,
    menu: &RadialMenu,
    tools: &[NativeTool],
    style: &str,
    panel: &[RECT; 7],
    can_undo: bool,
    can_redo: bool,
    toolbar_hidden: bool,
    keyboard_visible: bool,
    quick_colors: &[String; 2],
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
        keyboard_visible,
        quick_colors,
    );
}

mod radial_styled_renderer {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Palette {
        fill: u32,
        ink: u32,
        accent: u32,
        selected_ink: u32,
        divider: u32,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Theme {
        id: String,
        fill: String,
        ink: String,
        accent: String,
        selected_ink: String,
        divider: String,
    }

    fn themes() -> &'static [(String, Palette)] {
        static THEMES: OnceLock<Vec<(String, Palette)>> = OnceLock::new();
        THEMES.get_or_init(|| {
            let source: Vec<Theme> =
                serde_json::from_str(include_str!("../../../src/lib/drawing-themes.json"))
                    .expect("Shared drawing themes must be valid JSON");
            let argb = |hex: &str| {
                let digits = hex
                    .strip_prefix('#')
                    .filter(|value| value.len() == 6)
                    .expect("Theme colors must be #RRGGBB");
                0xff000000 | u32::from_str_radix(digits, 16).expect("Invalid theme color")
            };
            let themes: Vec<_> = source
                .into_iter()
                .map(|theme| {
                    (
                        theme.id,
                        Palette {
                            fill: argb(&theme.fill),
                            ink: argb(&theme.ink),
                            accent: argb(&theme.accent),
                            selected_ink: argb(&theme.selected_ink),
                            divider: argb(&theme.divider),
                        },
                    )
                })
                .collect();
            assert!(
                themes.iter().any(|(id, _)| id == "pearl"),
                "Missing default pearl theme"
            );
            themes
        })
    }

    pub(super) fn has_theme(style: &str) -> bool {
        themes().iter().any(|(id, _)| id == style)
    }

    fn palette(style: &str) -> Palette {
        let id = normalize_radial_style(style);
        themes().iter().find(|(theme, _)| theme == id).unwrap().1
    }

    fn colorref(argb: u32) -> COLORREF {
        COLORREF(((argb >> 16) & 255) | (argb & 0x00ff00) | ((argb & 255) << 16))
    }

    fn icon_ink(colors: Palette, hovered: bool, enabled: bool) -> u32 {
        if hovered && enabled {
            colors.selected_ink
        } else if enabled {
            colors.ink
        } else {
            // Blend opaque ink with its fill: alpha is flattened by the native overlay.
            let channel = |shift: u32| -> u32 {
                (((colors.ink >> shift) & 255u32) + ((colors.fill >> shift) & 255u32)) / 2u32
            };
            0xff000000 | (channel(16) << 16) | (channel(8) << 8) | channel(0)
        }
    }

    fn sector_fill(colors: Palette, _index: usize, hovered: bool) -> u32 {
        if hovered {
            colors.accent
        } else {
            colors.fill
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

    #[allow(dead_code)]
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

    unsafe fn fill(graphics: *mut c_void, points: &[PointF], color: u32) {
        let mut brush = std::ptr::null_mut();
        if GdipCreateSolidFill(color, &mut brush) == 0 {
            GdipFillPolygon(graphics, brush, points.as_ptr(), points.len() as i32, 0);
            GdipDeleteBrush(brush);
        }
    }

    fn point_f(menu: &RadialMenu, angle: f64, radius: f64) -> PointF {
        PointF {
            x: (menu.center.x as f64 + angle.cos() * radius) as f32,
            y: (menu.center.y as f64 + angle.sin() * radius) as f32,
        }
    }

    fn sector_points_f(menu: &RadialMenu, index: usize) -> Vec<PointF> {
        let inner = menu.radius * if index < 6 { 0.22 } else { 0.61 };
        let outer = menu.radius * if index < 6 { 0.61 } else { 1.0 };
        let (start, sweep) = radial_sector(index);
        (0..=64)
            .map(|step| point_f(menu, start + sweep * step as f64 / 64.0, outer))
            .chain(
                (0..=64)
                    .rev()
                    .map(|step| point_f(menu, start + sweep * step as f64 / 64.0, inner)),
            )
            .collect()
    }

    fn panel_points_f(rect: &RECT, radius: f64) -> Vec<PointF> {
        let left = f64::from(rect.left);
        let top = f64::from(rect.top);
        let right = f64::from(rect.right - 1);
        let bottom = f64::from(rect.bottom - 1);
        let corner = (6.0 * radius / 78.0)
            .min((right - left) / 2.0)
            .min((bottom - top) / 2.0);
        [
            (right - corner, top + corner),
            (right - corner, bottom - corner),
            (left + corner, bottom - corner),
            (left + corner, top + corner),
        ]
        .iter()
        .enumerate()
        .flat_map(|(index, &(x, y))| {
            (0..=16).map(move |step| {
                let angle = (index as f64 - 1.0 + step as f64 / 16.0) * std::f64::consts::FRAC_PI_2;
                PointF {
                    x: (x + corner * angle.cos()) as f32,
                    y: (y + corner * angle.sin()) as f32,
                }
            })
        })
        .collect()
    }

    #[allow(dead_code)]
    fn panel_points(rect: &RECT, radius: f64) -> Vec<WinPoint> {
        let left = f64::from(rect.left);
        let top = f64::from(rect.top);
        let right = f64::from(rect.right - 1);
        let bottom = f64::from(rect.bottom - 1);
        let corner = (6.0 * radius / 78.0)
            .min((right - left) / 2.0)
            .min((bottom - top) / 2.0);
        [
            (right - corner, top + corner),
            (right - corner, bottom - corner),
            (left + corner, bottom - corner),
            (left + corner, top + corner),
        ]
        .iter()
        .enumerate()
        .flat_map(|(index, &(x, y))| {
            (0..=8).map(move |step| {
                let angle = (index as f64 - 1.0 + step as f64 / 8.0) * std::f64::consts::FRAC_PI_2;
                WinPoint {
                    x: (x + corner * angle.cos()).round() as i32,
                    y: (y + corner * angle.sin()).round() as i32,
                }
            })
        })
        .collect()
    }

    pub(super) unsafe fn draw(
        dc: HDC,
        menu: &RadialMenu,
        tools: &[NativeTool],
        style: &str,
        panel: &[RECT; 7],
        can_undo: bool,
        can_redo: bool,
        toolbar_hidden: bool,
        keyboard_visible: bool,
        quick_colors: &[String; 2],
    ) {
        if !menu.radius.is_finite() || menu.radius <= 0.0 {
            return;
        }
        let colors = palette(style);
        // These are geometry supplied by input handling, not independently laid out buttons.
        with_gdiplus_graphics(dc, false, |graphics| {
            let mut pen = std::ptr::null_mut();
            let pen = if GdipCreatePen1(
                colors.divider,
                (menu.radius / 78.0).max(1.0) as f32,
                2,
                &mut pen,
            ) == 0
                && !pen.is_null()
            {
                Some(pen)
            } else {
                None
            };
            for index in 0..=RADIAL_CLEAR_SLOT {
                let points = sector_points_f(menu, index);
                let hovered = menu.hovered == Some(index) && radial_action(index, tools).is_some();
                fill(graphics, &points, sector_fill(colors, index, hovered));
                if let Some(pen) = pen {
                    GdipDrawPolygon(graphics, pen, points.as_ptr(), points.len() as i32);
                }
            }
            let hub: Vec<_> = (0..128)
                .map(|step| {
                    point_f(
                        menu,
                        step as f64 * std::f64::consts::TAU / 128.0,
                        menu.radius * 0.22,
                    )
                })
                .collect();
            fill(graphics, &hub, colors.fill);
            if let Some(pen) = pen {
                GdipDrawPolygon(graphics, pen, hub.as_ptr(), hub.len() as i32);
            }
            for (index, rect) in panel.iter().enumerate() {
                if rect.right <= rect.left || rect.bottom <= rect.top {
                    continue;
                }
                let points = panel_points_f(rect, menu.radius);
                let hovered =
                    menu.hovered == Some(13 + index) && panel_enabled(index, can_undo, can_redo);
                fill(
                    graphics,
                    &points,
                    if hovered { colors.accent } else { colors.fill },
                );
                if let Some(pen) = pen {
                    GdipDrawPolygon(graphics, pen, points.as_ptr(), points.len() as i32);
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
                    colorref(icon_ink(colors, menu.hovered == Some(index), true)),
                ),
                RadialAction::Clear => draw_radial_clear_icon(
                    dc,
                    center,
                    menu.radius / 156.0,
                    colorref(icon_ink(colors, menu.hovered == Some(index), true)),
                ),
            }
        }
        // The empty hub still dismisses the menu through the unchanged hit regions.
        for (index, rect) in panel.iter().enumerate() {
            if rect.right <= rect.left || rect.bottom <= rect.top {
                continue;
            }
            draw_panel_icon(
                dc,
                menu.radius,
                rect,
                index,
                icon_ink(
                    colors,
                    menu.hovered == Some(13 + index),
                    panel_enabled(index, can_undo, can_redo),
                ),
                toolbar_hidden,
                keyboard_visible,
                quick_colors,
            );
        }
    }

    fn swatch_argb(color: &str) -> u32 {
        let rgb = u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0xef2b2d);
        // Pure RGB zero is the transparent key in the overlay's alpha conversion.
        0xff000000 | if rgb == 0 { 0x010101 } else { rgb }
    }

    unsafe fn draw_panel_icon(
        dc: HDC,
        radius: f64,
        rect: &RECT,
        index: usize,
        ink: u32,
        toolbar_hidden: bool,
        keyboard_visible: bool,
        quick_colors: &[String; 2],
    ) {
        let center = Point {
            x: rect.left + (rect.right - rect.left) / 2,
            y: rect.top + (rect.bottom - rect.top) / 2,
        };
        let scale = (radius / 78.0)
            .min((rect.right - rect.left) as f64 / 28.0)
            .min((rect.bottom - rect.top) as f64 / 28.0);
        // Local units are 0.1px; `with_icon_transform` scales and centers them.
        let p = |x: f64, y: f64| Point {
            x: (x * 10.0).round() as i32,
            y: (y * 10.0).round() as i32,
        };
        let stroke = 15;
        let color = colorref(ink);
        let line = |a: (f64, f64), b: (f64, f64)| {
            draw_antialiased_shape(
                dc,
                &NativeTool::Line,
                p(a.0, a.1),
                p(b.0, b.1),
                color,
                stroke,
                0.0,
            );
        };
        with_icon_transform(center, scale, || match index {
            0 | 1 => {
                with_gdiplus_graphics(dc, false, |graphics| {
                    let mut brush = std::ptr::null_mut();
                    let color = swatch_argb(&quick_colors[index]);
                    if GdipCreateSolidFill(color, &mut brush) == 0 {
                        let size = 120;
                        GdipFillEllipseI(
                            graphics,
                            brush,
                            -size / 2,
                            -size / 2,
                            size,
                            size,
                        );
                        GdipDeleteBrush(brush);
                    }
                    // Keep custom swatches legible against both dark and light themes.
                    if let Some(pen) = create_gdiplus_pen(ink, stroke) {
                        let points: Vec<_> = (0..48)
                            .map(|step| {
                                let angle = step as f64 * std::f64::consts::TAU / 48.0;
                                let point = p(angle.cos() * 7.0, angle.sin() * 7.0);
                                WinPoint {
                                    x: point.x,
                                    y: point.y,
                                }
                            })
                            .collect();
                        GdipDrawPolygonI(graphics, pen, points.as_ptr(), points.len() as i32);
                        GdipDeletePen(pen);
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
                let outline = [
                    (-9.0, 0.0),
                    (-5.0, -4.0),
                    (0.0, -6.0),
                    (5.0, -4.0),
                    (9.0, 0.0),
                    (5.0, 4.0),
                    (0.0, 6.0),
                    (-5.0, 4.0),
                    (-9.0, 0.0),
                ];
                for pair in outline.windows(2) {
                    line(pair[0], pair[1]);
                }
                draw_antialiased_shape(
                    dc,
                    &NativeTool::Ellipse,
                    p(-2.5, -2.5),
                    p(2.5, 2.5),
                    color,
                    stroke,
                    0.0,
                );
                if toolbar_hidden {
                    line((-9.0, -8.0), (9.0, 8.0));
                }
            }
            5 => {
                for (a, b) in [
                    ((-9.0, -6.0), (9.0, -6.0)),
                    ((9.0, -6.0), (9.0, 6.0)),
                    ((9.0, 6.0), (-9.0, 6.0)),
                    ((-9.0, 6.0), (-9.0, -6.0)),
                    ((-6.0, -2.0), (-4.0, -2.0)),
                    ((-1.0, -2.0), (1.0, -2.0)),
                    ((4.0, -2.0), (6.0, -2.0)),
                    ((-5.0, 2.0), (5.0, 2.0)),
                ] {
                    line(a, b);
                }
                if !keyboard_visible {
                    line((-9.0, -8.0), (9.0, 8.0));
                }
            }
            6 => {
                // Eight teeth and a circular bore, without text or font dependencies.
                for step in 0..32 {
                    let vertex = |i: usize| {
                        let angle = i as f64 * std::f64::consts::TAU / 32.0;
                        let radius = if i % 4 < 2 { 9.0 } else { 6.5 };
                        (angle.cos() * radius, angle.sin() * radius)
                    };
                    line(vertex(step), vertex((step + 1) % 32));
                }
                draw_antialiased_shape(
                    dc,
                    &NativeTool::Ellipse,
                    p(-3.0, -3.0),
                    p(3.0, 3.0),
                    color,
                    stroke,
                    0.0,
                );
            }
            _ => {}
        });
    }

    #[cfg(test)]
    mod hover_tests {
        use super::*;

        #[test]
        fn quick_colors_render_custom_values_and_keep_black_opaque() {
            assert_eq!(swatch_argb("#ef2b2d"), 0xffef2b2d);
            assert_eq!(swatch_argb("#2d37d6"), 0xff2d37d6);
            assert_eq!(swatch_argb("#12abcd"), 0xff12abcd);
            assert_eq!(swatch_argb("#000000"), 0xff010101);
        }

        #[test]
        fn shadow_is_bounded_feathered_and_only_outside_silhouettes() {
            let mut panel = [RECT::default(); 7];
            panel[6] = RECT {
                left: 130,
                top: 90,
                right: 150,
                bottom: 110,
            };
            let menu = RadialMenu {
                center: Point { x: 100, y: 100 },
                radius: 20.0,
                hovered: None,
                panel,
            };
            for point in [
                menu.center,
                Point { x: 120, y: 100 },
                Point { x: 130, y: 90 },
                Point { x: 149, y: 109 },
                Point { x: 0, y: 0 },
                Point { x: 300, y: 300 },
                Point {
                    x: i32::MIN,
                    y: i32::MAX,
                },
            ] {
                assert_eq!(radial_shadow_alpha(&menu, &panel, point), 0);
            }
            let alpha = |x, y| radial_shadow_alpha(&menu, &panel, Point { x, y });
            assert!(alpha(100, 121) > alpha(100, 79));
            assert!(alpha(100, 121) > alpha(100, 124));
            assert!(alpha(100, 124) > alpha(100, 128));
            assert_eq!(alpha(100, 129), 0);
            assert!(alpha(140, 110) > alpha(140, 89));
            assert!(alpha(140, 110) > alpha(140, 114));
            assert_eq!(alpha(140, 118), 0);
            for y in 65..125 {
                for x in 65..160 {
                    assert!(alpha(x, y) <= 60);
                }
            }
        }

        #[test]
        fn shadow_rejects_invalid_radii_and_ignores_empty_panel_rectangles() {
            let panel = [RECT::default(); 7];
            for radius in [0.0, -1.0, f64::NAN, f64::INFINITY] {
                let menu = RadialMenu {
                    center: Point { x: 100, y: 100 },
                    radius,
                    hovered: None,
                    panel,
                };
                assert_eq!(radial_shadow_alpha(&menu, &panel, menu.center), 0);
            }
            let menu = RadialMenu {
                center: Point { x: 100, y: 100 },
                radius: 20.0,
                hovered: None,
                panel,
            };
            assert_eq!(radial_shadow_alpha(&menu, &panel, Point { x: 0, y: 1 }), 0);
        }

        #[test]
        fn shared_theme_values_and_legacy_fallback_are_exact() {
            let expected = [
                ("pearl", [0xfcfbfc, 0x292c30, 0x292c30, 0xffffff, 0xd8d5dc]),
                (
                    "graphite",
                    [0x292d30, 0xffffff, 0xe4e7eb, 0x292d30, 0x555b62],
                ),
                ("ocean", [0x173b55, 0xffffff, 0x32a5f5, 0xffffff, 0x49657b]),
                ("butter", [0xfcfbfc, 0x49331d, 0xe7b838, 0x49331d, 0xddd6ca]),
                ("orange", [0xfce4ec, 0x372328, 0xff8a24, 0xffffff, 0xe9c7d2]),
                ("sand", [0xf7ebdf, 0x513726, 0xb97555, 0xffffff, 0xdfcfc1]),
            ];
            assert_eq!(themes().len(), expected.len());
            for (id, values) in expected {
                let colors = palette(id);
                assert_eq!(
                    [
                        colors.fill,
                        colors.ink,
                        colors.accent,
                        colors.selected_ink,
                        colors.divider
                    ],
                    values.map(|value| value | 0xff000000),
                    "{id}"
                );
            }
            for id in [
                "", "unknown", "legacy", "original", "gray", "colorful", "blue", "classic",
                "minimal",
            ] {
                assert_eq!(palette(id), palette("pearl"));
            }
        }

        #[test]
        fn dark_icons_and_disabled_panel_ink_follow_the_theme() {
            assert_eq!(colorref(0xff173b55), COLORREF(0x553b17));
            for id in ["graphite", "ocean"] {
                let colors = palette(id);
                assert_eq!(icon_ink(colors, false, true), 0xffffffff);
                assert_eq!(icon_ink(colors, true, true), colors.selected_ink);
                assert_eq!(
                    icon_ink(colors, true, false),
                    icon_ink(colors, false, false)
                );
                assert_ne!(icon_ink(colors, false, false), colors.fill);
                assert_ne!(icon_ink(colors, false, false), colors.ink);
            }
        }

        #[test]
        fn six_themes_are_distinct_and_hover_uses_accent() {
            let styles = ["pearl", "graphite", "ocean", "butter", "orange", "sand"];
            for (index, style) in styles.iter().enumerate() {
                let colors = palette(style);
                for other in &styles[index + 1..] {
                    assert_ne!(colors, palette(other));
                }
                for slot in 0..=RADIAL_CLEAR_SLOT {
                    assert_eq!(sector_fill(colors, slot, true), colors.accent);
                    assert_eq!(sector_fill(colors, slot, false), colors.fill);
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
                    panel: [RECT::default(); 7],
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
        fn rounded_panel_silhouettes_stay_inside_unchanged_hit_rectangles() {
            for radius in [78.0, 117.0, 156.0] {
                for (width, height) in [(1, 1), (2, 3), (30, 30), (64, 30)] {
                    let rect = RECT {
                        left: -100,
                        top: 50,
                        right: -100 + width,
                        bottom: 50 + height,
                    };
                    let points = panel_points(&rect, radius);
                    assert_eq!(points.len(), 36);
                    for point in points {
                        assert!(point.x >= rect.left && point.x < rect.right);
                        assert!(point.y >= rect.top && point.y < rect.bottom);
                    }
                }
            }
        }

        #[test]
        fn undo_redo_disabled_independently_without_disabling_other_buttons() {
            for (undo, redo) in [(false, false), (false, true), (true, false), (true, true)] {
                assert_eq!(panel_enabled(2, undo, redo), undo);
                assert_eq!(panel_enabled(3, undo, redo), redo);
                for index in [0, 1, 4, 5, 6] {
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
                    RECT {
                        left: 270,
                        top: 242,
                        right: 300,
                        bottom: 272,
                    },
                    RECT {
                        left: 304,
                        top: 242,
                        right: 334,
                        bottom: 272,
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
                for style in ["pearl", "graphite", "ocean", "butter", "orange", "sand"] {
                    for (variant, hovered, toolbar_hidden, keyboard_visible) in [
                        ("ring", 1, true, false),
                        ("clear", RADIAL_CLEAR_SLOT, false, true),
                        ("red", 13, false, true),
                        ("blue", 14, false, true),
                        ("custom-colors", 14, false, true),
                        ("panel", 15, false, true),
                        ("disabled", 16, true, true),
                        ("eye", 17, false, false),
                        ("keyboard", 18, true, true),
                        ("settings", 19, false, false),
                    ] {
                        pixels.fill(0);
                        menu.hovered = Some(hovered);
                        let quick_colors = if variant == "custom-colors" {
                            ["#12abcd".into(), "#000000".into()]
                        } else {
                            default_radial_colors()
                        };
                        draw_radial_styled(
                            HDC(canvas.memory_dc.0),
                            &menu,
                            &RADIAL_TOOLS,
                            style,
                            &panel,
                            true,
                            false,
                            toolbar_hidden,
                            keyboard_visible,
                            &quick_colors,
                        );
                        assert!(GdiFlush().as_bool());
                        for y in 175..186 {
                            for x in 175..186 {
                                assert_eq!(
                                    pixels[y * 360 + x] & 0xffffff,
                                    palette(style).fill & 0xffffff,
                                    "The center must stay blank for {style}"
                                );
                            }
                        }
                        for index in 0..2 {
                            let rect = panel[index];
                            let center = ((rect.top + rect.bottom) / 2 * 360
                                + (rect.left + rect.right) / 2)
                                as usize;
                            assert_eq!(
                                pixels[center] & 0xffffff,
                                swatch_argb(&quick_colors[index]) & 0xffffff
                            );
                        }
                        // Match production's premultiplied black shadow on transparent RGB only.
                        for (index, pixel) in pixels.iter_mut().enumerate() {
                            if *pixel & 0xffffff == 0 {
                                *pixel = u32::from(radial_shadow_alpha(
                                    &menu,
                                    &panel,
                                    Point {
                                        x: (index % 360) as i32,
                                        y: (index / 360) as i32,
                                    },
                                )) << 24;
                            } else {
                                *pixel |= 0xff000000;
                            }
                        }
                        assert_eq!(pixels[0], 0);
                        assert_eq!(pixels[180 * 360 + 264], 0);
                        assert_eq!(pixels[359 * 360 + 359], 0);
                        let painted = pixels.iter().filter(|pixel| **pixel != 0).count();
                        assert!((22_000..=35_000).contains(&painted), "{style}: {painted}");
                        assert!((1..=60).contains(&(pixels[261 * 360 + 180] >> 24)));
                        for (index, rect) in panel.iter().enumerate() {
                            if hovered == 13 + index && panel_enabled(index, true, false) {
                                assert_eq!(
                                    pixels[((rect.top + 3) * 360 + rect.left + 3) as usize]
                                        & 0xffffff,
                                    palette(style).accent & 0xffffff
                                );
                            }
                        }
                        let undo_bg = pixels[177 * 360 + 273] & 0xffffff;
                        let redo_bg = pixels[177 * 360 + 307] & 0xffffff;
                        if hovered == 15 {
                            assert_eq!(undo_bg, palette(style).accent & 0xffffff);
                        } else {
                            assert_eq!(undo_bg, palette(style).fill & 0xffffff);
                        }
                        assert_eq!(redo_bg, palette(style).fill & 0xffffff);
                        if hovered == 1 {
                            let (start, sweep) = radial_sector(1);
                            let sample = point(&menu, start + sweep / 2.0, menu.radius * 0.54);
                            assert_eq!(
                                pixels[(sample.y * 360 + sample.x) as usize] & 0xffffff,
                                palette(style).accent & 0xffffff
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
