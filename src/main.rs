mod caster;
mod framebuffer;
mod game;
mod maze;
mod player;
mod renderer;

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use std::f32::consts::PI;
use std::time::{Duration, Instant};

use crate::framebuffer::Framebuffer;
use crate::game::{GameSettings, GameState};
use crate::maze::load_maze;
use crate::player::process_events;
use crate::renderer::{
    draw_text, render_3d, render_minimap, render_pause_menu, render_top_down, render_welcome_screen,
};

const BLOCK_SIZE: usize = 100;

/// Amplitud del campo de visión (field of view), en radianes.
const FOV: f32 = PI / 3.0;

fn main() {
    let window_width = 1300;
    let window_height = 900;
    let framebuffer_width = 1300;
    let framebuffer_height = 900;

    let (maze, mut player) = load_maze("./maze.txt", BLOCK_SIZE);

    let mut framebuffer = Framebuffer::new(framebuffer_width, framebuffer_height);
    framebuffer.set_background_color(0x333355);

    let mut window = Window::new(
        "SCP_Circuitum",
        window_width,
        window_height,
        WindowOptions::default(),
    )
    .unwrap();

    let mut previous_frame = Instant::now();
    let mut fps_timer = Instant::now();

    let mut fps_frame_count: u32 = 0;

    let mut fps_text = String::from("FPS: --");

    let mut settings = GameSettings::default();

    let mut game_state = GameState::Welcome;

    while window.is_open() {
        let frame_start = Instant::now();

        let delta_time = frame_start.duration_since(previous_frame).as_secs_f32();

        previous_frame = frame_start;

        if game_state == GameState::Welcome && window.is_key_pressed(Key::Enter, KeyRepeat::No) {
            game_state = GameState::Playing;

            previous_frame = Instant::now();
        }

        if matches!(game_state, GameState::Playing | GameState::Paused)
            && window.is_key_pressed(Key::Escape, KeyRepeat::No)
        {
            game_state.toggle_pause();

            if game_state == GameState::Playing {
                previous_frame = Instant::now();
            }
        }

        if game_state == GameState::Paused {
            if window.is_key_pressed(Key::Right, KeyRepeat::No) {
                settings.select_next_fps();
            }

            if window.is_key_pressed(Key::Left, KeyRepeat::No) {
                settings.select_previous_fps();
            }
        }

        if game_state == GameState::Playing {
            process_events(&window, &mut player, &maze, BLOCK_SIZE, delta_time);

            let i = player.pos.x as usize / BLOCK_SIZE;

            let j = player.pos.y as usize / BLOCK_SIZE;

            if maze.get(j).and_then(|row| row.get(i)) == Some(&'g') {
                println!("¡Meta alcanzada! Fin del juego.");

                break;
            }
        }

        framebuffer.clear();

        if game_state == GameState::Welcome {
            render_welcome_screen(&mut framebuffer);
        } else {
            if window.is_key_down(Key::M) {
                render_top_down(&mut framebuffer, &maze, &player);
            } else {
                render_3d(&mut framebuffer, &maze, &player);

                render_minimap(&mut framebuffer, &maze, &player);
            }

            if settings.show_fps {
                draw_text(&mut framebuffer, &fps_text, 20, 20, 2, 0xFFFFFF);
            }

            if game_state == GameState::Paused {
                render_pause_menu(&mut framebuffer, settings.target_fps());
            }
        }

        window
            .update_with_buffer(&framebuffer.buffer, framebuffer_width, framebuffer_height)
            .unwrap();
        fps_frame_count += 1;

        let fps_elapsed = fps_timer.elapsed();

        if fps_elapsed >= Duration::from_secs(1) {
            let measured_fps = fps_frame_count as f64 / fps_elapsed.as_secs_f64();

            fps_text = format!("FPS: {:.0}", measured_fps,);

            fps_frame_count = 0;
            fps_timer = Instant::now();
        }
        let elapsed_frame_time = frame_start.elapsed();

        let target_frame_time = settings.target_frame_time();

        if elapsed_frame_time < target_frame_time {
            let remaining_frame_time = target_frame_time - elapsed_frame_time;

            std::thread::sleep(remaining_frame_time);
        }
    }
}
