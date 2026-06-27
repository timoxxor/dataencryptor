use eframe::egui;
use image::AnimationDecoder;
use std::fs::File;
use std::io::BufReader;
use std::time::{Duration, Instant};

pub struct GifPlayer {
    frames: Vec<(egui::TextureHandle, Duration)>,
    current_frame: usize,
    last_update: Instant,
    pub is_playing: bool,
}

impl GifPlayer {
    pub fn new(ctx: &egui::Context, gif_path: &str) -> Self {
        let file = File::open(gif_path).expect("Failed to open GIF file");
        let reader = BufReader::new(file);
        let decoder = image::codecs::gif::GifDecoder::new(reader).expect("Ошибка декодирования");
        let decoded_frames = decoder
            .into_frames()
            .collect_frames()
            .expect("Ошибка чтения кадров");

        let mut frames = Vec::new();
        for (i, frame) in decoded_frames.into_iter().enumerate() {
            let delay = Duration::from(frame.delay());
            let buffer = frame.into_buffer();
            let width = buffer.width() as usize;
            let height = buffer.height() as usize;

            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [width, height],
                buffer.as_flat_samples().samples,
            );

            // Загружаем текстуры заранее, чтобы не делать этого во время рендера
            let texture = ctx.load_texture(
                format!("gif_frame_{}", i),
                color_image,
                Default::default(),
            );

            frames.push((texture, delay));
        }

        Self {
            frames,
            current_frame: 0,
            last_update: Instant::now(),
            is_playing: true,
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        if self.frames.is_empty() {
            ui.label("Пустой или поврежденный GIF...");
            return;
        }

        let ctx = ui.clone();

        // Логика переключения кадров
        if self.is_playing {
            let now = Instant::now();
            let elapsed = now.duration_since(self.last_update);
            let current_delay = self.frames[self.current_frame].1;

            if elapsed >= current_delay {
                if self.current_frame < self.frames.len() - 1 {
                    self.current_frame += 1;
                    self.last_update = now;
                    ctx.request_repaint_after(self.frames[self.current_frame].1);
                } else {
                    // Анимация завершилась (проигралась 1 раз)
                    self.is_playing = false;
                }
            } else {
                ctx.request_repaint_after(current_delay - elapsed);
            }
        }

        // Рендеринг текущего кадра
        let (texture, _) = &self.frames[self.current_frame];
        ui.image(texture);
    }
}