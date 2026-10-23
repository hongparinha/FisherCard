mod deck;
mod home;
mod modal;
mod shell;
mod study;
mod widgets;

pub use shell::show;

pub fn app_icon() -> eframe::egui::IconData {
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let i = ((y * size + x) * 4) as usize;
            let bg = cover(sd_round(px, py, 2.0, 2.0, 60.0, 60.0, 14.0));
            blend(&mut rgba[i..i + 4], 0, 122, 255, bg);
            let card = cover(sd_round(px, py, 16.0, 14.0, 32.0, 36.0, 5.0));
            blend(&mut rgba[i..i + 4], 255, 255, 255, card);
            let line = cover(sd_round(px, py, 22.0, 24.0, 20.0, 3.0, 1.5));
            blend(&mut rgba[i..i + 4], 0, 122, 255, line * 0.9);
            let line2 = cover(sd_round(px, py, 22.0, 32.0, 14.0, 3.0, 1.5));
            blend(&mut rgba[i..i + 4], 0, 122, 255, line2 * 0.55);
        }
    }
    eframe::egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

fn sd_round(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let cx = px - (x + w * 0.5);
    let cy = py - (y + h * 0.5);
    let qx = cx.abs() - (w * 0.5 - r);
    let qy = cy.abs() - (h * 0.5 - r);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.min(0.0).max(qy.min(0.0)) - r
}

fn cover(sd: f32) -> f32 {
    (0.5 - sd).clamp(0.0, 1.0)
}

fn blend(pixel: &mut [u8], red: u8, green: u8, blue: u8, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let src_a = alpha.clamp(0.0, 1.0);
    let dst_a = pixel[3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        return;
    }
    for (channel, src) in [red, green, blue].into_iter().enumerate() {
        let dst = pixel[channel] as f32 / 255.0;
        let out = (src as f32 / 255.0 * src_a + dst * dst_a * (1.0 - src_a)) / out_a;
        pixel[channel] = (out * 255.0).round() as u8;
    }
    pixel[3] = (out_a * 255.0).round() as u8;
}
