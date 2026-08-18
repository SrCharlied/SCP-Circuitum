use crate::caster::cast_ray;
use crate::maze::Maze;
use crate::player::Player;
use nalgebra_glm::Vec2;
use std::f32::consts::{PI, TAU};

const LINE_OF_SIGHT_MARGIN: f32 = 1.0;
const OBSERVATION_SAMPLE_FACTORS: [f32; 5] = [-1.0, -0.5, 0.0, 0.5, 1.0];

pub struct Scp173 {
    pub pos: Vec2,
    pub height: f32,
    width: f32,
}

impl Scp173 {
    pub fn new(pos: Vec2, height: f32, width_to_height_ratio: f32) -> Self {
        Self {
            pos,
            height,
            width: height * width_to_height_ratio,
        }
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
}
