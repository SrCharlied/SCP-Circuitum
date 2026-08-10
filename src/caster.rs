use crate::maze::Maze;
use crate::player::Player;

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub distance: f32,
    pub cell: char,
    pub hit_x: f32,
    pub hit_y: f32,
}

pub fn cast_ray(maze: &Maze, player: &Player, angulo: f32, block_size: usize) -> Option<RayHit> {
    let mut distancia = 0.0;

    loop {
        let world_x = player.pos.x + distancia * angulo.cos();
        let world_y = player.pos.y + distancia * angulo.sin();

        if world_x < 0.0 || world_y < 0.0 {
            return None;
        }

        let x = world_x as usize;
        let y = world_y as usize;

        let map_x = x / block_size;
        let map_y = y / block_size;

        if map_y >= maze.len() || map_x >= maze[map_y].len() {
            return None;
        }

        let cell = maze[map_y][map_x];

        if cell != ' ' {
            return Some(RayHit {
                distance: distancia,
                cell,
                hit_x: world_x,
                hit_y: world_y,
            });
        }

        distancia += 1.0;
    }
}
