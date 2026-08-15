mod caster;
mod framebuffer;
mod game;
mod maze;
mod player;
mod renderer;
mod texture;

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use std::f32::consts::PI;
use std::time::{Duration, Instant};

use crate::framebuffer::Framebuffer;
use crate::game::{GameSession, GameSettings, GameState, VictoryMenuOption};
use crate::maze::load_maze;
use crate::player::process_events;
use crate::renderer::{
    WorldSprite, draw_text, render_3d, render_level_transition, render_minimap, render_pause_menu,
    render_stamina_bar, render_test_sprite, render_victory_screen, render_welcome_screen,
};
use crate::texture::{SpriteTexture, TextureSet};

const BLOCK_SIZE: usize = 100;

/// Amplitud del campo de visión (field of view), en radianes.
const FOV: f32 = PI / 3.0;

const LEVEL_TRANSITION_DURATION: f32 = 4.5;

fn main() {
    let window_width = 1300;
    let window_height = 900;
    let framebuffer_width = 1300;
    let framebuffer_height = 900;

    let mut game_session = GameSession::default();

    let (mut maze, mut player) = load_maze(game_session.current_level_path(), BLOCK_SIZE);

    let textures = TextureSet::from_files(
        &[
            "./assets/textures/wall_industrial.png",
            "./assets/textures/wall_industrial_connected.png",
        ],
        "./assets/textures/column_reinforced.png",
        "./assets/textures/goal_elevator.png",
    )
    .unwrap_or_else(|error| panic!("{error}"));

    let mut framebuffer = Framebuffer::new(framebuffer_width, framebuffer_height);
    framebuffer.set_background_color(0x333355);
    let mut depth_buffer = vec![f32::INFINITY; framebuffer_width];

    let test_sprite = WorldSprite {
        x: 550.0,
        y: 150.0,
        size: 120.0,
    };

    let test_sprite_texture = SpriteTexture::from_file("./assets/sprites/scp_173.png")
        .unwrap_or_else(|error| panic!("{error}"));

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

    let mut render_3d_total = Duration::ZERO;

    let mut render_3d_max = Duration::ZERO;

    let mut render_3d_samples: u32 = 0;

    let mut render_3d_text = String::from("3D: -- ms | max -- ms");

    let mut settings = GameSettings::default();

    let mut level_transition_remaining = 0.0_f32;

    let mut game_state = GameState::Welcome;

    let mut victory_menu_option = VictoryMenuOption::default();

    let mut welcome_enter_locked = false;

    while window.is_open() {
        let frame_start = Instant::now();

        let delta_time = frame_start.duration_since(previous_frame).as_secs_f32();

        previous_frame = frame_start;

        match game_state {
            GameState::Welcome => {
                if welcome_enter_locked {
                    if !window.is_key_down(Key::Enter) {
                        welcome_enter_locked = false;
                    }
                } else if window.is_key_pressed(Key::Enter, KeyRepeat::No) {
                    game_state = GameState::Playing;

                    previous_frame = Instant::now();
                }
            }

            GameState::Playing => {
                if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
                    game_state.toggle_pause();
                }
            }

            GameState::Paused => {
                if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
                    game_state.toggle_pause();

                    previous_frame = Instant::now();
                } else {
                    if window.is_key_pressed(Key::Right, KeyRepeat::No) {
                        settings.select_next_fps();
                    }

                    if window.is_key_pressed(Key::Left, KeyRepeat::No) {
                        settings.select_previous_fps();
                    }
                }
            }

            GameState::LevelTransition => {}

            GameState::Victory => {
                if window.is_key_pressed(Key::Right, KeyRepeat::No) {
                    victory_menu_option.select_next();
                }

                if window.is_key_pressed(Key::Left, KeyRepeat::No) {
                    victory_menu_option.select_previous();
                }

                if window.is_key_pressed(Key::Enter, KeyRepeat::No) {
                    match victory_menu_option {
                        VictoryMenuOption::MainMenu => {
                            game_session.reset();

                            let (first_level_maze, first_level_player) =
                                load_maze(game_session.current_level_path(), BLOCK_SIZE);

                            maze = first_level_maze;
                            player = first_level_player;

                            game_state = GameState::Welcome;

                            victory_menu_option = VictoryMenuOption::default();

                            welcome_enter_locked = true;

                            previous_frame = Instant::now();
                        }

                        VictoryMenuOption::Exit => {
                            break;
                        }
                    }
                }
            }
        }

        if game_state == GameState::Playing {
            process_events(&window, &mut player, &maze, BLOCK_SIZE, delta_time);

            let map_x = player.pos.x as usize / BLOCK_SIZE;

            let map_y = player.pos.y as usize / BLOCK_SIZE;

            let current_cell = maze.get(map_y).and_then(|row| row.get(map_x)).copied();

            if matches!(current_cell, Some('g' | 'G')) {
                if game_session.advance_level() {
                    game_state = GameState::LevelTransition;

                    level_transition_remaining = LEVEL_TRANSITION_DURATION;

                    println!("Elevador hacia: {}", game_session.current_level_path(),);
                } else {
                    game_state = GameState::Victory;

                    victory_menu_option = VictoryMenuOption::default();

                    println!("¡Último nivel completado! Victoria.");
                }
            }
        }

        if game_state == GameState::LevelTransition {
            level_transition_remaining = (level_transition_remaining - delta_time).max(0.0);

            if level_transition_remaining <= 0.0 {
                let (next_maze, next_player) =
                    load_maze(game_session.current_level_path(), BLOCK_SIZE);

                maze = next_maze;
                player = next_player;

                game_state = GameState::Playing;

                previous_frame = Instant::now();

                println!("Nivel cargado: {}", game_session.current_level_path(),);
            }
        }

        framebuffer.clear();

        match game_state {
            GameState::Welcome => {
                render_welcome_screen(&mut framebuffer);
            }

            GameState::Victory => {
                render_victory_screen(&mut framebuffer, victory_menu_option);
            }

            GameState::Playing | GameState::Paused => {
                let render_3d_start = Instant::now();

                render_3d(
                    &mut framebuffer,
                    &mut depth_buffer,
                    &maze,
                    &player,
                    &textures,
                    game_session.current_level_number(),
                );

                render_test_sprite(
                    &mut framebuffer,
                    &depth_buffer,
                    &player,
                    &test_sprite,
                    &test_sprite_texture,
                );

                let render_3d_elapsed = render_3d_start.elapsed();

                render_3d_total += render_3d_elapsed;

                render_3d_max = render_3d_max.max(render_3d_elapsed);

                render_3d_samples += 1;

                render_minimap(&mut framebuffer, &maze, &player);

                if settings.show_fps {
                    draw_text(&mut framebuffer, &fps_text, 20, 20, 2, 0xFFFFFF);
                    draw_text(&mut framebuffer, &render_3d_text, 20, 40, 2, 0xFFFFFF);
                }

                render_stamina_bar(
                    &mut framebuffer,
                    player.stamina_ratio(),
                    player.is_sprint_exhausted(),
                );

                if game_state == GameState::Paused {
                    render_pause_menu(&mut framebuffer, settings.target_fps());
                }
            }

            GameState::LevelTransition => {
                let transition_progress =
                    (1.0 - level_transition_remaining / LEVEL_TRANSITION_DURATION).clamp(0.0, 1.0);

                render_level_transition(
                    &mut framebuffer,
                    game_session.current_level_number(),
                    transition_progress,
                );
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

            if render_3d_samples > 0 {
                let average_render_3d_ms =
                    render_3d_total.as_secs_f64() * 1000.0 / render_3d_samples as f64;

                render_3d_text = format!(
                    "3D: {:.2} ms | max {:.2} ms",
                    average_render_3d_ms,
                    render_3d_max.as_secs_f64() * 1000.0,
                );
            }

            fps_frame_count = 0;
            fps_timer = Instant::now();

            render_3d_total = Duration::ZERO;

            render_3d_max = Duration::ZERO;

            render_3d_samples = 0;
        }

        let elapsed_frame_time = frame_start.elapsed();

        let target_frame_time = settings.target_frame_time();

        if elapsed_frame_time < target_frame_time {
            let remaining_frame_time = target_frame_time - elapsed_frame_time;

            std::thread::sleep(remaining_frame_time);
        }
    }
}
