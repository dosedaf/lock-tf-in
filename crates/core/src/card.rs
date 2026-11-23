use anyhow::Result;
use chrono::{Datelike, Duration, Local};
use fontdue::{Font, FontSettings};
use image::{ImageEncoder, Rgb, RgbImage};

use crate::db::Db;
use crate::model::CardKind;

const CERAMIC_GIF: &[u8] = include_bytes!("../../../assets/img/ceramic.gif");
const FONT_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Regular.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Bold.ttf");
const FONT_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Italic.ttf");

const W: u32 = 1080;
const H: u32 = 1080;
const MARGIN: f32 = 90.0;
const HI: Rgb<u8> = Rgb([240, 208, 160]);
const DIM: Rgb<u8> = Rgb([168, 120, 48]);

pub fn hm(minutes: i64) -> String {
    if minutes == 0 {
        return "0m".into();
    }
    let h = minutes / 60;
    let m = minutes % 60;
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

fn font(bytes: &'static [u8]) -> Font {
    Font::from_bytes(bytes, FontSettings::default()).expect("bundled font")
}

fn text_width(font: &Font, text: &str, size: f32) -> f32 {
    text.chars()
        .map(|c| font.metrics(c, size).advance_width)
        .sum()
}

fn draw_text_centered(
    img: &mut RgbImage,
    font: &Font,
    text: &str,
    size: f32,
    cx: f32,
    cy: f32,
    color: Rgb<u8>,
) {
    let width = text_width(font, text, size);
    let line = font
        .horizontal_line_metrics(size)
        .map(|m| (m.ascent, m.descent, m.line_gap))
        .unwrap_or((size, -size * 0.25, 0.0));
    let baseline = cy + (line.0 + line.1) / 2.0;
    let mut x = cx - width / 2.0;
    for ch in text.chars() {
        let (m, bitmap) = font.rasterize(ch, size);
        if m.width > 0 && m.height > 0 {
            let top = baseline - (m.ymin + m.height as i32) as f32;
            let gx = x.round() as i32 + m.xmin;
            let gy = top.round() as i32;
            for row in 0..m.height {
                for col in 0..m.width {
                    let cov = bitmap[row * m.width + col];
                    if cov == 0 {
                        continue;
                    }
                    let px = gx + col as i32;
                    let py = gy + row as i32;
                    if px < 0 || py < 0 || px >= W as i32 || py >= H as i32 {
                        continue;
                    }
                    let p = img.get_pixel_mut(px as u32, py as u32);
                    let a = cov as f32 / 255.0;
                    for i in 0..3 {
                        p[i] = (p[i] as f32 * (1.0 - a) + color[i] as f32 * a).round() as u8;
                    }
                }
            }
        }
        x += m.advance_width;
    }
}

fn render(big: &str, label: &str) -> Result<Vec<u8>> {
    let tile = image::load_from_memory(CERAMIC_GIF)?.to_rgb8();
    let mut img = RgbImage::new(W, H);
    let (tw, th) = (tile.width().max(1), tile.height().max(1));
    for y in (0..H).step_by(th as usize) {
        for x in (0..W).step_by(tw as usize) {
            for ty in 0..th {
                for tx in 0..tw {
                    let (px, py) = (x + tx, y + ty);
                    if px >= W || py >= H {
                        continue;
                    }
                    let src = tile.get_pixel(tx, ty);
                    // dim the clay 0.85x, keep the gold legible
                    let out = Rgb([
                        (src[0] as f32 * 0.85) as u8,
                        (src[1] as f32 * 0.85) as u8,
                        (src[2] as f32 * 0.85) as u8,
                    ]);
                    img.put_pixel(px, py, out);
                }
            }
        }
    }

    let f_label = font(FONT_ITALIC);
    let f_bold = font(FONT_BOLD);
    let f_cap = font(FONT_REGULAR);

    let cx = W as f32 / 2.0;
    if !label.is_empty() {
        draw_text_centered(&mut img, &f_label, label, 38.0, cx, H as f32 / 2.0 - 250.0, DIM);
    }

    let mut size = 210.0f32;
    while size > 90.0 && text_width(&f_bold, big, size) > W as f32 - 2.0 * MARGIN {
        size -= 10.0;
    }
    draw_text_centered(&mut img, &f_bold, big, size, cx, H as f32 / 2.0 - 10.0, HI);
    draw_text_centered(
        &mut img,
        &f_cap,
        "focused",
        30.0,
        cx,
        H as f32 / 2.0 + 160.0,
        DIM,
    );

    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut out);
    encoder.write_image(img.as_raw(), W, H, image::ExtendedColorType::Rgb8)?;
    Ok(out)
}

const MONTHS: [&str; 12] = [
    "january", "february", "march", "april", "may", "june", "july", "august", "september",
    "october", "november", "december",
];

impl Db {
    pub fn card_png(&self, kind: &CardKind) -> Result<Vec<u8>> {
        let today = Local::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();
        match kind {
            CardKind::Day => {
                let focused: i64 = self.conn.query_row(
                    "SELECT COALESCE(SUM(minutes),0) FROM pomo_periods WHERE substr(started_at,1,10)=?",
                    [&today_str],
                    |r| r.get(0),
                )?;
                render(
                    &hm(focused),
                    &today.format("%d %b %Y").to_string().to_lowercase(),
                )
            }
            CardKind::Week => {
                let since = (today - Duration::days(6))
                    .format("%Y-%m-%d")
                    .to_string();
                let focused: i64 = self.conn.query_row(
                    "SELECT COALESCE(SUM(minutes),0) FROM pomo_periods WHERE substr(started_at,1,10)>=?",
                    [&since],
                    |r| r.get(0),
                )?;
                render(&hm(focused), "last 7 days")
            }
            CardKind::Month { year, month } => {
                if !(1..=12).contains(month) || !(1990..=2100).contains(year) {
                    anyhow::bail!("invalid month");
                }
                let key = format!("{year:04}-{month:02}");
                let focused: i64 = self.conn.query_row(
                    "SELECT COALESCE(SUM(minutes),0) FROM pomo_periods WHERE substr(started_at,1,7)=?",
                    [&key],
                    |r| r.get(0),
                )?;
                let label = format!("{} {}", MONTHS[*month as usize - 1], year);
                render(&hm(focused), &label)
            }
        }
    }
}

// keep the compiler honest about the date helper being used
#[allow(dead_code)]
fn _unused(d: chrono::NaiveDate) -> i32 {
    d.year()
}
