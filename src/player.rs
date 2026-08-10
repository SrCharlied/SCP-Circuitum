use crate::maze::Maze;
use minifb::{Key, Window};
use nalgebra_glm::Vec2;
use std::f32::consts::PI;

const PLAYER_RADIUS_FACTOR: f32 = 0.20;

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
) {
    const ROTATION_SPEED: f32 = PI * 1.2;

    if window.is_key_down(Key::A) {
        player.a -= ROTATION_SPEED * delta_time;
    }

    if window.is_key_down(Key::D) {
        player.a += ROTATION_SPEED * delta_time;
    }

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
