use crate::maze::Maze;
use minifb::{Key, Window};
use nalgebra_glm::Vec2;
use std::f32::consts::PI;

const PLAYER_RADIUS_FACTOR: f32 = 0.20;
pub struct Player {
    pub pos: Vec2,
    pub a: f32,
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

pub fn process_events(window: &Window, player: &mut Player, maze: &Maze, block_size: usize) {
    const MOVE_SPEED: f32 = 10.0;
    const ROTATION_SPEED: f32 = PI / 50.0;

    if window.is_key_down(Key::A) {
        player.a -= ROTATION_SPEED;
    }

    if window.is_key_down(Key::D) {
        player.a += ROTATION_SPEED;
    }

    let mut movement = 0.0;

    if window.is_key_down(Key::W) {
        movement += MOVE_SPEED;
    }

    if window.is_key_down(Key::S) {
        movement -= MOVE_SPEED;
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
