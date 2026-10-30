use macroquad::prelude::*;

// ---------------------------------------------------------------------------
// Constantes de física y "game feel"
// ---------------------------------------------------------------------------
const PHYSICS_DT: f32 = 1.0 / 120.0; // paso fijo: física determinista

const MAX_SPEED: f32 = 260.0;
const GROUND_ACCEL: f32 = 4000.0;
const GROUND_FRICTION: f32 = 3500.0;
const AIR_ACCEL: f32 = 1500.0;
const AIR_FRICTION: f32 = 500.0;

const GRAVITY: f32 = 1500.0;
const FALL_MULT: f32 = 1.4; // cae más rápido de lo que sube
const LOW_JUMP_MULT: f32 = 2.4; // soltar el botón = salto más corto
const JUMP_VELOCITY: f32 = -670.0; // altura máx. ≈ v²/(2·GRAVITY) ≈ 150 px
const MAX_FALL_SPEED: f32 = 900.0;

const COYOTE_TIME: f32 = 0.10; // puedes saltar un instante tras dejar una plataforma
const JUMP_BUFFER_TIME: f32 = 0.12; // el salto se "recuerda" si lo pulsas justo antes de aterrizar

const KILL_Y: f32 = 1200.0;

// ---------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------
fn approach(value: f32, target: f32, delta: f32) -> f32 {
    if value < target {
        (value + delta).min(target)
    } else {
        (value - delta).max(target)
    }
}

/// AABB estricto: dos rectángulos que solo se tocan NO colisionan.
/// (`Rect::overlaps` de macroquad usa <= y >=, y trataría el suelo bajo tus pies
/// como un choque lateral.)
fn intersects(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y
}

// ---------------------------------------------------------------------------
// Entrada
// ---------------------------------------------------------------------------
struct Input {
    move_x: f32,
    jump_held: bool,
}

impl Input {
    fn read() -> Self {
        let mut move_x = 0.0;
        if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
            move_x -= 1.0;
        }
        if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
            move_x += 1.0;
        }
        let jump_held = is_key_down(KeyCode::Space)
            || is_key_down(KeyCode::W)
            || is_key_down(KeyCode::Up);
        Self { move_x, jump_held }
    }

    fn jump_pressed() -> bool {
        is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up)
    }
}

// ---------------------------------------------------------------------------
// Jugador
// ---------------------------------------------------------------------------
struct Player {
    pos: Vec2,
    vel: Vec2,
    size: Vec2,
    grounded: bool,
    facing: f32,
    coyote: f32,
    jump_buffer: f32,
}

impl Player {
    fn new(pos: Vec2) -> Self {
        Self {
            pos,
            vel: Vec2::ZERO,
            size: vec2(32.0, 48.0),
            grounded: false,
            facing: 1.0,
            coyote: 0.0,
            jump_buffer: 0.0,
        }
    }

    fn rect(&self) -> Rect {
        Rect::new(self.pos.x, self.pos.y, self.size.x, self.size.y)
    }

    fn respawn(&mut self, at: Vec2) {
        *self = Player::new(at);
    }

    /// Un paso de física de duración fija `dt`.
    fn step(&mut self, input: &Input, solids: &[Rect], dt: f32) {
        // --- Horizontal: aceleración y fricción en vez de velocidad instantánea
        let target = input.move_x * MAX_SPEED;
        let rate = match (input.move_x != 0.0, self.grounded) {
            (true, true) => GROUND_ACCEL,
            (true, false) => AIR_ACCEL,
            (false, true) => GROUND_FRICTION,
            (false, false) => AIR_FRICTION,
        };
        self.vel.x = approach(self.vel.x, target, rate * dt);
        if input.move_x != 0.0 {
            self.facing = input.move_x.signum();
        }

        // --- Temporizadores de coyote time y jump buffer
        if self.grounded {
            self.coyote = COYOTE_TIME;
        } else {
            self.coyote = (self.coyote - dt).max(0.0);
        }
        self.jump_buffer = (self.jump_buffer - dt).max(0.0);

        // --- Salto
        if self.jump_buffer > 0.0 && self.coyote > 0.0 {
            self.vel.y = JUMP_VELOCITY;
            self.grounded = false;
            self.coyote = 0.0;
            self.jump_buffer = 0.0;
        }

        // --- Gravedad variable (salto de altura variable + caída más pesada)
        let gravity = if self.vel.y > 0.0 {
            GRAVITY * FALL_MULT
        } else if !input.jump_held {
            GRAVITY * LOW_JUMP_MULT
        } else {
            GRAVITY
        };
        self.vel.y = (self.vel.y + gravity * dt).min(MAX_FALL_SPEED);

        // --- Eje X
        self.pos.x += self.vel.x * dt;
        for solid in solids {
            if intersects(&self.rect(), solid) {
                if self.vel.x > 0.0 {
                    self.pos.x = solid.x - self.size.x;
                } else if self.vel.x < 0.0 {
                    self.pos.x = solid.x + solid.w;
                }
                self.vel.x = 0.0;
            }
        }

        // --- Eje Y
        self.pos.y += self.vel.y * dt;
        self.grounded = false;
        for solid in solids {
            if intersects(&self.rect(), solid) {
                if self.vel.y > 0.0 {
                    self.pos.y = solid.y - self.size.y;
                    self.grounded = true;
                } else if self.vel.y < 0.0 {
                    self.pos.y = solid.y + solid.h;
                }
                self.vel.y = 0.0;
            }
        }
    }

    fn draw(&self) {
        draw_rectangle(self.pos.x, self.pos.y, self.size.x, self.size.y, RED);
        // Un "ojo" para ver hacia dónde mira
        let eye_x = if self.facing > 0.0 {
            self.pos.x + self.size.x - 12.0
        } else {
            self.pos.x + 4.0
        };
        draw_rectangle(eye_x, self.pos.y + 10.0, 8.0, 8.0, WHITE);
    }
}

// ---------------------------------------------------------------------------
// Nivel
// ---------------------------------------------------------------------------
struct Level {
    solids: Vec<Rect>,
    coins: Vec<Vec2>,
    spawn: Vec2,
}

impl Level {
    fn new() -> Self {
        let solids = vec![
            // suelo con un hueco
            Rect::new(-200.0, 500.0, 1000.0, 200.0),
            Rect::new(950.0, 500.0, 1200.0, 200.0),
            // paredes en los extremos
            Rect::new(-232.0, -400.0, 32.0, 1100.0),
            Rect::new(2150.0, -400.0, 32.0, 1100.0),
            // plataformas
            Rect::new(300.0, 380.0, 200.0, 20.0),
            Rect::new(150.0, 260.0, 150.0, 20.0),
            Rect::new(600.0, 300.0, 100.0, 20.0),
            Rect::new(800.0, 380.0, 100.0, 20.0),
            Rect::new(1100.0, 400.0, 120.0, 20.0),
            Rect::new(1300.0, 300.0, 120.0, 20.0),
            Rect::new(1500.0, 200.0, 120.0, 20.0),
            Rect::new(1750.0, 330.0, 250.0, 20.0),
        ];
        let coins = vec![
            vec2(390.0, 340.0),
            vec2(210.0, 220.0),
            vec2(640.0, 260.0),
            vec2(840.0, 340.0),
            vec2(860.0, 520.0), // sobre el hueco... ¡cuidado!
            vec2(1150.0, 360.0),
            vec2(1350.0, 260.0),
            vec2(1550.0, 160.0),
            vec2(1850.0, 290.0),
        ];
        Self {
            solids,
            coins,
            spawn: vec2(100.0, 400.0),
        }
    }
}

const COIN_SIZE: f32 = 16.0;

fn coin_rect(c: Vec2) -> Rect {
    Rect::new(c.x - COIN_SIZE / 2.0, c.y - COIN_SIZE / 2.0, COIN_SIZE, COIN_SIZE)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
fn window_conf() -> Conf {
    Conf {
        window_title: "Plataformero AABB - Rust".to_owned(),
        window_width: 960,
        window_height: 540,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut level = Level::new();
    let total_coins = level.coins.len();
    let mut collected = 0usize;
    let mut player = Player::new(level.spawn);

    let mut camera_pos = player.pos;

    loop {
        let frame_dt = get_frame_time().min(0.1);

        // --- Entradas "de evento": se leen por frame, no por paso de física
        if Input::jump_pressed() {
            player.jump_buffer = JUMP_BUFFER_TIME;
        }
        if is_key_pressed(KeyCode::R) {
            level = Level::new();
            collected = 0;
            player.respawn(level.spawn);
        }
        let input = Input::read();

        // --- Física con paso fijo
        // Se reparte el frame en subpasos iguales (<= PHYSICS_DT): sin tirones
        let steps = (frame_dt / PHYSICS_DT).ceil().max(1.0) as u32;
        let sub_dt = frame_dt / steps as f32;
        for _ in 0..steps {
            player.step(&input, &level.solids, sub_dt);
        }

        // --- Monedas
        let prect = player.rect();
        let before = level.coins.len();
        level.coins.retain(|&c| !prect.overlaps(&coin_rect(c)));
        collected += before - level.coins.len();

        // --- Caída al vacío
        if player.pos.y > KILL_Y {
            player.respawn(level.spawn);
        }

        // --- Cámara con suavizado (independiente del framerate)
        let target = player.pos + player.size / 2.0;
        let t = 1.0 - (-10.0 * frame_dt).exp();
        camera_pos += (target - camera_pos) * t;
        let camera_target = camera_pos.round(); // alineada a píxeles: sin temblor

        // --- Render del mundo
        clear_background(Color::from_rgba(40, 44, 60, 255));

        set_camera(&Camera2D {
            target: camera_target,
            zoom: vec2(2.0 / screen_width(), 2.0 / screen_height()),
            ..Default::default()
        });

        for solid in &level.solids {
            draw_rectangle(solid.x, solid.y, solid.w, solid.h, GRAY);
            // borde superior para dar relieve
            draw_rectangle(solid.x, solid.y, solid.w, 4.0, LIGHTGRAY);
        }
        for &c in &level.coins {
            let r = coin_rect(c);
            draw_rectangle(r.x, r.y, r.w, r.h, GOLD);
        }
        player.draw();

        // --- HUD (cámara por defecto)
        set_default_camera();
        draw_text(
            &format!("Monedas: {}/{}", collected, total_coins),
            16.0,
            28.0,
            28.0,
            WHITE,
        );
        draw_text(
            "A/D o flechas: mover | Espacio/W: saltar (mantén para más altura) | R: reiniciar",
            16.0,
            screen_height() - 16.0,
            20.0,
            LIGHTGRAY,
        );
        draw_text(&format!("FPS: {}", get_fps()), screen_width() - 100.0, 28.0, 24.0, WHITE);

        if collected == total_coins {
            let msg = "Todas las monedas! (R para reiniciar)";
            let dims = measure_text(msg, None, 36, 1.0);
            draw_text(msg, (screen_width() - dims.width) / 2.0, 80.0, 36.0, GOLD);
        }

        next_frame().await;
    }
}