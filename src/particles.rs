use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
use rand::RngExt;

const NUM_PARTICLES: usize = 100;
const CONNECT_DISTANCE: f32 = 70.0;
const MOUSE_RADIUS: f32 = 90.0;
const MOUSE_PUSH: f32 = 18.0;

struct Particle {
    pos: Pos2,
    vel: Vec2,
    phase: f32,
}

pub struct ParticleBackground {
    particles: Vec<Particle>,
    initialized: bool,
}

impl Default for ParticleBackground {
    fn default() -> Self {
        Self {
            particles: Vec::with_capacity(NUM_PARTICLES),
            initialized: false,
        }
    }
}

impl ParticleBackground {
    fn init_particles(&mut self, rect: egui::Rect) {
        let mut rng = rand::rng();
        self.particles.clear();

        for _ in 0..NUM_PARTICLES {
            self.particles.push(Particle {
                pos: Pos2::new(
                    rng.random_range(rect.left()..rect.right()),
                    rng.random_range(rect.top()..rect.bottom()),
                ),
                vel: Vec2::new(rng.random_range(-0.15..0.15), rng.random_range(-0.15..0.15)),
                phase: rng.random_range(0.0..1000.0),
            });
        }
        self.initialized = true;
    }

    pub fn update_and_draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx();
        let rect = ctx.viewport_rect(); // Берем размеры всего доступного экрана
        let width = rect.width();
        let height = rect.height();

        // Если размеры изменились или это первый запуск — инициализируем частицы на весь экран
        if !self.initialized || width < 10.0 {
            self.init_particles(rect);
            return;
        }

        // Получаем художника для фонового слоя (чтобы интерфейс был поверх)
        let painter = ctx.layer_painter(egui::LayerId::background());
        
        // Заливаем фон черным цветом
        painter.rect_filled(rect, 0.0, Color32::BLACK);

        let mouse = ctx.pointer_hover_pos();

        // 1. Обновление позиций физических частиц
        for p in &mut self.particles {
            p.phase += 0.008;
            let drift = Vec2::new((p.phase * 1.27).sin(), (p.phase * 1.61).cos()) * 0.004;

            p.vel += drift;
            p.vel *= 0.992;

            let max_speed = 0.3;
            if p.vel.length_sq() > max_speed * max_speed {
                p.vel = p.vel.normalized() * max_speed;
            }

            p.pos += p.vel;

            // Телепортация по краям экрана (Зацикливание)
            if p.pos.x < rect.left() {
                p.pos.x += width;
            } else if p.pos.x > rect.right() {
                p.pos.x -= width;
            }

            if p.pos.y < rect.top() {
                p.pos.y += height;
            } else if p.pos.y > rect.bottom() {
                p.pos.y -= height;
            }
        }

        // 2. Рассчет визуальных координат с учетом мыши
        let mut draw_positions = Vec::with_capacity(self.particles.len());
        for p in &self.particles {
            let mut pos = p.pos;

            if let Some(mouse_pos) = mouse {
                let dir = pos - mouse_pos;
                let dist = dir.length();

                if dist > 0.001 && dist < MOUSE_RADIUS {
                    let t = 1.0 - dist / MOUSE_RADIUS;
                    let force = t * t * (3.0 - 2.0 * t);
                    let push = (force * MOUSE_PUSH).min(dist * 0.8);
                    pos += dir.normalized() * push;
                }
            }
            draw_positions.push(pos);
        }

        // 3. Отрисовка линий соединения
        for i in 0..draw_positions.len() {
            for j in (i + 1)..draw_positions.len() {
                let mut dx = draw_positions[j].x - draw_positions[i].x;
                let mut dy = draw_positions[j].y - draw_positions[i].y;

                if dx > width * 0.5 { dx -= width; }
                else if dx < -width * 0.5 { dx += width; }

                if dy > height * 0.5 { dy -= height; }
                else if dy < -height * 0.5 { dy += height; }

                let dist = (dx * dx + dy * dy).sqrt();

                if dist < CONNECT_DISTANCE {
                    let alpha = ((1.0 - dist / CONNECT_DISTANCE) * 110.0) as u8;
                    let start = draw_positions[i];
                    let end = Pos2::new(start.x + dx, start.y + dy);

                    painter.line_segment(
                        [start, end],
                        Stroke::new(1.0 as f32, Color32::from_rgba_premultiplied(170, 170, 170, alpha)),
                    );
                }
            }
        }

        // 4. Отрисовка самих точек
        for pos in draw_positions {
            painter.circle_filled(pos, 2.2, Color32::from_gray(190));
        }

        // Запрашиваем перерисовку на следующем кадре для плавной анимации
        ctx.request_repaint();
    }
}