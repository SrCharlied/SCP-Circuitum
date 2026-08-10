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

    pub fn select_next_fps(&mut self) {
        self.selected_fps_index = (self.selected_fps_index + 1) % FPS_OPTIONS.len();
    }

    pub fn select_previous_fps(&mut self) {
        if self.selected_fps_index == 0 {
            self.selected_fps_index = FPS_OPTIONS.len() - 1;
        } else {
            self.selected_fps_index -= 1;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Welcome,
    Playing,
    Paused,
    Victory,
}

impl GameState {
    pub fn toggle_pause(&mut self) {
        *self = match *self {
            GameState::Playing => GameState::Paused,

            GameState::Paused => GameState::Playing,

            other => other,
        };
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VictoryMenuOption {
    MainMenu,
    Exit,
}

impl Default for VictoryMenuOption {
    fn default() -> Self {
        Self::MainMenu
    }
}

impl VictoryMenuOption {
    pub fn select_next(&mut self) {
        *self = match *self {
            Self::MainMenu => Self::Exit,
            Self::Exit => Self::MainMenu,
        };
    }

    pub fn select_previous(&mut self) {
        self.select_next();
    }
}
