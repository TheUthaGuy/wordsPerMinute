//! Procedurally drawn app icon (rounded square with rising bars).

use eframe::egui::IconData;

pub fn rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let radius = s * 0.22;
    let mut out = vec![0u8; (size * size * 4) as usize];
    let bars = [(0.22, 0.30), (0.43, 0.46), (0.64, 0.62)]; // (x, height)
    let bar_w = 0.14;
    let base = 0.78;

    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Signed distance to the rounded square, for soft edges.
            let dx = (fx - s / 2.0).abs() - (s / 2.0 - radius);
            let dy = (fy - s / 2.0).abs() - (s / 2.0 - radius);
            let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() - radius;
            let coverage = (0.5 - outside).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            // Vertical gradient from blue to indigo.
            let t = fy / s;
            let mut col = [
                (59.0 + (99.0 - 59.0) * t) as u8,
                (130.0 + (102.0 - 130.0) * t) as u8,
                (246.0 + (241.0 - 246.0) * t) as u8,
            ];
            let (nx, ny) = (fx / s, fy / s);
            if bars
                .iter()
                .any(|&(bx, h)| nx >= bx && nx < bx + bar_w && ny <= base && ny >= base - h)
            {
                col = [255, 255, 255];
            }
            let i = ((y * size + x) * 4) as usize;
            out[i..i + 3].copy_from_slice(&col);
            out[i + 3] = (coverage * 255.0) as u8;
        }
    }
    out
}

pub fn app_icon(size: u32) -> IconData {
    IconData { rgba: rgba(size), width: size, height: size }
}
