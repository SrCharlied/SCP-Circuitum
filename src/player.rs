use crate::maze::Maze;
use minifb::{Key, Window};
use nalgebra_glm::Vec2;
use std::f32::consts::{PI, TAU};

const PLAYER_RADIUS_FACTOR: f32 = 0.20;

/// Radianes girados por cada píxel que recorre el mouse en
/// horizontal. Con la ventana de 1300 px, cruzarla entera equivale
/// a poco más de media vuelta. Es el único valor a tocar si la
/// rotación se siente lenta o brusca tras la prueba manual.
const MOUSE_ROTATION_SENSITIVITY: f32 = 0.0025;

pub const MAX_STAMINA: f32 = 7.0;

const WALK_SPEED: f32 = 350.0;
const SPRINT_SPEED: f32 = 500.0;
const EXHAUSTED_SPEED: f32 = 100.0;

const STAMINA_REGEN_DELAY: f32 = 1.0;
const STAMINA_REGEN_RATE: f32 = 1.4;

pub struct Player {
    pub pos: Vec2,
    pub a: f32,
    pub stamina: f32,
    stamina_regen_delay_remaining: f32,
    sprint_exhausted: bool,
}

impl Player {
    pub fn new(pos: Vec2, angle: f32) -> Self {
        Self {
            pos,
            a: angle,
            stamina: MAX_STAMINA,
            stamina_regen_delay_remaining: 0.0,
            sprint_exhausted: false,
        }
    }

    pub fn stamina_ratio(&self) -> f32 {
        (self.stamina / MAX_STAMINA).clamp(0.0, 1.0)
    }

    pub fn is_sprint_exhausted(&self) -> bool {
        self.sprint_exhausted
    }
}

/// Recuerda la última muestra horizontal del mouse para poder
/// convertir posiciones absolutas en desplazamientos relativos.
///
/// Solo guarda la coordenada X: la vista no tiene rotación vertical,
/// así que la Y nunca entra al estado.
#[derive(Debug, Default)]
pub struct MouseLook {
    previous_x: Option<f32>,
}

impl MouseLook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Consume la muestra horizontal del frame y devuelve cuánto se
    /// desplazó el mouse desde la anterior.
    ///
    /// La primera muestra tras un `reset` solo fija la referencia y
    /// devuelve `0.0`, de modo que empezar la partida o reanudarla
    /// nunca produce un salto de cámara. Si no hay muestra —el
    /// cursor salió de la ventana— la referencia se descarta, y al
    /// volver a entrar se vuelve a empezar desde cero.
    pub fn horizontal_delta(&mut self, sample_x: Option<f32>) -> f32 {
        let Some(current_x) = sample_x else {
            self.reset();

            return 0.0;
        };

        let delta = match self.previous_x {
            Some(previous_x) => current_x - previous_x,

            None => 0.0,
        };

        self.previous_x = Some(current_x);

        delta
    }

    pub fn reset(&mut self) {
        self.previous_x = None;
    }
}

/// Reduce un ángulo al rango [-PI, PI) para que no crezca sin
/// límite mientras el jugador gira.
///
/// `rem_euclid` devuelve [0, TAU), así que el extremo incluido es
/// `-PI` y el excluido es `PI`.
pub fn normalize_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// Combina las dos fuentes de rotación de un frame sobre el mismo
/// ángulo, sin que ninguna sobrescriba a la otra.
///
/// `mouse_delta_x` ya es una distancia recorrida entre muestras, así
/// que no se escala por `delta_time`: hacerlo ataría la sensibilidad
/// a los FPS. `keyboard_rotation` sí llega ya multiplicado por
/// `delta_time`, porque ahí A y D expresan una velocidad de giro.
pub fn rotated_angle(angle: f32, mouse_delta_x: f32, keyboard_rotation: f32) -> f32 {
    normalize_angle(angle + mouse_delta_x * MOUSE_ROTATION_SENSITIVITY + keyboard_rotation)
}

fn is_walkable(maze: &Maze, x: f32, y: f32, block_size: usize) -> bool {
    let radius = block_size as f32 * PLAYER_RADIUS_FACTOR;

    let check_points = [
        (x - radius, y - radius),
        (x + radius, y - radius),
        (x - radius, y + radius),
        (x + radius, y + radius),
    ];

    for (check_x, check_y) in check_points {
        if check_x < 0.0 || check_y < 0.0 {
            return false;
        }

        let map_x = check_x as usize / block_size;

        let map_y = check_y as usize / block_size;

        let cell = maze.get(map_y).and_then(|row| row.get(map_x)).copied();

        if !matches!(cell, Some(' ' | 'g' | 'G')) {
            return false;
        }
    }

    true
}

pub fn process_events(
    window: &Window,
    player: &mut Player,
    maze: &Maze,
    block_size: usize,
    delta_time: f32,
    mouse_delta_x: f32,
) {
    const ROTATION_SPEED: f32 = PI * 1.2;

    let mut keyboard_rotation = 0.0;

    if window.is_key_down(Key::A) {
        keyboard_rotation -= ROTATION_SPEED * delta_time;
    }

    if window.is_key_down(Key::D) {
        keyboard_rotation += ROTATION_SPEED * delta_time;
    }

    player.a = rotated_angle(player.a, mouse_delta_x, keyboard_rotation);

    let moving_forward = window.is_key_down(Key::W) && !window.is_key_down(Key::S);

    let sprint_key_pressed =
        window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);

    let sprint_requested = moving_forward && sprint_key_pressed;

    let can_sprint = !player.sprint_exhausted && player.stamina > 0.0;

    let is_sprinting = sprint_requested && can_sprint;

    if is_sprinting {
        player.stamina = (player.stamina - delta_time).max(0.0);

        player.stamina_regen_delay_remaining = STAMINA_REGEN_DELAY;

        if player.stamina <= f32::EPSILON {
            player.stamina = 0.0;
            player.sprint_exhausted = true;
        }
    } else {
        player.stamina_regen_delay_remaining =
            (player.stamina_regen_delay_remaining - delta_time).max(0.0);

        if player.stamina_regen_delay_remaining <= 0.0 && player.stamina < MAX_STAMINA {
            player.stamina = (player.stamina + STAMINA_REGEN_RATE * delta_time).min(MAX_STAMINA);
        }

        if player.sprint_exhausted && player.stamina >= MAX_STAMINA && !sprint_key_pressed {
            player.sprint_exhausted = false;
        }
    }
    let walking_speed = if player.sprint_exhausted {
        EXHAUSTED_SPEED
    } else {
        WALK_SPEED
    };

    let forward_speed = if is_sprinting {
        SPRINT_SPEED
    } else {
        walking_speed
    };

    let mut movement = 0.0;

    if window.is_key_down(Key::W) {
        movement += forward_speed * delta_time;
    }

    if window.is_key_down(Key::S) {
        movement -= walking_speed * delta_time;
    }

    if movement != 0.0 {
        let delta_x = movement * player.a.cos();

        let delta_y = movement * player.a.sin();

        let next_x = player.pos.x + delta_x;

        if is_walkable(maze, next_x, player.pos.y, block_size) {
            player.pos.x = next_x;
        }

        let next_y = player.pos.y + delta_y;

        if is_walkable(maze, player.pos.x, next_y, block_size) {
            player.pos.y = next_y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MOUSE_ROTATION_SENSITIVITY, MouseLook, normalize_angle, rotated_angle};
    use std::f32::consts::{PI, TAU};

    #[test]
    fn the_first_sample_only_sets_the_reference() {
        let mut mouse_look = MouseLook::new();

        assert_eq!(mouse_look.horizontal_delta(Some(640.0)), 0.0);
    }

    #[test]
    fn moving_right_produces_a_positive_delta() {
        let mut mouse_look = MouseLook::new();

        mouse_look.horizontal_delta(Some(640.0));

        assert_eq!(mouse_look.horizontal_delta(Some(700.0)), 60.0);
    }

    #[test]
    fn moving_left_produces_a_negative_delta() {
        let mut mouse_look = MouseLook::new();

        mouse_look.horizontal_delta(Some(640.0));

        assert_eq!(mouse_look.horizontal_delta(Some(580.0)), -60.0);
    }

    #[test]
    fn reset_drops_the_previous_reference() {
        let mut mouse_look = MouseLook::new();

        mouse_look.horizontal_delta(Some(640.0));

        mouse_look.reset();

        // Sin referencia, una posición muy lejana no puede provocar
        // un salto: solo vuelve a fijar el punto de partida.
        assert_eq!(mouse_look.horizontal_delta(Some(20.0)), 0.0);

        assert_eq!(mouse_look.horizontal_delta(Some(30.0)), 10.0);
    }

    #[test]
    fn a_missing_sample_drops_the_reference() {
        let mut mouse_look = MouseLook::new();

        mouse_look.horizontal_delta(Some(640.0));

        // El cursor salió de la ventana.
        assert_eq!(mouse_look.horizontal_delta(None), 0.0);

        // Al volver a entrar por el borde opuesto no hay salto.
        assert_eq!(mouse_look.horizontal_delta(Some(1290.0)), 0.0);
    }

    #[test]
    fn the_angle_stays_inside_a_single_turn() {
        for steps in 0..64 {
            let angle = normalize_angle(steps as f32 * 1.7);

            assert!(angle >= -PI && angle < PI, "ángulo fuera de rango: {angle}");
        }

        // Girar muchas vueltas no hace crecer el ángulo.
        let mut angle = 0.0;

        for _ in 0..1_000 {
            angle = rotated_angle(angle, 400.0, 0.0);
        }

        assert!(angle >= -PI && angle < PI, "ángulo fuera de rango: {angle}");
    }

    #[test]
    fn the_normalized_range_includes_minus_pi_and_excludes_pi() {
        // El rango real es [-PI, PI): `rem_euclid` devuelve [0, TAU),
        // así que media vuelta cae siempre en el extremo negativo.
        assert_eq!(normalize_angle(-PI), -PI);

        assert_eq!(normalize_angle(PI), -PI);
    }

    #[test]
    fn mouse_rotation_does_not_depend_on_the_frame_rate() {
        // El mismo recorrido horizontal repartido en distinta
        // cantidad de frames debe girar lo mismo.
        let mut angle_at_60_fps = 0.0;

        for _ in 0..10 {
            angle_at_60_fps = rotated_angle(angle_at_60_fps, 6.0, 0.0);
        }

        let mut angle_at_144_fps = 0.0;

        for _ in 0..24 {
            angle_at_144_fps = rotated_angle(angle_at_144_fps, 2.5, 0.0);
        }

        assert!(
            (angle_at_60_fps - angle_at_144_fps).abs() < 1e-4,
            "60 FPS dio {angle_at_60_fps} y 144 FPS dio {angle_at_144_fps}",
        );

        // Y ese giro corresponde al recorrido total, no al número
        // de muestras.
        let expected = 60.0 * MOUSE_ROTATION_SENSITIVITY;

        assert!((angle_at_60_fps - expected).abs() < 1e-4);
    }

    #[test]
    fn keyboard_and_mouse_rotate_the_same_angle_without_overwriting() {
        let keyboard_rotation = 0.20;

        let mouse_delta_x = 40.0;

        let combined = rotated_angle(0.0, mouse_delta_x, keyboard_rotation);

        let expected =
            normalize_angle(mouse_delta_x * MOUSE_ROTATION_SENSITIVITY + keyboard_rotation);

        assert!((combined - expected).abs() < 1e-6);

        // Ninguna de las dos fuentes anula a la otra.
        assert!(combined > rotated_angle(0.0, mouse_delta_x, 0.0));

        assert!(combined > rotated_angle(0.0, 0.0, keyboard_rotation));
    }

    #[test]
    fn a_still_mouse_does_not_rotate_the_camera() {
        let mut mouse_look = MouseLook::new();

        mouse_look.horizontal_delta(Some(650.0));

        let delta = mouse_look.horizontal_delta(Some(650.0));

        assert_eq!(delta, 0.0);

        assert_eq!(rotated_angle(1.0, delta, 0.0), normalize_angle(1.0));
    }

    #[test]
    fn normalize_angle_keeps_equivalent_directions_together() {
        let angle = 0.7;

        assert!((normalize_angle(angle + TAU) - angle).abs() < 1e-5);

        assert!((normalize_angle(angle - TAU) - angle).abs() < 1e-5);
    }
}
