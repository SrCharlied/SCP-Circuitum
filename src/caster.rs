use crate::maze::Maze;
use crate::player::Player;

pub fn cast_ray(
    maze: &Maze,
    player: &Player,
    angulo: f32,
    block_size: usize,
) -> Option<(f32, char)> {
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
            return Some((distancia, cell));
        }

        distancia += 1.0;
    }
}
