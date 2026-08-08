mod caster;
mod framebuffer;
mod maze;
mod player;

use minifb::{Key, Window, WindowOptions};
use std::f32::consts::PI;
use std::time::Duration;

use crate::caster::cast_ray;
use crate::framebuffer::Framebuffer;
use crate::maze::{load_maze, Maze};
use crate::player::{process_events, Player};

const BLOCK_SIZE: usize = 100;

/// Cantidad de rayos que se lanzan en abanico para formar el campo de visión.
const NUM_RAYS: usize = 5;

/// Amplitud del campo de visión (field of view), en radianes.
const FOV: f32 = PI / 3.0;

fn cell_color(cell: char) -> u32 {
    match cell {
        '+' => 0x00AAFF, // columnas
        '-' => 0xFF5555, // paredes horizontales
        '|' => 0xFF5555, // paredes verticales
        'g' | 'G' => 0x00FF00, // meta
        _ => 0xFFDDDD,   // cualquier otra cosa
    }
}

fn draw_cell(framebuffer: &mut Framebuffer, xo: usize, yo: usize, cell: char) {
    if cell == ' ' {
        return;
    }

    framebuffer.set_current_color(cell_color(cell));

    for x in xo..xo + BLOCK_SIZE {
        for y in yo..yo + BLOCK_SIZE {
            framebuffer.point(x, y);
        }
    }
}

fn render_3d(
    framebuffer: &mut Framebuffer,
    maze: &Maze,
    player: &Player,
) {
    let width = framebuffer.width;
    let height = framebuffer.height;

    let horizon = height as f32 / 2.0;

    let proyeccion_distancia =
        (width as f32 / 2.0) / (FOV / 2.0).tan();

    let delta_beta =
        FOV / (width - 1) as f32;

    for i in 0..width {
        let beta =
            -FOV / 2.0 + delta_beta * i as f32;

        let ray_angle =
            player.a + beta;

        if let Some((raw_distance, wall)) =
            cast_ray(maze, player, ray_angle, BLOCK_SIZE)
        {
            let distancia_corregida =
                raw_distance * beta.cos();

            // Evita dividir entre cero si el jugador queda
            // demasiado cerca o dentro de una pared.
            if distancia_corregida <= 0.0 {
                continue;
            }

            let wall_height =
                (BLOCK_SIZE as f32 / distancia_corregida)
                    * proyeccion_distancia;

            let top =
                horizon - wall_height / 2.0;

            let bottom =
                horizon + wall_height / 2.0;

            // Recortar la estaca para que quede dentro
            // de los límites verticales del framebuffer.
            let top_clamped =
                top.max(0.0).min((height - 1) as f32)
                    as usize;

            let bottom_clamped =
                bottom.max(0.0).min((height - 1) as f32)
                    as usize;

            framebuffer.set_current_color(
                cell_color(wall),
            );

            for y in top_clamped..=bottom_clamped {
                framebuffer.point(i, y);
            }
        }
    }
}

fn draw_debug_ray(
    framebuffer: &mut Framebuffer,
    player: &Player,
    angulo: f32,
    distancia: f32,
) {
    framebuffer.set_current_color(0xFFDDDD);

    let mut d = 0.0;

    while d <= distancia {
        let x =
            player.pos.x + d * angulo.cos();

        let y =
            player.pos.y + d * angulo.sin();

        if x >= 0.0 && y >= 0.0 {
            framebuffer.point(
                x as usize,
                y as usize,
            );
        }

        d += 1.0;
    }
}

fn render_top_down(framebuffer: &mut Framebuffer, maze: &Maze, player: &Player) {
    for (row, line) in maze.iter().enumerate() {
        for (col, &cell) in line.iter().enumerate() {
            draw_cell(framebuffer, col * BLOCK_SIZE, row * BLOCK_SIZE, cell);
        }
    }

    framebuffer.set_current_color(0xFFFF00);
    
    let px = player.pos.x as usize;
    let py = player.pos.y as usize;

    for x in px.saturating_sub(3)..=px + 3 {
        for y in py.saturating_sub(3)..=py + 3 {
            framebuffer.point(x, y);
        }
    }

    for i in 0..NUM_RAYS {
    let ray_fraction =
        i as f32 / (NUM_RAYS - 1) as f32;

    let angle =
        player.a - FOV / 2.0
            + FOV * ray_fraction;

    if let Some((distancia, _wall)) =
        cast_ray(
            maze,
            player,
            angle,
            BLOCK_SIZE,
        )
    {
        draw_debug_ray(
            framebuffer,
            player,
            angle,
            distancia,
        );
    }
    }
}

fn main() {
    let window_width = 1300;
    let window_height = 900;
    let framebuffer_width = 1300;
    let framebuffer_height = 900;
    let frame_delay = Duration::from_millis(16);

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

    while window.is_open() && !window.is_key_down(Key::Escape) {
        process_events(&window, &mut player, &maze, BLOCK_SIZE,);

        // ¿el jugador llegó a la meta? Se traduce su posición en píxeles a la
        // celda que ocupa y se revisa si esa celda es la marca `g`.
        let i = player.pos.x as usize / BLOCK_SIZE;
        let j = player.pos.y as usize / BLOCK_SIZE;
        if maze.get(j).and_then(|row| row.get(i)) == Some(&'g') {
            println!("¡Meta alcanzada! Fin del juego.");
            break;
        }

        framebuffer.clear();

        if window.is_key_down(Key::M){
            render_top_down(&mut framebuffer, &maze, &player);
        } else{
            render_3d(&mut framebuffer, &maze, &player);    
        }

        window
            .update_with_buffer(&framebuffer.buffer, framebuffer_width, framebuffer_height)
            .unwrap();

        std::thread::sleep(frame_delay);
    }
}
