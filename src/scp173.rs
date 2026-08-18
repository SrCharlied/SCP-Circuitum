use crate::caster::cast_ray;
use crate::maze::Maze;
use crate::player::Player;
use nalgebra_glm::Vec2;
use std::f32::consts::{PI, TAU};

const LINE_OF_SIGHT_MARGIN: f32 = 1.0;
const OBSERVATION_SAMPLE_FACTORS: [f32; 5] = [-1.0, -0.5, 0.0, 0.5, 1.0];

// valores de movimiento
const MOVEMENT_SPEED_CELLS_PER_SECOND: f32 = 1.8;
const COLLISION_RADIUS_CELLS: f32 = 0.20;
const STOP_DISTANCE_CELLS: f32 = 0.35;
const MAX_MOVEMENT_DELTA: f32 = 0.05;

pub struct Scp173 {
    pub pos: Vec2,
    pub height: f32,
    width: f32,
    spawn_pos: Vec2,
}

impl Scp173 {
    pub fn new(pos: Vec2, height: f32, width_to_height_ratio: f32) -> Self {
        Self {
            pos,
            height,
            width: height * width_to_height_ratio,
            spawn_pos: pos,
        }
    }

    pub fn reset(&mut self) {
        self.pos = self.spawn_pos;
    }

    pub fn is_observed(&self, maze: &Maze, player: &Player, block_size: usize, fov: f32) -> bool {
        let offset_x = self.pos.x - player.pos.x;
        let offset_y = self.pos.y - player.pos.y;
        let center_distance = offset_x.hypot(offset_y);

        if center_distance <= f32::EPSILON {
            return true;
        }

        let perpendicular_x = -offset_y / center_distance;
        let perpendicular_y = offset_x / center_distance;
        let half_width = self.width / 2.0;

        OBSERVATION_SAMPLE_FACTORS.iter().any(|factor| {
            let sample_x = self.pos.x + perpendicular_x * half_width * factor;
            let sample_y = self.pos.y + perpendicular_y * half_width * factor;

            sample_is_visible(maze, player, sample_x, sample_y, block_size, fov)
        })
    }

    pub fn update(
        &mut self,
        maze: &Maze,
        player: &Player,
        block_size: usize,
        observed: bool,
        delta_time: f32,
    ) {
        if observed || block_size == 0 || delta_time <= 0.0 {
            return;
        }

        let offset_x = player.pos.x - self.pos.x;

        let offset_y = player.pos.y - self.pos.y;

        let distance = offset_x.hypot(offset_y);

        let stop_distance = block_size as f32 * STOP_DISTANCE_CELLS;

        if distance <= stop_distance || distance <= f32::EPSILON {
            return;
        }

        let speed = block_size as f32 * MOVEMENT_SPEED_CELLS_PER_SECOND;

        let movement_distance =
            (speed * delta_time.min(MAX_MOVEMENT_DELTA)).min(distance - stop_distance);

        let direction_x = offset_x / distance;

        let direction_y = offset_y / distance;

        let radius = block_size as f32 * COLLISION_RADIUS_CELLS;

        let next_x = self.pos.x + direction_x * movement_distance;

        if is_walkable(maze, next_x, self.pos.y, radius, block_size) {
            self.pos.x = next_x;
        }

        let next_y = self.pos.y + direction_y * movement_distance;

        if is_walkable(maze, self.pos.x, next_y, radius, block_size) {
            self.pos.y = next_y;
        }
    }
}

fn is_walkable(maze: &Maze, x: f32, y: f32, radius: f32, block_size: usize) -> bool {
    let check_points = [
        (x - radius, y - radius),
        (x + radius, y - radius),
        (x - radius, y + radius),
        (x + radius, y + radius),
    ];

    check_points.iter().all(|(check_x, check_y)| {
        if *check_x < 0.0 || *check_y < 0.0 {
            return false;
        }

        let map_x = *check_x as usize / block_size;

        let map_y = *check_y as usize / block_size;

        let cell = maze.get(map_y).and_then(|row| row.get(map_x)).copied();

        matches!(cell, Some(' ' | 'g' | 'G'))
    })
}

fn sample_is_visible(
    maze: &Maze,
    player: &Player,
    sample_x: f32,
    sample_y: f32,
    block_size: usize,
    fov: f32,
) -> bool {
    let offset_x = sample_x - player.pos.x;
    let offset_y = sample_y - player.pos.y;
    let sample_distance = offset_x.hypot(offset_y);

    if sample_distance <= f32::EPSILON {
        return true;
    }

    let sample_angle = offset_y.atan2(offset_x);
    let relative_angle = normalize_angle(sample_angle - player.a);

    if relative_angle.abs() > fov / 2.0 {
        return false;
    }

    match cast_ray(maze, player, sample_angle, block_size) {
        Some(hit) => hit.distance + LINE_OF_SIGHT_MARGIN >= sample_distance,

        None => true,
    }
}

fn normalize_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

#[cfg(test)]
mod tests {
    use super::Scp173;
    use crate::maze::Maze;
    use crate::player::Player;
    use nalgebra_glm::Vec2;
    use std::f32::consts::PI;

    const BLOCK_SIZE: usize = 100;
    const FOV: f32 = PI / 3.0;

    fn open_room() -> Maze {
        vec![
            vec!['|', '|', '|', '|', '|'],
            vec!['|', ' ', ' ', ' ', '|'],
            vec!['|', ' ', ' ', ' ', '|'],
            vec!['|', ' ', ' ', ' ', '|'],
            vec!['|', '|', '|', '|', '|'],
        ]
    }

    fn scp_at(x: f32, y: f32) -> Scp173 {
        Scp173::new(Vec2::new(x, y), 120.0, 300.0 / 534.0)
    }

    #[test]
    fn observes_scp_inside_fov_with_clear_line_of_sight() {
        let maze = open_room();

        let player = Player::new(Vec2::new(150.0, 250.0), 0.0);

        let scp = scp_at(350.0, 250.0);

        assert!(scp.is_observed(&maze, &player, BLOCK_SIZE, FOV,));
    }

    #[test]
    fn does_not_observe_scp_outside_fov() {
        let maze = open_room();

        let player = Player::new(Vec2::new(150.0, 250.0), PI);

        let scp = scp_at(350.0, 250.0);

        assert!(!scp.is_observed(&maze, &player, BLOCK_SIZE, FOV,));
    }

    #[test]
    fn does_not_observe_scp_behind_wall() {
        let mut maze = open_room();

        maze[2][2] = '|';
        let player = Player::new(Vec2::new(150.0, 250.0), 0.0);

        let scp = scp_at(350.0, 250.0);

        assert!(!scp.is_observed(&maze, &player, BLOCK_SIZE, FOV,));
    }

    #[test]
    fn observes_scp_when_its_edge_intersects_the_fov() {
        let maze = open_room();

        let player = Player::new(Vec2::new(150.0, 250.0), 0.0);

        let scp = scp_at(330.0, 365.0);

        assert!(scp.is_observed(&maze, &player, BLOCK_SIZE, FOV,));
    }

    #[test]
    fn observed_scp_does_not_move() {
        let maze = open_room();

        let player = Player::new(Vec2::new(350.0, 250.0), 0.0);

        let mut scp = scp_at(150.0, 250.0);

        let initial_position = scp.pos;

        scp.update(&maze, &player, BLOCK_SIZE, true, 1.0 / 60.0);

        assert_eq!(scp.pos, initial_position,);
    }

    #[test]
    fn unobserved_scp_moves_toward_player() {
        let maze = open_room();

        let player = Player::new(Vec2::new(350.0, 250.0), 0.0);

        let mut scp = scp_at(150.0, 250.0);

        let initial_distance = (player.pos - scp.pos).norm();

        scp.update(&maze, &player, BLOCK_SIZE, false, 1.0 / 60.0);

        assert!((player.pos - scp.pos).norm() < initial_distance);
    }

    #[test]
    fn scp_does_not_cross_walls() {
        let mut maze = open_room();

        maze[2][2] = '|';

        let player = Player::new(Vec2::new(350.0, 250.0), 0.0);

        let mut scp = scp_at(150.0, 250.0);

        for _ in 0..120 {
            scp.update(&maze, &player, BLOCK_SIZE, false, 1.0 / 60.0);
        }

        assert!(scp.pos.x <= 180.0 + f32::EPSILON);

        assert_eq!(scp.pos.y, 250.0,);
    }

    #[test]
    fn movement_is_stable_across_frame_rates() {
        let maze = open_room();

        let player = Player::new(Vec2::new(350.0, 250.0), 0.0);

        let mut scp_at_60_fps = scp_at(150.0, 250.0);

        let mut scp_at_120_fps = scp_at(150.0, 250.0);

        for _ in 0..30 {
            scp_at_60_fps.update(&maze, &player, BLOCK_SIZE, false, 1.0 / 60.0);
        }

        for _ in 0..60 {
            scp_at_120_fps.update(&maze, &player, BLOCK_SIZE, false, 1.0 / 120.0);
        }

        assert!((scp_at_60_fps.pos.x - scp_at_120_fps.pos.x).abs() < 0.01);

        assert!((scp_at_60_fps.pos.y - scp_at_120_fps.pos.y).abs() < 0.01);
    }
}
