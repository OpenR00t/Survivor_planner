//! Gruvbox dark, green-forward, with CaskaydiaCove picked up from the system if installed.

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Stroke};
use std::path::PathBuf;
use std::sync::Arc;

pub const BG0_H: Color32 = Color32::from_rgb(0x1d, 0x20, 0x21);
pub const BG0: Color32 = Color32::from_rgb(0x28, 0x28, 0x28);
pub const BG1: Color32 = Color32::from_rgb(0x3c, 0x38, 0x36);
pub const BG2: Color32 = Color32::from_rgb(0x50, 0x49, 0x45);
pub const BG3: Color32 = Color32::from_rgb(0x66, 0x5c, 0x54);
pub const FG: Color32 = Color32::from_rgb(0xeb, 0xdb, 0xb2);
pub const FG4: Color32 = Color32::from_rgb(0xa8, 0x99, 0x84);
pub const GRAY: Color32 = Color32::from_rgb(0x92, 0x83, 0x74);
pub const GREEN: Color32 = Color32::from_rgb(0xb8, 0xbb, 0x26);
pub const GREEN_DIM: Color32 = Color32::from_rgb(0x79, 0x74, 0x0e);
pub const AQUA: Color32 = Color32::from_rgb(0x8e, 0xc0, 0x7c);
pub const YELLOW: Color32 = Color32::from_rgb(0xfa, 0xbd, 0x2f);
pub const ORANGE: Color32 = Color32::from_rgb(0xfe, 0x80, 0x19);
pub const RED: Color32 = Color32::from_rgb(0xfb, 0x49, 0x34);
pub const BLUE: Color32 = Color32::from_rgb(0x83, 0xa5, 0x98);

pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(FG);
    v.panel_fill = BG0;
    v.window_fill = BG0;
    v.extreme_bg_color = BG0_H;
    v.faint_bg_color = Color32::from_rgb(0x2e, 0x2c, 0x2b);
    v.code_bg_color = BG1;
    v.hyperlink_color = AQUA;
    v.warn_fg_color = ORANGE;
    v.error_fg_color = RED;
    v.selection.bg_fill = Color32::from_rgb(0x98, 0x97, 0x1a);
    v.selection.stroke = Stroke::new(1.0_f32, BG0_H);
    v.window_stroke = Stroke::new(1.0_f32, BG2);

    v.widgets.noninteractive.bg_fill = BG0;
    v.widgets.noninteractive.weak_bg_fill = BG0;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BG1);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, FG);

    v.widgets.inactive.bg_fill = BG1;
    v.widgets.inactive.weak_bg_fill = BG1;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, FG);

    v.widgets.hovered.bg_fill = BG2;
    v.widgets.hovered.weak_bg_fill = BG2;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, GREEN);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5_f32, FG);

    v.widgets.active.bg_fill = BG3;
    v.widgets.active.weak_bg_fill = BG3;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, GREEN);
    v.widgets.active.fg_stroke = Stroke::new(2.0_f32, FG);

    v.widgets.open.bg_fill = BG1;
    v.widgets.open.weak_bg_fill = BG1;
    ctx.set_visuals(v);

    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.5));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.5));
    });

    install_fonts(ctx);
}

/// Look for CaskaydiaCove Nerd Font files in the usual Windows/Linux/macOS font folders.
/// Proportional text uses the "Propo" variant if present; monospace uses "Mono".
fn install_fonts(ctx: &egui::Context) {
    let files = font_files();
    let pick = |want: &[&str], avoid: &[&str]| -> Option<PathBuf> {
        files
            .iter()
            .find(|p| {
                let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                n.contains("caskaydiacove")
                    && n.contains("regular")
                    && !n.contains("italic")
                    && want.iter().all(|w| n.contains(w))
                    && avoid.iter().all(|a| !n.contains(a))
            })
            .cloned()
    };

    let prop = pick(&["propo"], &[]).or_else(|| pick(&[], &["mono", "propo"])).or_else(|| pick(&[], &[]));
    let mono = pick(&["mono"], &[]).or_else(|| pick(&[], &["propo"]));

    let mut fonts = FontDefinitions::default();
    let mut changed = false;
    for (key, path, family) in
        [("cove-prop", prop, FontFamily::Proportional), ("cove-mono", mono, FontFamily::Monospace)]
    {
        let Some(path) = path else { continue };
        let Ok(bytes) = std::fs::read(&path) else { continue };
        fonts.font_data.insert(key.to_owned(), Arc::new(FontData::from_owned(bytes)));
        if let Some(list) = fonts.families.get_mut(&family) {
            list.insert(0, key.to_owned());
        }
        changed = true;
    }
    if changed {
        ctx.set_fonts(fonts);
    }
}

fn font_files() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(windir) = std::env::var("WINDIR") {
        dirs.push(PathBuf::from(windir).join("Fonts"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts"));
    }
    if let Some(h) = dirs::home_dir() {
        dirs.push(h.join(".local/share/fonts"));
        dirs.push(h.join(".fonts"));
        dirs.push(h.join("Library/Fonts"));
    }
    dirs.push(PathBuf::from("/usr/share/fonts"));

    let mut out = Vec::new();
    for d in dirs {
        walk(&d, 0, &mut out);
    }
    out.sort();
    out
}

fn walk(dir: &std::path::Path, depth: u8, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth < 3 {
                walk(&p, depth + 1, out);
            }
        } else if p
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| x.eq_ignore_ascii_case("ttf") || x.eq_ignore_ascii_case("otf"))
        {
            out.push(p);
        }
    }
}
