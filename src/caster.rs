use crate::maze::Maze;
use crate::player::Player;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallSide {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub distance: f32,
    pub cell: char,
    pub hit_x: f32,
    pub hit_y: f32,
    pub side: WallSide,
}

pub fn cast_ray(maze: &Maze, player: &Player, angulo: f32, block_size: usize) -> Option<RayHit> {
    if maze.is_empty() || block_size == 0 {
        return None;
    }

    let block_size_f32 = block_size as f32;

    // DDA trabaja en coordenadas de
    // celdas, no en píxeles del mundo.
    let position_x = player.pos.x / block_size_f32;

    let position_y = player.pos.y / block_size_f32;

    let direction_x = angulo.cos();
    let direction_y = angulo.sin();

    let mut map_x = position_x.floor() as i32;

    let mut map_y = position_y.floor() as i32;

    if map_x < 0 || map_y < 0 {
        return None;
    }

    // Distancia necesaria para cruzar
    // una celda completa en cada eje.
    let delta_distance_x = if direction_x.abs() <= f32::EPSILON {
        f32::INFINITY
    } else {
        (1.0 / direction_x).abs()
    };

    let delta_distance_y = if direction_y.abs() <= f32::EPSILON {
        f32::INFINITY
    } else {
        (1.0 / direction_y).abs()
    };

    // Dirección de avance y distancia
    // hasta el primer borde vertical.
    let (step_x, mut side_distance_x) = if direction_x < 0.0 {
        (-1, (position_x - map_x as f32) * delta_distance_x)
    } else {
        (1, (map_x as f32 + 1.0 - position_x) * delta_distance_x)
    };

    // Dirección de avance y distancia
    // hasta el primer borde horizontal.
    let (step_y, mut side_distance_y) = if direction_y < 0.0 {
        (-1, (position_y - map_y as f32) * delta_distance_y)
    } else {
        (1, (map_y as f32 + 1.0 - position_y) * delta_distance_y)
    };

    loop {
        let (distance_in_cells, side) = if side_distance_x < side_distance_y {
            let distance = side_distance_x;

            side_distance_x += delta_distance_x;

            map_x += step_x;

            (distance, WallSide::Vertical)
        } else {
            let distance = side_distance_y;

            side_distance_y += delta_distance_y;

            map_y += step_y;

            (distance, WallSide::Horizontal)
        };

        if map_x < 0 || map_y < 0 {
            return None;
        }

        let map_x_usize = map_x as usize;

        let map_y_usize = map_y as usize;

        let cell = maze
            .get(map_y_usize)
            .and_then(|row| row.get(map_x_usize))
            .copied()?;

        if cell == ' ' {
            continue;
        }
        // Convertir la distancia medida
        // en celdas nuevamente a unidades
        // del mundo.
        let distance = distance_in_cells * block_size_f32;

        let hit_x = player.pos.x + distance * direction_x;

        let hit_y = player.pos.y + distance * direction_y;

        return Some(RayHit {
            distance,
            cell,
            hit_x,
            hit_y,
            side,
        });
    }
}
