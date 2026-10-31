use macroquad::prelude::*;

// ---------------------------------------------------------------------------
// Constantes de física y "game feel"
// ---------------------------------------------------------------------------
const PHYSICS_DT: f32 = 1.0 / 120.0; // máximo tamaño de subpaso de física

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

const KILL_Y: f32 = 900.0; // caer por debajo de esto = muerte
const COIN_SIZE: f32 = 16.0;
const BLOCK_SIZE: f32 = 32.0;
const BUMP_TIME: f32 = 0.2;

// Tamaños del jugador: empieza pequeño y el hongo duplica su altura
const PLAYER_W: f32 = 32.0;
const SMALL_H: f32 = 24.0;
const BIG_H: f32 = 48.0;
const GROW_TIME: f32 = 0.9; // duración de la animación de crecer (el juego se congela)
const SHRINK_TIME: f32 = 0.3; // animación visual al encoger

// Bolas de fuego
const FIREBALL_SIZE: f32 = 12.0;
const FIRE_SPEED: f32 = 380.0;
const FIRE_GRAVITY: f32 = 1400.0;
const FIRE_BOUNCE: f32 = 380.0; // velocidad de rebote en el suelo
const FIRE_LIFE: f32 = 2.5; // segundos de vida máxima
const FIRE_COOLDOWN: f32 = 0.25;
const MAX_FIREBALLS: usize = 2; // como en Mario: máximo 2 a la vez

// Límites del mundo visibles para la cámara (evitan ver el vacío fuera del nivel)
const WORLD_LEFT: f32 = -232.0;
const WORLD_RIGHT: f32 = 3332.0;
const WORLD_TOP: f32 = -120.0;
const WORLD_BOTTOM: f32 = 700.0;

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

fn same_rect(a: &Rect, b: &Rect) -> bool {
    a.x == b.x && a.y == b.y && a.w == b.w && a.h == b.h
}

fn coin_rect(c: Vec2) -> Rect {
    Rect::new(c.x - COIN_SIZE / 2.0, c.y - COIN_SIZE / 2.0, COIN_SIZE, COIN_SIZE)
}

fn clamp_axis(v: f32, lo: f32, hi: f32, half: f32) -> f32 {
    let min = lo + half;
    let max = hi - half;
    if min > max {
        (lo + hi) / 2.0
    } else {
        v.clamp(min, max)
    }
}

fn fmt_time(t: f32) -> String {
    let secs = t as u32;
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

fn draw_centered_text(msg: &str, y: f32, size: u16, color: Color) {
    let dims = measure_text(msg, None, size, 1.0);
    draw_text(msg, (screen_width() - dims.width) / 2.0, y, size as f32, color);
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

    fn fire_pressed() -> bool {
        is_key_pressed(KeyCode::F) || is_key_pressed(KeyCode::X)
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
    big: bool,      // true = tamaño completo (el hongo duplica la altura)
    fire: bool,     // true = además tiene la flor de fuego (implica big)
    grow_t: f32,    // > 0 mientras dura la animación de crecer
    shrink_t: f32,  // animación visual al encoger
    invuln: f32,
    stun: f32,      // sin control horizontal tras un golpe
    launched: bool, // impulso externo (rebote): no se recorta al soltar el salto
    shoot_cd: f32,  // espera entre disparos
    bumped: Option<Rect>, // sólido contra el que se golpeó la cabeza en este frame
}

impl Player {
    fn new(pos: Vec2) -> Self {
        Self {
            pos,
            vel: Vec2::ZERO,
            size: vec2(PLAYER_W, SMALL_H), // empieza con la mitad de altura
            grounded: false,
            facing: 1.0,
            coyote: 0.0,
            jump_buffer: 0.0,
            big: false,
            fire: false,
            grow_t: 0.0,
            shrink_t: 0.0,
            invuln: 0.0,
            stun: 0.0,
            launched: false,
            shoot_cd: 0.0,
            bumped: None,
        }
    }

    fn rect(&self) -> Rect {
        Rect::new(self.pos.x, self.pos.y, self.size.x, self.size.y)
    }

    /// Caja de daño un poco más pequeña que el cuerpo (más justo con los pinchos).
    fn hurtbox(&self) -> Rect {
        Rect::new(self.pos.x + 5.0, self.pos.y + 6.0, self.size.x - 10.0, self.size.y - 8.0)
    }

    fn respawn(&mut self, at: Vec2) {
        *self = Player::new(at);
        self.invuln = 1.0; // un segundo de gracia al reaparecer
    }

    /// ¿Hay espacio libre encima para pasar a tamaño completo?
    fn can_grow(&self, solids: &[Rect]) -> bool {
        let grown = Rect::new(self.pos.x, self.pos.y - (BIG_H - SMALL_H), self.size.x, BIG_H);
        !solids.iter().any(|s| intersects(&grown, s))
    }

    /// Fin de la animación de crecer: la altura se duplica manteniendo los pies en su sitio.
    fn finish_grow(&mut self) {
        self.pos.y -= BIG_H - SMALL_H;
        self.size.y = BIG_H;
        self.big = true;
    }

    /// Vuelve al tamaño pequeño (y pierde el fuego) manteniendo los pies en su sitio.
    fn shrink(&mut self) {
        if self.big {
            self.pos.y += BIG_H - SMALL_H;
            self.size.y = SMALL_H;
            self.big = false;
            self.fire = false;
            self.shrink_t = SHRINK_TIME;
        }
    }

    /// Un subpaso de física de duración `dt`.
    fn step(&mut self, input: &Input, solids: &[Rect], dt: f32) {
        // --- Horizontal: aceleración y fricción
        // Aturdido (tras un golpe): sin control horizontal un instante
        let move_x = if self.stun > 0.0 { 0.0 } else { input.move_x };
        let target = move_x * MAX_SPEED;
        let rate = if self.stun > 0.0 {
            150.0
        } else {
            match (move_x != 0.0, self.grounded) {
                (true, true) => GROUND_ACCEL,
                (true, false) => AIR_ACCEL,
                (false, true) => GROUND_FRICTION,
                (false, false) => AIR_FRICTION,
            }
        };
        self.vel.x = approach(self.vel.x, target, rate * dt);
        if move_x != 0.0 {
            self.facing = move_x.signum();
        }

        // --- Coyote time y jump buffer
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

        // --- Gravedad variable
        if self.vel.y >= 0.0 {
            self.launched = false;
        }
        let gravity = if self.vel.y > 0.0 {
            GRAVITY * FALL_MULT
        } else if !input.jump_held && !self.launched {
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
                    self.bumped = Some(*solid); // cabezazo (para los bloques bonus)
                }
                self.vel.y = 0.0;
            }
        }
    }

    fn draw(&self) {
        // Parpadeo durante la invulnerabilidad
        let visible = self.invuln <= 0.0 || ((get_time() * 20.0) as i64) % 2 == 0;
        if !visible {
            return;
        }

        let feet = self.pos.y + self.size.y;
        let mut w = self.size.x;
        let mut h = self.size.y;
        if self.grow_t > 0.0 {
            // Animación de crecer: de SMALL_H a BIG_H (suavizado) con un pequeño "estirón" lateral
            let p = (1.0 - self.grow_t / GROW_TIME).clamp(0.0, 1.0);
            let eased = p * p * (3.0 - 2.0 * p);
            h = SMALL_H + (BIG_H - SMALL_H) * eased;
            w = self.size.x * (1.0 + 0.2 * (p * std::f32::consts::PI).sin());
        } else if self.shrink_t > 0.0 {
            // Animación de encoger: el sprite baja de BIG_H a SMALL_H
            let p = (self.shrink_t / SHRINK_TIME).clamp(0.0, 1.0);
            h = self.size.y + (BIG_H - SMALL_H) * p;
        }
        let x = self.pos.x + (self.size.x - w) / 2.0;
        let y = feet - h;

        if self.fire {
            // Traje de fuego: blanco con la parte de abajo roja
            draw_rectangle(x, y, w, h, Color::from_rgba(250, 245, 235, 255));
            draw_rectangle(x, y + h * 0.6, w, h * 0.4, RED);
        } else {
            draw_rectangle(x, y, w, h, RED);
        }

        // Un "ojo" para ver hacia dónde mira
        let eye = if h > 36.0 { 8.0 } else { 6.0 };
        let eye_x = if self.facing > 0.0 { x + w - eye - 4.0 } else { x + 4.0 };
        let eye_color = if self.fire { BLACK } else { WHITE };
        draw_rectangle(eye_x, y + h * 0.2, eye, eye, eye_color);
    }
}

// ---------------------------------------------------------------------------
// Bolas de fuego
// ---------------------------------------------------------------------------
/// Avanza en horizontal, cae con gravedad y rebota en suelo y techo.
/// Se apaga al chocar con una pared, al agotar su vida o al matar a un enemigo.
struct Fireball {
    pos: Vec2, // esquina superior izquierda
    vel: Vec2,
    life: f32,
    alive: bool,
}

impl Fireball {
    fn new(pos: Vec2, dir: f32) -> Self {
        Self {
            pos,
            vel: vec2(dir * FIRE_SPEED, 150.0),
            life: FIRE_LIFE,
            alive: true,
        }
    }

    fn rect(&self) -> Rect {
        Rect::new(self.pos.x, self.pos.y, FIREBALL_SIZE, FIREBALL_SIZE)
    }

    fn update(&mut self, dt: f32, solids: &[Rect]) {
        if !self.alive {
            return;
        }
        self.life -= dt;
        if self.life <= 0.0 {
            self.alive = false;
            return;
        }

        self.vel.y = (self.vel.y + FIRE_GRAVITY * dt).min(700.0);

        // Eje X: una pared la apaga
        self.pos.x += self.vel.x * dt;
        for s in solids {
            if intersects(&self.rect(), s) {
                self.alive = false;
                return;
            }
        }

        // Eje Y: rebota en el suelo y en el techo
        self.pos.y += self.vel.y * dt;
        for s in solids {
            if intersects(&self.rect(), s) {
                if self.vel.y > 0.0 {
                    self.pos.y = s.y - FIREBALL_SIZE;
                    self.vel.y = -FIRE_BOUNCE;
                } else if self.vel.y < 0.0 {
                    self.pos.y = s.y + s.h;
                    self.vel.y = 120.0;
                }
            }
        }
    }

    fn draw(&self) {
        let c = self.pos + vec2(FIREBALL_SIZE / 2.0, FIREBALL_SIZE / 2.0);
        draw_circle(c.x, c.y, 7.0, ORANGE);
        draw_circle(c.x, c.y, 4.0, YELLOW);
    }
}

// ---------------------------------------------------------------------------
// Elementos del nivel
// ---------------------------------------------------------------------------
struct Checkpoint {
    rect: Rect,
    respawn: Vec2,
}

#[derive(Clone, Copy, PartialEq)]
enum BonusKind {
    Coin,    // +50 puntos
    PowerUp, // hongo si eres pequeño; flor de fuego si ya eres grande (como en Mario)
}

/// Bloque "?" sólido: se activa golpeándolo desde abajo con la cabeza.
struct BonusBlock {
    rect: Rect,
    kind: BonusKind,
    used: bool,
    bump: f32, // animación de golpe
}

impl BonusBlock {
    fn new(x: f32, y: f32, kind: BonusKind) -> Self {
        Self {
            rect: Rect::new(x, y, BLOCK_SIZE, BLOCK_SIZE),
            kind,
            used: false,
            bump: 0.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum PowerKind {
    Mushroom, // camina y cae; hace crecer al jugador pequeño
    Flower,   // se queda quieta sobre el bloque; da el poder de fuego al jugador grande
}

/// Power-up que sale de un bloque "?".
struct PowerUp {
    kind: PowerKind,
    pos: Vec2, // esquina superior izquierda
    vel: Vec2,
    size: Vec2,
    dir: f32,
    emerge: f32, // 0.0 = dentro del bloque, 1.0 = ya fuera
    start_y: f32,
    end_y: f32,
    alive: bool,
}

impl PowerUp {
    const SPEED: f32 = 80.0;
    const EMERGE_TIME: f32 = 0.6;

    fn from_block(block: &Rect, dir: f32, kind: PowerKind) -> Self {
        let size = vec2(28.0, 28.0);
        let start_y = block.y + block.h - size.y; // escondido dentro del bloque
        let end_y = block.y - size.y; // apoyado encima
        Self {
            kind,
            pos: vec2(block.x + (block.w - size.x) / 2.0, start_y),
            vel: Vec2::ZERO,
            size,
            dir,
            emerge: 0.0,
            start_y,
            end_y,
            alive: true,
        }
    }

    fn rect(&self) -> Rect {
        Rect::new(self.pos.x, self.pos.y, self.size.x, self.size.y)
    }

    fn update(&mut self, dt: f32, solids: &[Rect]) {
        if !self.alive {
            return;
        }

        // Fase 1: sale del bloque
        if self.emerge < 1.0 {
            self.emerge = (self.emerge + dt / Self::EMERGE_TIME).min(1.0);
            self.pos.y = self.start_y + (self.end_y - self.start_y) * self.emerge;
            return;
        }

        // La flor se queda donde está
        if self.kind == PowerKind::Flower {
            return;
        }

        // Fase 2 (hongo): camina, cae y rebota en las paredes
        self.vel.x = self.dir * Self::SPEED;
        self.vel.y = (self.vel.y + GRAVITY * dt).min(MAX_FALL_SPEED);

        self.pos.x += self.vel.x * dt;
        for s in solids {
            if intersects(&self.rect(), s) {
                if self.vel.x > 0.0 {
                    self.pos.x = s.x - self.size.x;
                } else if self.vel.x < 0.0 {
                    self.pos.x = s.x + s.w;
                }
                self.dir = -self.dir;
                self.vel.x = 0.0;
            }
        }

        self.pos.y += self.vel.y * dt;
        for s in solids {
            if intersects(&self.rect(), s) {
                if self.vel.y > 0.0 {
                    self.pos.y = s.y - self.size.y;
                } else if self.vel.y < 0.0 {
                    self.pos.y = s.y + s.h;
                }
                self.vel.y = 0.0;
            }
        }
    }

    fn draw(&self) {
        match self.kind {
            PowerKind::Mushroom => self.draw_mushroom(),
            PowerKind::Flower => self.draw_flower(),
        }
    }

    fn draw_mushroom(&self) {
        let r = self.rect();
        let cx = r.x + r.w / 2.0;
        let cy = r.y + 15.0;
        let rad = 14.0;
        let cap = Color::from_rgba(225, 60, 60, 255);

        // Sombrero: semicírculo hecho con triángulos
        let n = 10;
        for i in 0..n {
            let a0 = std::f32::consts::PI * (1.0 + i as f32 / n as f32);
            let a1 = std::f32::consts::PI * (1.0 + (i + 1) as f32 / n as f32);
            draw_triangle(
                vec2(cx, cy),
                vec2(cx + rad * a0.cos(), cy + rad * a0.sin()),
                vec2(cx + rad * a1.cos(), cy + rad * a1.sin()),
                cap,
            );
        }
        draw_circle(cx - 5.0, cy - 6.0, 3.0, WHITE);
        draw_circle(cx + 6.0, cy - 8.0, 3.0, WHITE);

        // Tallo con ojitos
        draw_rectangle(cx - 8.0, cy, 16.0, 13.0, Color::from_rgba(250, 235, 200, 255));
        draw_rectangle(cx - 5.0, cy + 4.0, 3.0, 4.0, BLACK);
        draw_rectangle(cx + 2.0, cy + 4.0, 3.0, 4.0, BLACK);
    }

    fn draw_flower(&self) {
        let r = self.rect();
        let cx = r.x + r.w / 2.0;
        let top = r.y;
        let green = Color::from_rgba(60, 170, 70, 255);

        // Tallo y hojas
        draw_rectangle(cx - 2.0, top + 16.0, 4.0, 12.0, green);
        draw_triangle(vec2(cx, top + 24.0), vec2(cx - 11.0, top + 18.0), vec2(cx - 3.0, top + 28.0), green);
        draw_triangle(vec2(cx, top + 24.0), vec2(cx + 11.0, top + 18.0), vec2(cx + 3.0, top + 28.0), green);

        // Pétalos (cambian de color para llamar la atención) y centro
        let pulse = ((get_time() as f32) * 8.0).sin() * 0.5 + 0.5;
        let petal = Color::new(1.0, 0.45 + 0.35 * pulse, 0.1, 1.0);
        let hy = top + 10.0;
        for i in 0..6 {
            let a = i as f32 * std::f32::consts::PI / 3.0;
            draw_circle(cx + 7.0 * a.cos(), hy + 7.0 * a.sin(), 5.0, petal);
        }
        draw_circle(cx, hy, 5.5, YELLOW);
        draw_circle(cx - 1.5, hy - 1.0, 1.2, BLACK);
        draw_circle(cx + 1.5, hy - 1.0, 1.2, BLACK);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum EnemyKind {
    Walker, // patrulla por el suelo / una plataforma
    Flyer,  // patrulla en el aire con vaivén vertical
}

struct Enemy {
    kind: EnemyKind,
    pos: Vec2, // esquina superior izquierda
    size: Vec2,
    min_x: f32, // límites de patrulla (coordenada x izquierda del enemigo)
    max_x: f32,
    dir: f32,
    speed: f32,
    base_y: f32,
    phase: f32,
    alive: bool,
}

impl Enemy {
    fn walker(min_x: f32, max_x: f32, ground_y: f32) -> Self {
        let size = vec2(30.0, 26.0);
        Self {
            kind: EnemyKind::Walker,
            pos: vec2((min_x + max_x) / 2.0, ground_y - size.y),
            size,
            min_x,
            max_x,
            dir: 1.0,
            speed: 60.0,
            base_y: ground_y - size.y,
            phase: 0.0,
            alive: true,
        }
    }

    fn flyer(min_x: f32, max_x: f32, base_y: f32, phase: f32) -> Self {
        let size = vec2(28.0, 20.0);
        Self {
            kind: EnemyKind::Flyer,
            pos: vec2((min_x + max_x) / 2.0, base_y),
            size,
            min_x,
            max_x,
            dir: -1.0,
            speed: 80.0,
            base_y,
            phase,
            alive: true,
        }
    }

    fn rect(&self) -> Rect {
        Rect::new(self.pos.x, self.pos.y, self.size.x, self.size.y)
    }

    fn update(&mut self, dt: f32, t: f32) {
        if !self.alive {
            return;
        }
        self.pos.x += self.dir * self.speed * dt;
        if self.pos.x <= self.min_x {
            self.pos.x = self.min_x;
            self.dir = 1.0;
        } else if self.pos.x >= self.max_x {
            self.pos.x = self.max_x;
            self.dir = -1.0;
        }
        if self.kind == EnemyKind::Flyer {
            self.pos.y = self.base_y + (t * 3.0 + self.phase).sin() * 25.0;
        }
    }
}

struct Level {
    solids: Vec<Rect>,
    spikes: Vec<Rect>,
    enemies: Vec<Enemy>,
    bonus_blocks: Vec<BonusBlock>,
    coins: Vec<Vec2>,
    checkpoints: Vec<Checkpoint>,
    goal: Rect,
    spawn: Vec2,
}

impl Level {
    fn new() -> Self {
        // Suelo en y = 500. Cuatro tramos separados por pozos:
        //   A: -200..800   pozo 800..950
        //   B:  950..1500  pozo 1500..1700
        //   C: 1700..2300  pozo 2300..2450
        //   D: 2450..3300
        let solids = vec![
            // --- suelo
            Rect::new(-200.0, 500.0, 1000.0, 200.0),
            Rect::new(950.0, 500.0, 550.0, 200.0),
            Rect::new(1700.0, 500.0, 600.0, 200.0),
            Rect::new(2450.0, 500.0, 850.0, 200.0),
            // --- paredes de los extremos
            Rect::new(-232.0, -400.0, 32.0, 1100.0),
            Rect::new(3300.0, -400.0, 32.0, 1100.0),
            // --- sección A: plataformas iniciales
            Rect::new(300.0, 380.0, 200.0, 20.0),
            Rect::new(150.0, 260.0, 150.0, 20.0),
            Rect::new(600.0, 300.0, 100.0, 20.0),
            Rect::new(800.0, 380.0, 100.0, 20.0), // sobre el primer pozo
            // --- sección B
            Rect::new(1030.0, 400.0, 100.0, 20.0),
            Rect::new(1180.0, 320.0, 100.0, 20.0),
            Rect::new(1390.0, 440.0, 40.0, 60.0), // bloque bajo
            Rect::new(1560.0, 430.0, 80.0, 20.0), // escalón en el pozo ancho
            // --- sección C: subida opcional a la plataforma alta (cada salto ≈ 90 px)
            Rect::new(2110.0, 410.0, 90.0, 20.0),
            Rect::new(2290.0, 320.0, 90.0, 20.0),
            Rect::new(2090.0, 230.0, 150.0, 20.0),
            // --- sección D: muro alto (hay que mantener el salto)
            Rect::new(2880.0, 380.0, 40.0, 120.0),
        ];

        let spikes = vec![
            Rect::new(1260.0, 484.0, 90.0, 16.0),
            Rect::new(1810.0, 484.0, 70.0, 16.0),
            Rect::new(1990.0, 484.0, 70.0, 16.0),
            Rect::new(2600.0, 484.0, 70.0, 16.0),
            Rect::new(2760.0, 484.0, 70.0, 16.0),
        ];

        // Caminantes: x izquierda mín/máx de patrulla y altura del suelo.
        // Voladores: x mín/máx, altura base y fase del vaivén.
        let enemies = vec![
            Enemy::walker(380.0, 700.0, 500.0),       // A: suelo inicial
            Enemy::flyer(810.0, 920.0, 250.0, 0.0),   // sobre el primer pozo
            Enemy::walker(1010.0, 1220.0, 500.0),     // B: antes de los pinchos
            Enemy::flyer(1510.0, 1660.0, 320.0, 1.5), // sobre el pozo ancho
            Enemy::walker(2075.0, 2260.0, 500.0),     // C: al pie de la subida
            Enemy::flyer(2600.0, 2800.0, 270.0, 3.0), // D: sobre los pinchos
            Enemy::walker(3050.0, 3140.0, 500.0),     // D: guarda la bandera
        ];

        // Bloques "?": con el suelo en y=500, un bloque en y=380 queda a 40 px sobre
        // la cabeza del jugador grande (se puede pasar por debajo y golpearlo saltando).
        // Los bloques PowerUp sueltan un hongo si eres pequeño y una flor si ya eres grande.
        let bonus_blocks = vec![
            BonusBlock::new(160.0, 380.0, BonusKind::PowerUp), // cerca del inicio: primer hongo
            BonusBlock::new(240.0, 380.0, BonusKind::Coin),
            BonusBlock::new(540.0, 380.0, BonusKind::PowerUp), // si ya eres grande: flor de fuego
            BonusBlock::new(1450.0, 380.0, BonusKind::Coin),
            BonusBlock::new(2180.0, 140.0, BonusKind::Coin), // premio de la ruta alta
            BonusBlock::new(2470.0, 380.0, BonusKind::PowerUp), // por si te encogieron
            BonusBlock::new(3100.0, 380.0, BonusKind::Coin),
        ];

        let coins = vec![
            // A
            vec2(400.0, 340.0),
            vec2(225.0, 220.0),
            vec2(650.0, 260.0),
            vec2(850.0, 340.0),
            // B
            vec2(1080.0, 360.0),
            vec2(1230.0, 280.0),
            vec2(1305.0, 350.0), // sobre los pinchos: se toma en lo alto del salto
            vec2(1410.0, 400.0),
            vec2(1600.0, 390.0),
            // C
            vec2(1845.0, 350.0),
            vec2(2025.0, 350.0),
            vec2(2155.0, 370.0),
            vec2(2335.0, 280.0),
            vec2(2175.0, 190.0),
            // D
            vec2(2635.0, 350.0),
            vec2(2795.0, 350.0),
            vec2(2900.0, 340.0),
        ];

        let checkpoints = vec![
            Checkpoint { rect: Rect::new(985.0, 420.0, 12.0, 80.0), respawn: vec2(975.0, 476.0) },
            Checkpoint { rect: Rect::new(1740.0, 420.0, 12.0, 80.0), respawn: vec2(1735.0, 476.0) },
            Checkpoint { rect: Rect::new(2490.0, 420.0, 12.0, 80.0), respawn: vec2(2475.0, 476.0) },
        ];

        Self {
            solids,
            spikes,
            enemies,
            bonus_blocks,
            coins,
            checkpoints,
            goal: Rect::new(3180.0, 420.0, 40.0, 80.0),
            spawn: vec2(100.0, 476.0), // justo sobre el suelo (500 - SMALL_H)
        }
    }
}

// ---------------------------------------------------------------------------
// Estado del juego
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Title,
    Playing,
    Won,
}

/// Texto flotante ("+50", "FUEGO!") que sube y se desvanece.
struct Popup {
    pos: Vec2,
    t: f32,
    text: String,
}

struct Game {
    level: Level,
    player: Player,
    checkpoint: Vec2,
    collected: usize,
    total_coins: usize,
    bonus: usize,
    total_bonus: usize,
    kills: usize,
    total_enemies: usize,
    score: u32,
    deaths: u32,
    time: f32,
    phase: Phase,
    camera: Vec2,
    toast: String,
    toast_t: f32,
    popups: Vec<Popup>,
    powerups: Vec<PowerUp>,
    fireballs: Vec<Fireball>,
}

impl Game {
    fn with_phase(phase: Phase) -> Self {
        let level = Level::new();
        let player = Player::new(level.spawn);
        let total_coins = level.coins.len();
        // "Bonus" cuenta solo los bloques de monedas
        let total_bonus = level
            .bonus_blocks
            .iter()
            .filter(|b| b.kind == BonusKind::Coin)
            .count();
        let total_enemies = level.enemies.len();
        let checkpoint = level.spawn;
        let mut game = Self {
            level,
            player,
            checkpoint,
            collected: 0,
            total_coins,
            bonus: 0,
            total_bonus,
            kills: 0,
            total_enemies,
            score: 0,
            deaths: 0,
            time: 0.0,
            phase,
            camera: Vec2::ZERO,
            toast: String::new(),
            toast_t: 0.0,
            popups: Vec::new(),
            powerups: Vec::new(),
            fireballs: Vec::new(),
        };
        game.camera = game.camera_target(); // sin "viaje" de cámara al empezar
        game
    }

    fn say(&mut self, msg: &str) {
        self.toast = msg.to_string();
        self.toast_t = 2.5;
    }

    fn popup(&mut self, pos: Vec2, text: &str) {
        self.popups.push(Popup { pos, t: 0.0, text: text.to_string() });
    }

    /// El jugador grande pierde el tamaño (y el fuego): sale despedido (hacia atrás y
    /// arriba), sin control un instante y con invulnerabilidad para salir del peligro.
    fn knockback(&mut self) {
        self.player.shrink(); // vuelve al tamaño pequeño
        self.player.invuln = 1.5;
        self.player.vel = vec2(-self.player.facing * 320.0, -560.0);
        self.player.launched = true;
        self.player.stun = 0.3;
        self.player.grounded = false;
        self.player.coyote = 0.0;
    }

    fn die(&mut self) {
        self.deaths += 1;
        self.player.respawn(self.checkpoint);
        self.fireballs.clear();
        self.camera = self.camera_target();
    }

    /// Cámara limitada a los bordes del mundo: nunca se ve el vacío.
    fn clamp_camera(&self, c: Vec2) -> Vec2 {
        vec2(
            clamp_axis(c.x, WORLD_LEFT, WORLD_RIGHT, screen_width() / 2.0),
            clamp_axis(c.y, WORLD_TOP, WORLD_BOTTOM, screen_height() / 2.0),
        )
    }

    fn camera_target(&self) -> Vec2 {
        self.clamp_camera(self.player.pos + self.player.size / 2.0)
    }

    fn update(&mut self, frame_dt: f32) {
        self.toast_t = (self.toast_t - frame_dt).max(0.0);
        for p in &mut self.popups {
            p.t += frame_dt;
        }
        self.popups.retain(|p| p.t < 1.0);
        for b in &mut self.level.bonus_blocks {
            b.bump = (b.bump - frame_dt).max(0.0);
        }

        // ------------------------------------------------------------------
        // Pantalla de título: la cámara recorre el nivel mientras tanto
        // ------------------------------------------------------------------
        if self.phase == Phase::Title {
            self.time += frame_dt;
            for e in &mut self.level.enemies {
                e.update(frame_dt, self.time);
            }
            let s = (self.time * 0.12).sin() * 0.5 + 0.5;
            let c = vec2(300.0 + s * 2600.0, 380.0);
            self.camera = self.clamp_camera(c);

            if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
                *self = Game::with_phase(Phase::Playing);
            }
            return;
        }

        // ------------------------------------------------------------------
        // Partida
        // ------------------------------------------------------------------
        // Animación de crecer: el mundo se congela unos instantes (como en Mario)
        let mut frozen = false;
        if self.phase == Phase::Playing && self.player.grow_t > 0.0 {
            self.player.grow_t -= frame_dt;
            if self.player.grow_t <= 0.0 {
                self.player.grow_t = 0.0;
                self.player.finish_grow();
            }
            frozen = true;
        }

        if self.phase == Phase::Playing && !frozen {
            self.time += frame_dt;
            self.player.invuln = (self.player.invuln - frame_dt).max(0.0);
            self.player.stun = (self.player.stun - frame_dt).max(0.0);
            self.player.shrink_t = (self.player.shrink_t - frame_dt).max(0.0);
            self.player.shoot_cd = (self.player.shoot_cd - frame_dt).max(0.0);

            // --- Entradas de evento (por frame)
            if Input::jump_pressed() {
                self.player.jump_buffer = JUMP_BUFFER_TIME;
            }
            let input = Input::read();

            // --- Disparo (solo con la flor de fuego)
            if self.player.fire
                && self.player.shoot_cd <= 0.0
                && Input::fire_pressed()
                && self.fireballs.len() < MAX_FIREBALLS
            {
                let dir = self.player.facing;
                let x = if dir > 0.0 {
                    self.player.pos.x + self.player.size.x
                } else {
                    self.player.pos.x - FIREBALL_SIZE
                };
                let y = self.player.pos.y + self.player.size.y * 0.35;
                self.fireballs.push(Fireball::new(vec2(x, y), dir));
                self.player.shoot_cd = FIRE_COOLDOWN;
            }

            // --- Enemigos
            for e in &mut self.level.enemies {
                e.update(frame_dt, self.time);
            }

            // --- Sólidos de este frame (suelo + bloques bonus)
            let mut colliders = self.level.solids.clone();
            for b in &self.level.bonus_blocks {
                colliders.push(b.rect);
            }

            // --- Física: el frame se divide en subpasos iguales
            let steps = (frame_dt / PHYSICS_DT).ceil().max(1.0) as u32;
            let sub_dt = frame_dt / steps as f32;
            for _ in 0..steps {
                self.player.step(&input, &colliders, sub_dt);
                for p in &mut self.powerups {
                    p.update(sub_dt, &colliders);
                }
                for f in &mut self.fireballs {
                    f.update(sub_dt, &colliders);
                }
            }

            // --- Cabezazo a un bloque bonus
            if let Some(hit) = self.player.bumped.take() {
                let mut reward: Option<(BonusKind, Vec2, Rect)> = None;
                for b in &mut self.level.bonus_blocks {
                    if !b.used && same_rect(&b.rect, &hit) {
                        b.used = true;
                        b.bump = BUMP_TIME;
                        reward = Some((b.kind, vec2(b.rect.x + b.rect.w / 2.0, b.rect.y - 6.0), b.rect));
                    }
                }
                if let Some((kind, pos, brect)) = reward {
                    match kind {
                        BonusKind::Coin => {
                            self.bonus += 1;
                            self.score += 50;
                            self.popup(pos, "+50");
                        }
                        BonusKind::PowerUp => {
                            // Pequeño -> hongo. Grande -> flor de fuego.
                            let pk = if self.player.big { PowerKind::Flower } else { PowerKind::Mushroom };
                            // el hongo sale hacia el lado contrario al jugador
                            let player_cx = self.player.pos.x + self.player.size.x / 2.0;
                            let dir = if player_cx < brect.x + brect.w / 2.0 { 1.0 } else { -1.0 };
                            self.powerups.push(PowerUp::from_block(&brect, dir, pk));
                        }
                    }
                }
            }

            let prect = self.player.rect();

            // --- Monedas
            let before = self.level.coins.len();
            self.level.coins.retain(|&c| !intersects(&prect, &coin_rect(c)));
            let gained = before - self.level.coins.len();
            self.collected += gained;
            self.score += (10 * gained) as u32;

            // --- Power-ups
            // Si no hay espacio encima para crecer, el hongo espera a que lo cojas en un sitio libre
            let can_grow = self.player.big || self.player.can_grow(&colliders);
            let mut picked: Option<PowerKind> = None;
            for p in &mut self.powerups {
                if p.alive && p.emerge >= 1.0 && can_grow && intersects(&prect, &p.rect()) {
                    p.alive = false;
                    picked = Some(p.kind);
                }
            }
            self.powerups.retain(|p| p.alive && p.pos.y < KILL_Y);
            if let Some(kind) = picked {
                self.score += 200;
                let pp = vec2(self.player.pos.x + self.player.size.x / 2.0, self.player.pos.y - 8.0);
                if !self.player.big {
                    // hongo (o flor con tamaño pequeño): crece
                    self.player.grow_t = GROW_TIME;
                    self.popup(pp, "SUPER! +200");
                    self.say("Creces y aguantas un golpe");
                } else if kind == PowerKind::Flower && !self.player.fire {
                    self.player.fire = true;
                    self.popup(pp, "FUEGO! +200");
                    self.say("Flor de fuego: dispara con F o X");
                } else {
                    self.popup(pp, "+200");
                }
            }

            // --- Checkpoints
            let mut new_cp: Option<Vec2> = None;
            for cp in &self.level.checkpoints {
                if intersects(&prect, &cp.rect) && self.checkpoint != cp.respawn {
                    new_cp = Some(cp.respawn);
                }
            }
            if let Some(p) = new_cp {
                self.checkpoint = p;
                self.say("Checkpoint!");
            }

            // --- Bolas de fuego contra enemigos
            let mut fire_kills = 0usize;
            let mut fire_kill_pos = Vec2::ZERO;
            for f in &mut self.fireballs {
                if !f.alive {
                    continue;
                }
                let fr = f.rect();
                for e in &mut self.level.enemies {
                    if e.alive && intersects(&fr, &e.rect()) {
                        e.alive = false;
                        f.alive = false;
                        fire_kills += 1;
                        fire_kill_pos = vec2(e.pos.x + e.size.x / 2.0, e.pos.y - 6.0);
                        break;
                    }
                }
            }
            self.fireballs.retain(|f| f.alive && f.pos.y < KILL_Y);
            if fire_kills > 0 {
                self.kills += fire_kills;
                self.score += (100 * fire_kills) as u32;
                self.popup(fire_kill_pos, "+100");
            }

            // --- Enemigos: pisarlos los elimina, tocarlos de lado/desde abajo hace daño
            let hurt = self.player.hurtbox();
            let feet = self.player.pos.y + self.player.size.y;
            let prev_feet = feet - self.player.vel.y * frame_dt;
            let falling = self.player.vel.y > 0.0;
            let mut stomped = 0usize;
            let mut touched_enemy = false;
            let mut stomp_pos = Vec2::ZERO;
            for e in &mut self.level.enemies {
                if !e.alive {
                    continue;
                }
                let r = e.rect();
                let hitbox = Rect::new(r.x + 2.0, r.y + 2.0, r.w - 4.0, r.h - 2.0);
                if intersects(&hurt, &hitbox) {
                    if falling && prev_feet <= r.y + 10.0 {
                        e.alive = false;
                        stomped += 1;
                        stomp_pos = vec2(r.x + r.w / 2.0, r.y - 6.0);
                    } else {
                        touched_enemy = true;
                    }
                }
            }
            if stomped > 0 {
                self.kills += stomped;
                self.score += (100 * stomped) as u32;
                self.popup(stomp_pos, "+100");
                // rebote: más alto si mantienes el botón de salto
                self.player.vel.y = if input.jump_held { -560.0 } else { -420.0 };
                self.player.launched = true; // el rebote no se recorta al soltar el salto
                self.player.grounded = false;
                self.player.coyote = 0.0;
                touched_enemy = false;
            }

            // --- Peligros: pinchos y enemigos (ser grande absorbe un golpe)
            let on_spikes = self.level.spikes.iter().any(|s| {
                intersects(&hurt, &Rect::new(s.x + 3.0, s.y + 6.0, s.w - 6.0, s.h - 6.0))
            });
            let mut dead = false;
            if (on_spikes || touched_enemy) && self.player.invuln <= 0.0 {
                if self.player.big {
                    self.knockback();
                    self.say("Encoges: perdiste el super");
                } else {
                    dead = true;
                }
            }
            if self.player.pos.y > KILL_Y {
                dead = true; // el vacío mata aunque seas grande
            }
            if dead {
                self.die();
            }

            // --- Meta
            if intersects(&self.player.rect(), &self.level.goal) {
                self.phase = Phase::Won;
            }
        } else if self.phase == Phase::Won && is_key_pressed(KeyCode::Enter) {
            *self = Game::with_phase(Phase::Playing);
            return;
        }

        // --- Cámara con suavizado (independiente del framerate), limitada al mundo
        let target = self.camera_target();
        let t = 1.0 - (-10.0 * frame_dt).exp();
        self.camera += (target - self.camera) * t;
    }

    fn draw(&self) {
        clear_background(Color::from_rgba(40, 44, 60, 255));

        set_camera(&Camera2D {
            target: self.camera.round(), // alineada a píxeles: sin temblor
            zoom: vec2(2.0 / screen_width(), 2.0 / screen_height()),
            ..Default::default()
        });

        // --- Sólidos
        for solid in &self.level.solids {
            draw_rectangle(solid.x, solid.y, solid.w, solid.h, GRAY);
            draw_rectangle(solid.x, solid.y, solid.w, 4.0, LIGHTGRAY);
        }

        // --- Power-ups (antes que los bloques: así "salen" desde dentro de ellos)
        for p in &self.powerups {
            if p.alive {
                p.draw();
            }
        }

        // --- Bloques bonus
        for b in &self.level.bonus_blocks {
            let off = -(b.bump / BUMP_TIME) * 8.0;
            let r = b.rect;
            let fill = if b.used {
                Color::from_rgba(110, 85, 60, 255)
            } else {
                Color::from_rgba(245, 190, 50, 255)
            };
            draw_rectangle(r.x, r.y + off, r.w, r.h, fill);
            draw_rectangle_lines(r.x, r.y + off, r.w, r.h, 3.0, Color::from_rgba(120, 80, 20, 255));
            if !b.used {
                draw_text("?", r.x + 10.0, r.y + off + 25.0, 30.0, Color::from_rgba(120, 70, 10, 255));
            }
        }

        // --- Pinchos
        for s in &self.level.spikes {
            let n = (s.w / 16.0).round().max(1.0) as usize;
            let tw = s.w / n as f32;
            for i in 0..n {
                let x0 = s.x + i as f32 * tw;
                draw_triangle(
                    vec2(x0, s.y + s.h),
                    vec2(x0 + tw, s.y + s.h),
                    vec2(x0 + tw / 2.0, s.y),
                    Color::from_rgba(215, 70, 70, 255),
                );
            }
        }

        // --- Checkpoints
        for cp in &self.level.checkpoints {
            let active = self.checkpoint == cp.respawn;
            let color = if active { GREEN } else { Color::from_rgba(150, 150, 160, 255) };
            draw_rectangle(cp.rect.x, cp.rect.y, 4.0, cp.rect.h, LIGHTGRAY);
            draw_triangle(
                vec2(cp.rect.x + 4.0, cp.rect.y),
                vec2(cp.rect.x + 28.0, cp.rect.y + 10.0),
                vec2(cp.rect.x + 4.0, cp.rect.y + 20.0),
                color,
            );
        }

        // --- Meta (bandera)
        {
            let g = &self.level.goal;
            draw_rectangle(g.x + 4.0, g.y, 4.0, g.h, LIGHTGRAY);
            draw_triangle(
                vec2(g.x + 8.0, g.y),
                vec2(g.x + g.w, g.y + 14.0),
                vec2(g.x + 8.0, g.y + 28.0),
                GOLD,
            );
        }

        // --- Monedas (con un pequeño vaivén)
        let tnow = get_time() as f32;
        for &c in &self.level.coins {
            let bob = (tnow * 4.0 + c.x * 0.05).sin() * 3.0;
            draw_circle(c.x, c.y + bob, COIN_SIZE / 2.0, GOLD);
            draw_circle(c.x - 2.0, c.y + bob - 2.0, 3.0, YELLOW);
        }

        // --- Enemigos
        for e in &self.level.enemies {
            if !e.alive {
                continue;
            }
            let r = e.rect();
            match e.kind {
                EnemyKind::Walker => {
                    draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(150, 70, 200, 255));
                    draw_rectangle(r.x, r.y + r.h - 5.0, r.w, 5.0, Color::from_rgba(110, 45, 150, 255));
                    let eye_x = if e.dir > 0.0 { r.x + r.w - 13.0 } else { r.x + 5.0 };
                    draw_rectangle(eye_x, r.y + 6.0, 8.0, 8.0, WHITE);
                    let pupil_x = if e.dir > 0.0 { eye_x + 4.0 } else { eye_x };
                    draw_rectangle(pupil_x, r.y + 9.0, 4.0, 4.0, BLACK);
                    draw_line(eye_x - 1.0, r.y + 4.0, eye_x + 9.0, r.y + 7.0, 2.0, BLACK);
                }
                EnemyKind::Flyer => {
                    let cx = r.x + r.w / 2.0;
                    let cy = r.y + r.h / 2.0;
                    let flap = (tnow * 18.0).sin() * 8.0;
                    let wing = Color::from_rgba(255, 190, 120, 255);
                    draw_triangle(vec2(cx - 4.0, cy), vec2(cx - 22.0, cy - 6.0 + flap), vec2(cx - 12.0, cy + 4.0), wing);
                    draw_triangle(vec2(cx + 4.0, cy), vec2(cx + 22.0, cy - 6.0 + flap), vec2(cx + 12.0, cy + 4.0), wing);
                    draw_circle(cx, cy, 10.0, Color::from_rgba(240, 120, 40, 255));
                    draw_circle(cx + e.dir * 4.0, cy - 2.0, 3.0, WHITE);
                    draw_circle(cx + e.dir * 5.0, cy - 2.0, 1.5, BLACK);
                }
            }
        }

        // --- Bolas de fuego
        for f in &self.fireballs {
            if f.alive {
                f.draw();
            }
        }

        if self.phase != Phase::Title {
            self.player.draw();
        }

        // --- Textos flotantes
        for p in &self.popups {
            let dims = measure_text(&p.text, None, 24, 1.0);
            let y = p.pos.y - p.t * 40.0;
            let alpha = (1.0 - p.t).max(0.0);
            draw_text(&p.text, p.pos.x - dims.width / 2.0, y, 24.0, Color::new(1.0, 0.85, 0.2, alpha));
        }

        // -------------------------------------------------------------------
        // Interfaz (cámara por defecto)
        // -------------------------------------------------------------------
        set_default_camera();

        if self.phase == Phase::Title {
            self.draw_title();
            return;
        }

        draw_text(
            &format!(
                "Puntos: {}   Monedas: {}/{}   Bonus: {}/{}",
                self.score, self.collected, self.total_coins, self.bonus, self.total_bonus
            ),
            16.0,
            26.0,
            26.0,
            WHITE,
        );
        draw_text(
            &format!(
                "Muertes: {}   Tiempo: {}   Enemigos: {}/{}",
                self.deaths,
                fmt_time(self.time),
                self.kills,
                self.total_enemies
            ),
            16.0,
            50.0,
            24.0,
            LIGHTGRAY,
        );
        draw_text(
            "Objetivo: llega a la bandera del final",
            16.0,
            76.0,
            22.0,
            Color::from_rgba(200, 220, 255, 255),
        );
        if self.player.fire {
            draw_text("Fuego: pulsa F o X para disparar", 16.0, 100.0, 22.0, ORANGE);
        } else if self.player.big {
            draw_text("Tamaño completo: absorbe un golpe", 16.0, 100.0, 22.0, SKYBLUE);
        }

        draw_text(
            "A/D o flechas: mover | Espacio/W: saltar (mantén para más altura) | F/X: fuego | R: reiniciar | Esc: menú",
            16.0,
            screen_height() - 16.0,
            18.0,
            LIGHTGRAY,
        );
        draw_text(
            &format!("FPS: {}", get_fps()),
            screen_width() - 100.0,
            28.0,
            24.0,
            WHITE,
        );

        if self.toast_t > 0.0 {
            draw_centered_text(&self.toast, 130.0, 30, GOLD);
        }

        if self.phase == Phase::Won {
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.65));
            let h = screen_height() / 2.0;
            draw_centered_text("NIVEL COMPLETADO!", h - 70.0, 56, GOLD);
            draw_centered_text(&format!("Puntos: {}", self.score), h - 20.0, 36, WHITE);
            draw_centered_text(
                &format!(
                    "Monedas: {}/{}   Bonus: {}/{}   Enemigos: {}/{}",
                    self.collected, self.total_coins, self.bonus, self.total_bonus, self.kills, self.total_enemies
                ),
                h + 20.0,
                26,
                LIGHTGRAY,
            );
            draw_centered_text(
                &format!("Muertes: {}   Tiempo: {}", self.deaths, fmt_time(self.time)),
                h + 50.0,
                26,
                LIGHTGRAY,
            );
            draw_centered_text("Pulsa ENTER o R para jugar de nuevo", h + 100.0, 26, WHITE);
        }
    }

    fn draw_title(&self) {
        let w = screen_width();
        let h = screen_height();

        draw_rectangle(0.0, 0.0, w, h, Color::new(0.04, 0.05, 0.1, 0.6));

        // Título con sombra
        let title = "PLATAFORMERO";
        draw_centered_text(title, h * 0.27 + 4.0, 80, Color::new(0.0, 0.0, 0.0, 0.7));
        draw_centered_text(title, h * 0.27, 80, GOLD);
        draw_centered_text(
            "Pinchos, enemigos, hongos, fuego y muchas monedas",
            h * 0.27 + 40.0,
            26,
            Color::from_rgba(200, 220, 255, 255),
        );

        // Instrucciones
        let lines = [
            "A/D o flechas: moverse",
            "Espacio, W o flecha arriba: saltar (mantén para más altura)",
            "F o X: lanzar bolas de fuego (con la flor)   R: reiniciar   Esc: menú",
            "Pisa a los enemigos para eliminarlos",
            "Golpea los cuadros ?: hongo para crecer, flor para disparar",
            "Llega a la bandera del final",
        ];
        let mut y = h * 0.5;
        for line in lines.iter() {
            draw_centered_text(line, y, 22, LIGHTGRAY);
            y += 28.0;
        }

        // Aviso parpadeante
        let a = 0.65 + 0.35 * ((get_time() as f32) * 4.0).sin();
        draw_centered_text("Pulsa ESPACIO o ENTER para empezar", h * 0.88, 32, Color::new(1.0, 1.0, 1.0, a));
    }
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
    let mut game = Game::with_phase(Phase::Title);

    loop {
        let frame_dt = get_frame_time().min(0.1);

        if game.phase != Phase::Title {
            if is_key_pressed(KeyCode::R) {
                game = Game::with_phase(Phase::Playing);
            } else if is_key_pressed(KeyCode::Escape) {
                game = Game::with_phase(Phase::Title);
            }
        }

        game.update(frame_dt);
        game.draw();

        next_frame().await;
    }
}