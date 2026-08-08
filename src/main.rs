mod caster;
mod framebuffer;
mod maze;
mod player;
mod renderer;

use minifb::{Key, Window, WindowOptions};
use std::f32::consts::PI;
use std::time::{Duration, Instant};

use crate::framebuffer::Framebuffer;
use crate::maze::load_maze;
use crate::player::process_events;
use crate::renderer::{render_3d, render_minimap, render_top_down};

const BLOCK_SIZE: usize = 100;

const TARGET_FPS: f64 = 60.0;

/// Amplitud del campo de visión (field of view), en radianes.
const FOV: f32 = PI / 3.0;

fn main() {
    let window_width = 1300;
    let window_height = 900;
    let framebuffer_width = 1300;
    let framebuffer_height = 900;
    let target_frame_time = Duration::from_secs_f64(1.0 / TARGET_FPS);

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

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let frame_start = Instant::now();

        let delta_time = frame_start.duration_since(previous_frame).as_secs_f32();

        previous_frame = frame_start;

        process_events(&window, &mut player, &maze, BLOCK_SIZE, delta_time);

        // ¿el jugador llegó a la meta? Se traduce su posición en píxeles a la
        // celda que ocupa y se revisa si esa celda es la marca `g`.
        let i = player.pos.x as usize / BLOCK_SIZE;
        let j = player.pos.y as usize / BLOCK_SIZE;
        if maze.get(j).and_then(|row| row.get(i)) == Some(&'g') {
            println!("¡Meta alcanzada! Fin del juego.");
            break;
        }

        framebuffer.clear();

        if window.is_key_down(Key::M) {
            render_top_down(&mut framebuffer, &maze, &player);
        } else {
            render_3d(&mut framebuffer, &maze, &player);

            render_minimap(&mut framebuffer, &maze, &player);
        }

        window
            .update_with_buffer(&framebuffer.buffer, framebuffer_width, framebuffer_height)
            .unwrap();
        fps_frame_count += 1;

        let fps_elapsed = fps_timer.elapsed();

        if fps_elapsed >= Duration::from_secs(1) {
            let fps = fps_frame_count as f64 / fps_elapsed.as_secs_f64();

            window.set_title(&format!("SCP_Circuitum | FPS: {:.0}", fps,));

            fps_frame_count = 0;
            fps_timer = Instant::now();
        }

        let elapsed_frame_time = frame_start.elapsed();

        if elapsed_frame_time < target_frame_time {
            let remaining_frame_time = target_frame_time - elapsed_frame_time;

            std::thread::sleep(remaining_frame_time);
        }
    }
}
