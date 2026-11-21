#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{self, FilterType};
use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage};
use std::{io::Write, path::Path, sync::Arc};

const PAPERS: [(&str, f32, f32); 6] = [("Letter", 8.5, 11.0), ("Legal", 8.5, 14.0), ("Tabloid", 11.0, 17.0), ("A5", 5.83, 8.27), ("A4", 8.27, 11.69), ("A3", 11.69, 16.54)];
const ACCENT: Color32 = Color32::from_rgb(59, 130, 246);
const RED: Color32 = Color32::from_rgb(230, 60, 60);
const UV: Rect = Rect { min: pos2(0.0, 0.0), max: pos2(1.0, 1.0) };

#[derive(PartialEq, Clone, Copy)]
enum Tab { Edit, Slice }
#[derive(PartialEq, Clone, Copy)]
enum Tool { Crop, Wand }

/// w,h = paper; m = margin; o = overlap; pw,ph = printable area; bw,bh = poster; iw,ih,ix,iy = image rect on poster (inches)
struct Lay { w: f32, h: f32, m: f32, o: f32, pw: f32, ph: f32, cols: u32, rows: u32, bw: f32, bh: f32, iw: f32, ih: f32, ix: f32, iy: f32 }

struct App {
    img: Option<RgbaImage>, hist: Vec<RgbaImage>, tex: Option<TextureHandle>, dirty: bool,
    logo: TextureHandle, hero: TextureHandle, tab: Tab, tool: Tool, tol: u32, crop: Option<(Pos2, Pos2)>,
    paper: usize, land: bool, margin: f32, overlap: f32, grid: bool, cols: u32, rows: u32, wid: f32, hei: f32,
    cut: bool, lab: bool, dpi: u32, status: String,
}

fn tex_from(ctx: &egui::Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let im = image::load_from_memory(bytes).unwrap().to_rgba8();
    ctx.load_texture(name, egui::ColorImage::from_rgba_unmultiplied([im.width() as usize, im.height() as usize], im.as_raw()), TextureOptions::LINEAR)
}
fn btn(ui: &mut egui::Ui, t: &str, primary: bool) -> bool {
    let mut b = egui::Button::new(egui::RichText::new(t).size(14.0));
    if primary { b = b.fill(ACCENT); }
    ui.add_sized([ui.available_width(), 34.0], b).clicked()
}
fn num(ui: &mut egui::Ui, label: &str, v: &mut f32, r: std::ops::RangeInclusive<f32>) {
    ui.horizontal(|ui| { ui.label(label); ui.add(egui::DragValue::new(v).speed(0.05).clamp_range(r).suffix(" in")); });
}
fn checker(ui: &egui::Ui, r: Rect) {
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, Color32::from_gray(60));
    let n = 16.0;
    let (mut y, mut x);
    y = 0;
    while r.min.y + y as f32 * n < r.max.y {
        x = 0;
        while r.min.x + x as f32 * n < r.max.x {
            if (x + y) % 2 == 0 { p.rect_filled(Rect::from_min_size(pos2(r.min.x + x as f32 * n, r.min.y + y as f32 * n), vec2(n, n)), 0.0, Color32::from_gray(85)); }
            x += 1;
        }
        y += 1;
    }
}
fn flood(img: &mut RgbaImage, x: u32, y: u32, tol: u32) {
    let (w, h) = img.dimensions();
    let b = *img.get_pixel(x.min(w - 1), y.min(h - 1));
    if b[3] == 0 { return; }
    let mut st = vec![(x.min(w - 1), y.min(h - 1))];
    while let Some((px, py)) = st.pop() {
        let p = img.get_pixel_mut(px, py);
        if p[3] == 0 { continue; }
        let d = (p[0] as i32 - b[0] as i32).abs() + (p[1] as i32 - b[1] as i32).abs() + (p[2] as i32 - b[2] as i32).abs();
        if d > tol as i32 * 3 { continue; }
        p[3] = 0;
        if px > 0 { st.push((px - 1, py)); }
        if px + 1 < w { st.push((px + 1, py)); }
        if py > 0 { st.push((px, py - 1)); }
        if py + 1 < h { st.push((px, py + 1)); }
    }
}
fn obj(out: &mut Vec<u8>, offs: &mut [usize], id: usize, body: &[u8]) {
    offs[id] = out.len();
    write!(out, "{} 0 obj\n", id).unwrap();
    out.extend_from_slice(body);
    out.extend_from_slice(b"\nendobj\n");
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        let ctx = &cc.egui_ctx;
        let mut v = egui::Visuals::dark();
        v.panel_fill = Color32::from_rgb(23, 24, 28);
        v.selection.bg_fill = ACCENT;
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] { w.rounding = egui::Rounding::same(8.0); }
        ctx.set_visuals(v);
        ctx.style_mut(|s| { s.spacing.item_spacing = vec2(8.0, 9.0); s.spacing.button_padding = vec2(10.0, 6.0); });
        Self {
            img: None, hist: vec![], tex: None, dirty: false,
            logo: tex_from(ctx, "logo", include_bytes!("../assets/logo.png")), hero: tex_from(ctx, "hero", include_bytes!("../assets/hero.png")),
            tab: Tab::Edit, tool: Tool::Crop, tol: 30, crop: None, paper: 0, land: false, margin: 0.25, overlap: 0.25,
            grid: true, cols: 3, rows: 3, wid: 36.0, hei: 48.0, cut: true, lab: true, dpi: 200, status: "Open or drop an image to begin.".into(),
        }
    }
    fn open_path(&mut self, p: &Path) {
        match image::open(p) {
            Ok(d) => {
                self.img = Some(d.to_rgba8()); self.hist.clear(); self.crop = None; self.tab = Tab::Edit; self.dirty = true;
                self.status = format!("Loaded {}", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
            }
            Err(e) => self.status = format!("Could not open image: {e}"),
        }
    }
    fn refresh(&mut self, ctx: &egui::Context) {
        if let Some(img) = &self.img {
            let (w, h) = img.dimensions();
            let k = (2048.0 / w.max(h) as f32).min(1.0);
            let small = imageops::resize(img, ((w as f32 * k) as u32).max(1), ((h as f32 * k) as u32).max(1), FilterType::Triangle);
            let ci = egui::ColorImage::from_rgba_unmultiplied([small.width() as usize, small.height() as usize], small.as_raw());
            self.tex = Some(ctx.load_texture("img", ci, TextureOptions::LINEAR));
        }
        self.dirty = false;
    }
    fn push_hist(&mut self) {
        if let Some(i) = &self.img { self.hist.push(i.clone()); if self.hist.len() > 10 { self.hist.remove(0); } }
    }
    fn apply_crop(&mut self) {
        let Some((a, b)) = self.crop else { self.status = "Drag on the image to choose a crop area.".into(); return; };
        let (x0, x1, y0, y1) = (a.x.min(b.x) as u32, a.x.max(b.x) as u32, a.y.min(b.y) as u32, a.y.max(b.y) as u32);
        if x1 <= x0 + 1 || y1 <= y0 + 1 { return; }
        self.push_hist();
        let n = self.img.as_ref().map(|i| imageops::crop_imm(i, x0, y0, x1 - x0, y1 - y0).to_image());
        if n.is_some() { self.img = n; }
        self.crop = None; self.dirty = true;
    }
    fn wand(&mut self, x: u32, y: u32) {
        self.push_hist();
        let tol = self.tol;
        if let Some(i) = &mut self.img { flood(i, x, y, tol); }
        self.dirty = true;
    }
    fn auto_bg(&mut self) {
        self.push_hist();
        let tol = self.tol;
        if let Some(i) = &mut self.img {
            let (w, h) = i.dimensions();
            for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] { flood(i, x, y, tol); }
        }
        self.dirty = true;
        self.status = "Background removed from the edges. Raise tolerance for more, or use Remove colour to touch up.".into();
    }
    fn layout(&self) -> Option<Lay> {
        let img = self.img.as_ref()?;
        let (mut w, mut h) = (PAPERS[self.paper].1, PAPERS[self.paper].2);
        if self.land { std::mem::swap(&mut w, &mut h); }
        let m = self.margin;
        let (pw, ph) = (w - 2.0 * m, h - 2.0 * m);
        let o = self.overlap.clamp(0.0, pw.min(ph) * 0.4);
        let (cols, rows, bw, bh) = if self.grid {
            let (c, r) = (self.cols.max(1), self.rows.max(1));
            (c, r, c as f32 * pw - (c - 1) as f32 * o, r as f32 * ph - (r - 1) as f32 * o)
        } else {
            let (bw, bh) = (self.wid.max(1.0), self.hei.max(1.0));
            (((bw - o) / (pw - o)).ceil().max(1.0) as u32, ((bh - o) / (ph - o)).ceil().max(1.0) as u32, bw, bh)
        };
        let k = (bw / img.width() as f32).min(bh / img.height() as f32);
        let (iw, ih) = (img.width() as f32 * k, img.height() as f32 * k);
        Some(Lay { w, h, m, o, pw, ph, cols, rows, bw, bh, iw, ih, ix: (bw - iw) / 2.0, iy: (bh - ih) / 2.0 })
    }
    /// Renders the printable area of one sheet as JPEG bytes.
    fn tile(&self, img: &RgbaImage, l: &Lay, r: u32, c: u32) -> Option<(Vec<u8>, u32, u32)> {
        let dpi = self.dpi as f32;
        let (pxw, pxh) = ((l.pw * dpi).round() as u32, (l.ph * dpi).round() as u32);
        let mut canvas = RgbaImage::from_pixel(pxw, pxh, Rgba([255, 255, 255, 255]));
        let (x0, y0) = (c as f32 * (l.pw - l.o), r as f32 * (l.ph - l.o));
        let (ax, bx) = (x0.max(l.ix), (x0 + l.pw).min(l.ix + l.iw));
        let (ay, by) = (y0.max(l.iy), (y0 + l.ph).min(l.iy + l.ih));
        if bx > ax && by > ay {
            let k = img.width() as f32 / l.iw;
            let sx = (((ax - l.ix) * k).round() as u32).min(img.width() - 1);
            let sy = (((ay - l.iy) * k).round() as u32).min(img.height() - 1);
            let sw = (((bx - ax) * k).round().max(1.0) as u32).min(img.width() - sx);
            let sh = (((by - ay) * k).round().max(1.0) as u32).min(img.height() - sy);
            let sub = imageops::crop_imm(img, sx, sy, sw, sh).to_image();
            let rs = imageops::resize(&sub, ((bx - ax) * dpi).round().max(1.0) as u32, ((by - ay) * dpi).round().max(1.0) as u32, FilterType::Triangle);
            imageops::overlay(&mut canvas, &rs, ((ax - x0) * dpi).round() as i64, ((ay - y0) * dpi).round() as i64);
        }
        let rgb = image::DynamicImage::ImageRgba8(canvas).to_rgb8();
        let mut jpg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpg, 90).write_image(rgb.as_raw(), pxw, pxh, ExtendedColorType::Rgb8).ok()?;
        Some((jpg, pxw, pxh))
    }
    fn make_pdf(&self) -> Option<Vec<u8>> {
        let img = self.img.as_ref()?;
        let l = self.layout()?;
        let n = (l.cols * l.rows) as usize;
        let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut offs = vec![0usize; 4 + 3 * n];
        let kids: String = (0..n).map(|i| format!("{} 0 R ", 4 + 3 * i)).collect();
        obj(&mut out, &mut offs, 1, b"<< /Type /Catalog /Pages 2 0 R >>");
        obj(&mut out, &mut offs, 2, format!("<< /Type /Pages /Count {n} /Kids [{kids}] >>").as_bytes());
        obj(&mut out, &mut offs, 3, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
        let (x, y, w, hh) = (l.m * 72.0, (l.h - l.m - l.ph) * 72.0, l.pw * 72.0, l.ph * 72.0);
        for r in 0..l.rows {
            for c in 0..l.cols {
                let i = (r * l.cols + c) as usize;
                let (pid, cid, iid) = (4 + 3 * i, 5 + 3 * i, 6 + 3 * i);
                let (jpg, pxw, pxh) = self.tile(img, &l, r, c)?;
                let mut s = format!("q {w:.2} 0 0 {hh:.2} {x:.2} {y:.2} cm /Im0 Do Q\n");
                if self.cut { s += &format!("0.4 w [6 4] 0 d {x:.2} {y:.2} {w:.2} {hh:.2} re S [] 0 d\n"); }
                if self.lab {
                    let ty = if l.m > 0.2 { (l.h - l.m) * 72.0 + 4.0 } else { (l.h - l.m) * 72.0 - 10.0 };
                    s += &format!("BT /F1 8 Tf {:.2} {ty:.2} Td (Sheet {} of {}  -  Row {}, Col {}) Tj ET\n", x + 2.0, i + 1, n, r + 1, c + 1);
                }
                obj(&mut out, &mut offs, pid, format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Contents {cid} 0 R /Resources << /Font << /F1 3 0 R >> /XObject << /Im0 {iid} 0 R >> >> >>", l.w * 72.0, l.h * 72.0).as_bytes());
                let mut cb = format!("<< /Length {} >>\nstream\n", s.len()).into_bytes();
                cb.extend_from_slice(s.as_bytes()); cb.extend_from_slice(b"endstream");
                obj(&mut out, &mut offs, cid, &cb);
                let mut ib = format!("<< /Type /XObject /Subtype /Image /Width {pxw} /Height {pxh} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n", jpg.len()).into_bytes();
                ib.extend_from_slice(&jpg); ib.extend_from_slice(b"\nendstream");
                obj(&mut out, &mut offs, iid, &ib);
            }
        }
        let xr = out.len();
        write!(out, "xref\n0 {}\n0000000000 65535 f \n", 4 + 3 * n).unwrap();
        for id in 1..(4 + 3 * n) { write!(out, "{:010} 00000 n \n", offs[id]).unwrap(); }
        write!(out, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF", 4 + 3 * n, xr).unwrap();
        Some(out)
    }
    fn print(&mut self) {
        let Some(b) = self.make_pdf() else { return; };
        let p = std::env::temp_dir().join("PosterCraft-print.pdf");
        if let Err(e) = std::fs::write(&p, b) { self.status = format!("Could not create print file: {e}"); return; }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use std::process::Command;
            let ok = Command::new("powershell").creation_flags(0x08000000)
                .args(["-NoProfile", "-Command", &format!("Start-Process -FilePath '{}' -Verb Print", p.display())])
                .status().map(|s| s.success()).unwrap_or(false);
            if ok { self.status = "Sent to your PDF app's print handler.".into(); }
            else {
                let _ = Command::new("cmd").creation_flags(0x08000000).args(["/C", "start", "", &p.display().to_string()]).status();
                self.status = "Opened the PDF - press Print in your PDF viewer.".into();
            }
        }
        #[cfg(not(windows))]
        { self.status = format!("Print file saved to {}", p.display()); }
    }
    fn side(&mut self, ui: &mut egui::Ui) {
        if btn(ui, "Open image...", true) {
            if let Some(p) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp", "gif"]).pick_file() { self.open_path(&p); }
        }
        ui.separator();
        if self.tab == Tab::Edit {
            ui.label(egui::RichText::new("CROP & CLEAN").small().weak());
            ui.horizontal(|ui| { ui.selectable_value(&mut self.tool, Tool::Crop, "Crop"); ui.selectable_value(&mut self.tool, Tool::Wand, "Remove colour"); });
            ui.add(egui::Slider::new(&mut self.tol, 2..=120).text("Tolerance"));
            if btn(ui, "Apply crop", false) { self.apply_crop(); }
            if btn(ui, "Auto-remove background", false) { self.auto_bg(); }
            if btn(ui, "Undo", false) { if let Some(h) = self.hist.pop() { self.img = Some(h); self.dirty = true; self.crop = None; } }
            ui.separator();
            if btn(ui, "Continue to slicer", true) { self.tab = Tab::Slice; }
        } else {
            ui.label(egui::RichText::new("PAPER").small().weak());
            egui::ComboBox::from_label("Size").selected_text(PAPERS[self.paper].0).show_ui(ui, |ui| {
                for (i, p) in PAPERS.iter().enumerate() { ui.selectable_value(&mut self.paper, i, format!("{} ({} x {} in)", p.0, p.1, p.2)); }
            });
            ui.checkbox(&mut self.land, "Landscape");
            num(ui, "Margin", &mut self.margin, 0.0..=1.5);
            num(ui, "Overlap", &mut self.overlap, 0.0..=1.0);
            ui.label(egui::RichText::new("POSTER SIZE").small().weak());
            ui.horizontal(|ui| { ui.selectable_value(&mut self.grid, true, "Grid (sheets)"); ui.selectable_value(&mut self.grid, false, "Exact size"); });
            if self.grid {
                ui.horizontal(|ui| { ui.label("Columns"); ui.add(egui::DragValue::new(&mut self.cols).clamp_range(1..=20)); ui.label("Rows"); ui.add(egui::DragValue::new(&mut self.rows).clamp_range(1..=20)); });
            } else {
                num(ui, "Width", &mut self.wid, 1.0..=200.0); num(ui, "Height", &mut self.hei, 1.0..=200.0);
            }
            ui.label(egui::RichText::new("OUTPUT").small().weak());
            ui.checkbox(&mut self.cut, "Cut lines"); ui.checkbox(&mut self.lab, "Sheet labels");
            egui::ComboBox::from_label("Quality").selected_text(format!("{} DPI", self.dpi)).show_ui(ui, |ui| {
                for d in [150u32, 200, 300] { ui.selectable_value(&mut self.dpi, d, format!("{d} DPI")); }
            });
            ui.separator();
            if btn(ui, "Export PDF...", true) {
                if let Some(p) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name("poster.pdf").save_file() {
                    self.status = match self.make_pdf() {
                        Some(b) => match std::fs::write(&p, b) { Ok(_) => format!("Saved {}", p.display()), Err(e) => format!("Save failed: {e}") },
                        None => "Nothing to export.".into(),
                    };
                }
            }
            if btn(ui, "Print...", false) { self.print(); }
            if btn(ui, "Back to editor", false) { self.tab = Tab::Edit; }
        }
    }
    fn center(&mut self, ui: &mut egui::Ui) {
        let Some(tex) = self.tex.clone() else {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.28);
                ui.image((self.hero.id(), vec2(150.0, 150.0)));
                ui.add_space(10.0);
                ui.label(egui::RichText::new("Welcome to PosterCraft").size(26.0).strong());
                ui.label("Open an image, or drop one here, to turn it into a multi-sheet poster.");
            });
            return;
        };
        let (iw, ih) = self.img.as_ref().map(|i| (i.width() as f32, i.height() as f32)).unwrap();
        let avail = ui.available_size();
        if self.tab == Tab::Edit {
            let s = ((avail.x - 8.0) / iw).min((avail.y - 8.0) / ih).max(0.01);
            ui.vertical_centered(|ui| {
                let (rect, resp) = ui.allocate_exact_size(vec2(iw * s, ih * s), Sense::click_and_drag());
                checker(ui, rect);
                ui.painter().image(tex.id(), rect, UV, Color32::WHITE);
                let to = |p: Pos2| pos2(((p.x - rect.min.x) / s).clamp(0.0, iw), ((p.y - rect.min.y) / s).clamp(0.0, ih));
                if let Some(p) = resp.interact_pointer_pos() {
                    match self.tool {
                        Tool::Crop => {
                            if resp.drag_started() { self.crop = Some((to(p), to(p))); }
                            else if resp.dragged() { if let Some(c) = &mut self.crop { c.1 = to(p); } }
                        }
                        Tool::Wand => { if resp.clicked() { let q = to(p); self.wand(q.x as u32, q.y as u32); } }
                    }
                }
                if let Some((a, b)) = self.crop {
                    let r = Rect::from_two_pos(pos2(rect.min.x + a.x * s, rect.min.y + a.y * s), pos2(rect.min.x + b.x * s, rect.min.y + b.y * s));
                    ui.painter().rect_filled(r, 0.0, ACCENT.gamma_multiply(0.18));
                    ui.painter().rect_stroke(r, 0.0, Stroke::new(2.0, ACCENT));
                }
            });
        } else if let Some(l) = self.layout() {
            let s = ((avail.x - 8.0) / l.bw).min((avail.y - 8.0) / l.bh).max(0.01);
            self.status = format!("{} x {} = {} sheets  |  poster {:.1} x {:.1} in", l.cols, l.rows, l.cols * l.rows, l.bw, l.bh);
            ui.vertical_centered(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(l.bw * s, l.bh * s), Sense::hover());
                let p = ui.painter().with_clip_rect(rect);
                p.rect_filled(rect, 0.0, Color32::WHITE);
                p.image(tex.id(), Rect::from_min_size(rect.min + vec2(l.ix, l.iy) * s, vec2(l.iw, l.ih) * s), UV, Color32::WHITE);
                for r in 0..l.rows {
                    for c in 0..l.cols {
                        let tl = rect.min + vec2(c as f32 * (l.pw - l.o) + l.m, r as f32 * (l.ph - l.o) + l.m) * s;
                        p.rect_stroke(Rect::from_min_size(tl, vec2(l.pw, l.ph) * s), 0.0, Stroke::new(1.0, RED));
                        p.text(tl + vec2(4.0, 4.0), Align2::LEFT_TOP, format!("{}", r * l.cols + c + 1), FontId::proportional(14.0), RED);
                    }
                }
            });
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _f: &mut eframe::Frame) {
        if let Some(p) = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone())) { self.open_path(&p); }
        if self.img.is_none() { self.tab = Tab::Edit; }
        if self.dirty { self.refresh(ctx); }
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.image((self.logo.id(), vec2(34.0, 34.0)));
                ui.label(egui::RichText::new("PosterCraft").size(24.0).strong());
                ui.add_space(28.0);
                ui.selectable_value(&mut self.tab, Tab::Edit, egui::RichText::new("1  Edit").size(16.0));
                if self.img.is_some() { ui.selectable_value(&mut self.tab, Tab::Slice, egui::RichText::new("2  Slice").size(16.0)); }
            });
            ui.add_space(6.0);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| { ui.label(egui::RichText::new(&self.status).weak()); });
        egui::SidePanel::left("side").exact_width(290.0).show(ctx, |ui| { ui.add_space(8.0); self.side(ui); });
        egui::CentralPanel::default().show(ctx, |ui| self.center(ui));
    }
}

fn main() -> eframe::Result {
    let ic = image::load_from_memory(include_bytes!("../assets/icon256.png")).unwrap().to_rgba8();
    let icon = egui::IconData { width: ic.width(), height: ic.height(), rgba: ic.into_raw() };
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("PosterCraft").with_inner_size([1280.0, 820.0]).with_min_inner_size([900.0, 600.0]).with_icon(Arc::new(icon)),
        ..Default::default()
    };
    eframe::run_native("PosterCraft", opts, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
