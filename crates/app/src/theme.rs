use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, Response, RichText,
    Sense, Stroke, Vec2,
};

pub const BG: Color32 = Color32::from_rgb(0x28, 0x18, 0x00);
pub const GOLD: Color32 = Color32::from_rgb(0xd8, 0xa2, 0x48);
pub const BRIGHT: Color32 = Color32::from_rgb(0xf0, 0xd0, 0xa0);
pub const MUTED: Color32 = Color32::from_rgb(0xa8, 0x78, 0x30);
pub const DIM: Color32 = Color32::from_rgb(0x8a, 0x62, 0x28);
pub const FAINT: Color32 = Color32::from_rgb(0x8a, 0x5a, 0x10);
pub const RED: Color32 = Color32::from_rgb(0xbb, 0x00, 0x00);
pub const RED_HOT: Color32 = Color32::from_rgb(0xff, 0x00, 0x00);
pub const LINE: Color32 = Color32::from_rgb(0x4a, 0x2a, 0x00);
pub const LINE_SOFT: Color32 = Color32::from_rgb(0x3a, 0x24, 0x00);
pub const FIELD: Color32 = Color32::from_rgb(0x1a, 0x0e, 0x00);
pub const BEVEL_HI: Color32 = Color32::from_rgb(0x6a, 0x4a, 0x10);
pub const BEVEL_LO: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);

pub fn regular_family() -> FontFamily {
    FontFamily::Proportional
}

pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}

pub fn italic_family() -> FontFamily {
    FontFamily::Name("italic".into())
}

pub fn regular_font(size: f32) -> FontId {
    FontId::new(size, regular_family())
}

pub fn bold_font(size: f32) -> FontId {
    FontId::new(size, bold_family())
}

pub fn italic_font(size: f32) -> FontId {
    FontId::new(size, italic_family())
}

pub fn t(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).size(size).family(regular_family())
}

pub fn b(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).size(size).family(bold_family())
}

pub fn i(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).size(size).family(italic_family())
}

pub fn tc(text: impl Into<String>, size: f32, color: Color32) -> RichText {
    t(text, size).color(color)
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let regular = FontData::from_static(include_bytes!("../../../assets/fonts/LiberationSans-Regular.ttf"));
    let bold = FontData::from_static(include_bytes!("../../../assets/fonts/LiberationSans-Bold.ttf"));
    let italic = FontData::from_static(include_bytes!("../../../assets/fonts/LiberationSans-Italic.ttf"));
    fonts.font_data.insert("ls-regular".into(), regular.into());
    fonts.font_data.insert("ls-bold".into(), bold.into());
    fonts.font_data.insert("ls-italic".into(), italic.into());

    let fallback: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let proportional: Vec<String> = std::iter::once("ls-regular".to_string())
        .chain(fallback.iter().cloned())
        .collect();
    fonts
        .families
        .insert(FontFamily::Proportional, proportional);
    fonts.families.insert(
        FontFamily::Name("bold".into()),
        std::iter::once("ls-bold".to_string())
            .chain(fallback.iter().cloned())
            .collect(),
    );
    fonts.families.insert(
        FontFamily::Name("italic".into()),
        std::iter::once("ls-italic".to_string())
            .chain(fallback.iter().cloned())
            .collect(),
    );
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(GOLD);
    style.visuals.panel_fill = Color32::TRANSPARENT;
    style.visuals.window_fill = FIELD;
    style.visuals.extreme_bg_color = FIELD;
    style.visuals.faint_bg_color = Color32::TRANSPARENT;
    style.visuals.code_bg_color = FIELD;
    style.visuals.selection.bg_fill = Color32::from_rgb(0x4a, 0x10, 0x08);
    style.visuals.selection.stroke = Stroke::new(1.0, RED);
    style.visuals.window_stroke = Stroke::new(1.0, BEVEL_HI);
    style.visuals.window_corner_radius = egui::CornerRadius::ZERO;
    style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
    style.visuals.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
    style.visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE_SOFT);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, GOLD);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x33, 0x20, 0x0a);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BEVEL_HI);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, BRIGHT);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(0x40, 0x28, 0x10);
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, RED);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, BRIGHT);
    style.visuals.widgets.open.bg_fill = Color32::from_rgb(0x33, 0x20, 0x0a);
    style.visuals.widgets.open.bg_stroke = Stroke::new(1.0, BEVEL_HI);
    style.visuals.widgets.open.fg_stroke = Stroke::new(1.0, BRIGHT);
    style.spacing.item_spacing = Vec2::new(8.0, 4.0);
    style.spacing.button_padding = Vec2::new(6.0, 3.0);
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::new(13.0, regular_family()),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::new(13.0, regular_family()),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        FontId::new(11.0, regular_family()),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(22.0, bold_family()),
    );
    ctx.set_style(style);
}

/// Red underlined text link, exact CSS-ish hit area.
pub fn link(ui: &mut egui::Ui, text: &str, size: f32, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(size, regular_family()),
        color,
    );
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::click());
    let hover = response.hovered();
    let color = if hover { RED_HOT } else { color };
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(size, regular_family()),
        color,
    );
    let y = rect.top() + (rect.height() - galley.size().y) / 2.0;
    ui.painter()
        .galley(Pos2::new(rect.left(), y), galley, color);
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), rect.bottom() - 1.0),
            Pos2::new(rect.right(), rect.bottom() - 1.0),
        ],
        Stroke::new(1.0, color),
    );
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// Gold underlined link (current nav item is bright and un-underlined).
pub fn nav_link(ui: &mut egui::Ui, text: &str, current: bool) -> Response {
    let (color, underline) = if current {
        (BRIGHT, false)
    } else {
        (RED, true)
    };
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(13.0, if current { bold_family() } else { regular_family() }),
        color,
    );
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::click());
    let hover = response.hovered() && !current;
    let color = if hover { RED_HOT } else { color };
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(13.0, if current { bold_family() } else { regular_family() }),
        color,
    );
    let y = rect.top() + (rect.height() - galley.size().y) / 2.0;
    ui.painter().galley(Pos2::new(rect.left(), y), galley, color);
    if underline {
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, color),
        );
    }
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// 1998 bevel button.
pub fn bevel(ui: &mut egui::Ui, text: &str, primary: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(12.0, if primary { bold_family() } else { regular_family() }),
        if primary { BRIGHT } else { GOLD },
    );
    let size = Vec2::new(galley.size().x + 22.0, 25.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = response.hovered();
    let fill = if hover {
        Color32::from_rgb(0x40, 0x28, 0x10)
    } else if primary {
        Color32::from_rgb(0x33, 0x20, 0x0b)
    } else {
        Color32::from_rgb(0x26, 0x17, 0x06)
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    let hi = if response.is_pointer_button_down_on() {
        BEVEL_LO
    } else {
        BEVEL_HI
    };
    let lo = if response.is_pointer_button_down_on() {
        BEVEL_HI
    } else {
        BEVEL_LO
    };
    ui.painter().line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, hi));
    ui.painter().line_segment([rect.left_top(), rect.left_bottom()], Stroke::new(1.0, hi));
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, lo),
    );
    ui.painter().line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0, lo),
    );
    let color = if hover { BRIGHT } else { GOLD };
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(12.0, if primary { bold_family() } else { regular_family() }),
        color,
    );
    ui.painter().galley(
        rect.center() - galley.size() / 2.0,
        galley,
        color,
    );
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

pub fn hairline(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, LINE),
    );
}

pub fn soft_rule(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, LINE_SOFT),
    );
}

pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(b(text, 17.0).color(GOLD));
}

pub fn empty_note(ui: &mut egui::Ui, text: &str, indent: f32) {
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.label(i(text, 13.0).color(MUTED));
    });
}

/// Paint a horizontal strikethrough over a rect.
pub fn strike(painter: &egui::Painter, rect: Rect, color: Color32) {
    let mid = rect.center().y;
    painter.line_segment(
        [
            Pos2::new(rect.left(), mid),
            Pos2::new(rect.right(), mid),
        ],
        Stroke::new(1.0, color),
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Caret {
    Right,
    Down,
}

pub fn paint_caret(painter: &egui::Painter, center: Pos2, size: f32, color: Color32, caret: Caret) {
    let s = size;
    let points = match caret {
        Caret::Right => vec![
            Pos2::new(center.x - s * 0.45, center.y - s * 0.6),
            Pos2::new(center.x - s * 0.45, center.y + s * 0.6),
            Pos2::new(center.x + s * 0.55, center.y),
        ],
        Caret::Down => vec![
            Pos2::new(center.x - s * 0.6, center.y - s * 0.45),
            Pos2::new(center.x + s * 0.6, center.y - s * 0.45),
            Pos2::new(center.x, center.y + s * 0.55),
        ],
    };
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

pub fn paint_x(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    let h = size / 2.0;
    painter.line_segment(
        [
            Pos2::new(center.x - h, center.y - h),
            Pos2::new(center.x + h, center.y + h),
        ],
        Stroke::new(1.0_f32, color),
    );
    painter.line_segment(
        [
            Pos2::new(center.x + h, center.y - h),
            Pos2::new(center.x - h, center.y + h),
        ],
        Stroke::new(1.0_f32, color),
    );
}

pub fn paint_plus(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    let h = size / 2.0;
    painter.line_segment(
        [
            Pos2::new(center.x - h, center.y),
            Pos2::new(center.x + h, center.y),
        ],
        Stroke::new(1.0_f32, color),
    );
    painter.line_segment(
        [
            Pos2::new(center.x, center.y - h),
            Pos2::new(center.x, center.y + h),
        ],
        Stroke::new(1.0_f32, color),
    );
}

pub fn paint_pencil(painter: &egui::Painter, rect: Rect, color: Color32) {
    let a = Pos2::new(rect.left() + 1.0, rect.bottom() - 1.0);
    let b = Pos2::new(rect.right() - 1.0, rect.top() + 1.0);
    painter.line_segment([a, b], Stroke::new(1.0_f32, color));
    painter.line_segment(
        [
            Pos2::new(b.x - 2.0, b.y + 2.0),
            Pos2::new(b.x + 1.0, b.y - 1.0),
        ],
        Stroke::new(1.0_f32, color),
    );
}

pub fn caret_link(ui: &mut egui::Ui, text: &str, caret: Caret) -> Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(13.0, regular_family()),
        RED,
    );
    let size = Vec2::new(galley.size().x + 12.0, galley.size().y);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = response.hovered();
    let color = if hover { RED_HOT } else { RED };
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(13.0, regular_family()),
        color,
    );
    let cy = rect.center().y;
    paint_caret(ui.painter(), Pos2::new(rect.left() + 3.0, cy), 7.0, color, caret);
    let text_left = rect.left() + 12.0;
    let y = rect.top() + (rect.height() - galley.size().y) / 2.0;
    ui.painter()
        .galley(Pos2::new(text_left, y), galley, color);
    ui.painter().line_segment(
        [
            Pos2::new(text_left, rect.bottom() - 1.0),
            Pos2::new(rect.right(), rect.bottom() - 1.0),
        ],
        Stroke::new(1.0_f32, color),
    );
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}
