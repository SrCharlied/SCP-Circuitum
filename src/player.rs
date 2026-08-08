use crate::maze::Maze;
use minifb::{Key, Window};
use nalgebra_glm::Vec2;
use std::f32::consts::PI;

pub struct Player {
    pub pos: Vec2,
    pub a: f32,
}

fn is_walkable(maze: &Maze, x: f32, y: f32, block_size: usize) -> bool {
    if x < 0.0 || y < 0.0 {
        return false;
    }

    let map_x = x as usize / block_size;

    let map_y = y as usize / block_size;

    let cell = maze.get(map_y).and_then(|row| row.get(map_x)).copied();

    matches!(cell, Some(' ' | 'g' | 'G'))
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

    if window.is_key_down(Key::W) {
        let next_x = player.pos.x + MOVE_SPEED * player.a.cos();

        let next_y = player.pos.y + MOVE_SPEED * player.a.sin();

        if is_walkable(maze, next_x, next_y, block_size) {
            player.pos.x = next_x;
            player.pos.y = next_y;
        }
    }

    if window.is_key_down(Key::S) {
        let next_x = player.pos.x - MOVE_SPEED * player.a.cos();

        let next_y = player.pos.y - MOVE_SPEED * player.a.sin();

        if is_walkable(maze, next_x, next_y, block_size) {
            player.pos.x = next_x;
            player.pos.y = next_y;
        }
    }
}
