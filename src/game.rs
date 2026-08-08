use std::time::Duration;

const FPS_OPTIONS: [u32; 3] = [60, 120, 144];

pub struct GameSettings {
    selected_fps_index: usize,
    pub show_fps: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            selected_fps_index: 0,
            show_fps: true,
        }
    }
}

impl GameSettings {
    pub fn target_fps(&self) -> u32 {
        FPS_OPTIONS[self.selected_fps_index]
    }

    pub fn target_frame_time(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.target_fps() as f64)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Playing,
    Paused,
}

impl GameState {
    pub fn toggle_pause(&mut self) {
        *self = match *self {
            GameState::Playing => GameState::Paused,

            GameState::Paused => GameState::Playing,
        };
    }
}
