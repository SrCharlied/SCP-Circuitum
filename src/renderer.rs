use crate::caster::cast_ray;
use crate::framebuffer::Framebuffer;
use crate::game::VictoryMenuOption;
use crate::maze::Maze;
use crate::player::Player;
use crate::{BLOCK_SIZE, FOV};
use font8x8::{BASIC_FONTS, UnicodeFonts};

const MINIMAP_CELL_SIZE: usize = 8;
const MINIMAP_MARGIN: usize = 20;

/// Cantidad de rayos que se lanzan en abanico para formar el campo de visión.
const NUM_RAYS: usize = 5;

pub fn cell_color(cell: char) -> u32 {
    match cell {
        '+' => 0x00AAFF,       // columnas
        '-' => 0xFF5555,       // paredes horizontales
        '|' => 0xFF5555,       // paredes verticales
        'g' | 'G' => 0x00FF00, // meta
        _ => 0xFFDDDD,         // cualquier otra cosa
    }
}

pub fn render_3d(framebuffer: &mut Framebuffer, maze: &Maze, player: &Player) {
    let width = framebuffer.width;
    let height = framebuffer.height;

    let horizon = height as f32 / 2.0;

    let proyeccion_distancia = (width as f32 / 2.0) / (FOV / 2.0).tan();

    let delta_beta = FOV / (width - 1) as f32;

    for i in 0..width {
        let beta = -FOV / 2.0 + delta_beta * i as f32;

        let ray_angle = player.a + beta;

        if let Some((raw_distance, wall)) = cast_ray(maze, player, ray_angle, BLOCK_SIZE) {
            let distancia_corregida = raw_distance * beta.cos();

            // Evita dividir entre cero si el jugador queda
            // demasiado cerca o dentro de una pared.
            if distancia_corregida <= 0.0 {
                continue;
            }

            let wall_height = (BLOCK_SIZE as f32 / distancia_corregida) * proyeccion_distancia;

            let top = horizon - wall_height / 2.0;

            let bottom = horizon + wall_height / 2.0;

            // Recortar la estaca para que quede dentro
            // de los límites verticales del framebuffer.
            let top_clamped = top.max(0.0).min((height - 1) as f32) as usize;

            let bottom_clamped = bottom.max(0.0).min((height - 1) as f32) as usize;

            // Dibujar el techo.
            framebuffer.set_current_color(0x0000FF);

            for y in 0..top_clamped {
                framebuffer.point(i, y);
            }

            // Dibujar la pared.
            framebuffer.set_current_color(cell_color(wall));

            for y in top_clamped..=bottom_clamped {
                framebuffer.point(i, y);
            }

            // Dibujar el suelo.
            framebuffer.set_current_color(0x292B30);

            for y in bottom_clamped.saturating_add(1)..height {
                framebuffer.point(i, y);
            }
        }
    }
}

fn fill_rect(
    framebuffer: &mut Framebuffer,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: u32,
) {
    framebuffer.set_current_color(color);

    for px in x..x + width {
        for py in y..y + height {
            framebuffer.point(px, py);
        }
    }
}

fn draw_char(
    framebuffer: &mut Framebuffer,
    character: char,
    x: usize,
    y: usize,
    scale: usize,
    color: u32,
) {
    if scale == 0 {
        return;
    }

    let Some(glyph) = BASIC_FONTS.get(character) else {
        return;
    };

    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            let pixel_is_active = (*bits & (1u8 << col)) != 0;

            if pixel_is_active {
                fill_rect(
                    framebuffer,
                    x + col * scale,
                    y + row * scale,
                    scale,
                    scale,
                    color,
                );
            }
        }
    }
}

pub fn draw_text(
    framebuffer: &mut Framebuffer,
    text: &str,
    x: usize,
    y: usize,
    scale: usize,
    color: u32,
) {
    if scale == 0 {
        return;
    }

    let initial_x = x;
    let mut cursor_x = x;
    let mut cursor_y = y;

    for character in text.chars() {
        if character == '\n' {
            cursor_x = initial_x;
            cursor_y += 9 * scale;
            continue;
        }

        draw_char(framebuffer, character, cursor_x, cursor_y, scale, color);

        cursor_x += 9 * scale;
    }
}

fn draw_centered_text(
    framebuffer: &mut Framebuffer,
    text: &str,
    y: usize,
    scale: usize,
    color: u32,
) {
    let text_width = text.chars().count() * 9 * scale;

    let x = framebuffer.width.saturating_sub(text_width) / 2;

    draw_text(framebuffer, text, x, y, scale, color);
}

pub fn render_welcome_screen(framebuffer: &mut Framebuffer) {
    let width = framebuffer.width;
    let height = framebuffer.height;

    fill_rect(framebuffer, 0, 0, width, height, 0x08080D);

    let content_y = height.saturating_sub(500) / 2;

    draw_centered_text(framebuffer, "SCP CIRCUITUM", content_y, 4, 0xFFFFFF);

    draw_centered_text(
        framebuffer,
        "ENCUENTRA LA SALIDA",
        content_y + 100,
        2,
        0xAAAAAA,
    );

    draw_centered_text(framebuffer, "W / S - MOVER", content_y + 190, 2, 0xCCCCCC);

    draw_centered_text(framebuffer, "A / D - GIRAR", content_y + 235, 2, 0xCCCCCC);

    draw_centered_text(
        framebuffer,
        "M - VISTA SUPERIOR",
        content_y + 280,
        2,
        0xCCCCCC,
    );

    draw_centered_text(framebuffer, "ESC - PAUSA", content_y + 325, 2, 0xCCCCCC);

    draw_centered_text(
        framebuffer,
        "ENTER - COMENZAR",
        content_y + 420,
        2,
        0xFFFF00,
    );
}

pub fn render_minimap(framebuffer: &mut Framebuffer, maze: &Maze, player: &Player) {
    if maze.is_empty() {
        return;
    }

    let maze_columns = maze.iter().map(|row| row.len()).max().unwrap_or(0);

    let minimap_width = maze_columns * MINIMAP_CELL_SIZE;

    let minimap_height = maze.len() * MINIMAP_CELL_SIZE;

    // Coloca el minimapa en la esquina superior derecha.
    let offset_x = framebuffer
        .width
        .saturating_sub(minimap_width + MINIMAP_MARGIN);

    let offset_y = MINIMAP_MARGIN;

    // Fondo ligeramente mayor para crear un marco.
    let panel_x = offset_x.saturating_sub(4);
    let panel_y = offset_y.saturating_sub(4);

    fill_rect(
        framebuffer,
        panel_x,
        panel_y,
        minimap_width + 8,
        minimap_height + 8,
        0x111118,
    );

    // Dibujar todas las celdas.
    for (row, line) in maze.iter().enumerate() {
        for (col, &cell) in line.iter().enumerate() {
            let color = match cell {
                ' ' => 0x292933,
                'g' | 'G' => 0x00FF00,
                _ => cell_color(cell),
            };

            let screen_x = offset_x + col * MINIMAP_CELL_SIZE;

            let screen_y = offset_y + row * MINIMAP_CELL_SIZE;

            fill_rect(
                framebuffer,
                screen_x,
                screen_y,
                MINIMAP_CELL_SIZE,
                MINIMAP_CELL_SIZE,
                color,
            );
        }
    }

    // Convertir la posición del jugador del mundo
    // a la escala reducida del minimapa.
    let minimap_scale = MINIMAP_CELL_SIZE as f32 / BLOCK_SIZE as f32;

    let player_x = offset_x + (player.pos.x * minimap_scale) as usize;

    let player_y = offset_y + (player.pos.y * minimap_scale) as usize;

    // Marcador del jugador.
    fill_rect(
        framebuffer,
        player_x.saturating_sub(2),
        player_y.saturating_sub(2),
        5,
        5,
        0xFFFF00,
    );
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

fn draw_debug_ray(framebuffer: &mut Framebuffer, player: &Player, angulo: f32, distancia: f32) {
    framebuffer.set_current_color(0xFFDDDD);

    let mut d = 0.0;

    while d <= distancia {
        let x = player.pos.x + d * angulo.cos();

        let y = player.pos.y + d * angulo.sin();

        if x >= 0.0 && y >= 0.0 {
            framebuffer.point(x as usize, y as usize);
        }

        d += 1.0;
    }
}

pub fn render_top_down(framebuffer: &mut Framebuffer, maze: &Maze, player: &Player) {
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
        let ray_fraction = i as f32 / (NUM_RAYS - 1) as f32;

        let angle = player.a - FOV / 2.0 + FOV * ray_fraction;

        if let Some((distancia, _wall)) = cast_ray(maze, player, angle, BLOCK_SIZE) {
            draw_debug_ray(framebuffer, player, angle, distancia);
        }
    }
}

pub fn render_pause_menu(framebuffer: &mut Framebuffer, target_fps: u32) {
    let panel_width = 520;
    let panel_height = 320;

    let panel_x = framebuffer.width.saturating_sub(panel_width) / 2;

    let panel_y = framebuffer.height.saturating_sub(panel_height) / 2;

    // Borde.
    fill_rect(
        framebuffer,
        panel_x,
        panel_y,
        panel_width,
        panel_height,
        0xFFFFFF,
    );

    // Interior.
    fill_rect(
        framebuffer,
        panel_x + 4,
        panel_y + 4,
        panel_width - 8,
        panel_height - 8,
        0x111118,
    );

    let title = "PAUSA";
    let title_scale = 3;

    let title_width = title.chars().count() * 9 * title_scale;

    let title_x = panel_x + panel_width.saturating_sub(title_width) / 2;

    draw_text(
        framebuffer,
        title,
        title_x,
        panel_y + 35,
        title_scale,
        0xFFFFFF,
    );

    let fps_option = format!("< FPS: {} >", target_fps,);

    let option_scale = 2;

    let option_width = fps_option.chars().count() * 9 * option_scale;

    let option_x = panel_x + panel_width.saturating_sub(option_width) / 2;

    draw_text(
        framebuffer,
        &fps_option,
        option_x,
        panel_y + 120,
        option_scale,
        0xFFFF00,
    );

    let change_hint = "IZQ / DER - CAMBIAR";

    let change_hint_scale = 2;

    let change_hint_width = change_hint.chars().count() * 9 * change_hint_scale;

    let change_hint_x = panel_x + panel_width.saturating_sub(change_hint_width) / 2;

    draw_text(
        framebuffer,
        change_hint,
        change_hint_x,
        panel_y + 180,
        change_hint_scale,
        0xAAAAAA,
    );

    let continue_hint = "ESC - CONTINUAR";

    let continue_hint_scale = 2;

    let continue_hint_width = continue_hint.chars().count() * 9 * continue_hint_scale;

    let continue_hint_x = panel_x + panel_width.saturating_sub(continue_hint_width) / 2;

    draw_text(
        framebuffer,
        continue_hint,
        continue_hint_x,
        panel_y + 245,
        continue_hint_scale,
        0xAAAAAA,
    );
}

pub fn render_victory_screen(framebuffer: &mut Framebuffer, selected_option: VictoryMenuOption) {
    let width = framebuffer.width;
    let height = framebuffer.height;

    fill_rect(framebuffer, 0, 0, width, height, 0x07100A);

    let content_y = height.saturating_sub(420) / 2;

    draw_centered_text(framebuffer, "VICTORIA", content_y, 5, 0x66FF88);

    draw_centered_text(
        framebuffer,
        "HAS ENCONTRADO LA SALIDA",
        content_y + 110,
        2,
        0xFFFFFF,
    );

    draw_centered_text(
        framebuffer,
        "SCP CIRCUITUM COMPLETADO",
        content_y + 165,
        2,
        0xAAAAAA,
    );

    let button_width = 260;
    let button_height = 70;
    let button_gap = 40;

    let buttons_width = button_width * 2 + button_gap;

    let buttons_x = width.saturating_sub(buttons_width) / 2;

    let buttons_y = content_y + 250;

    draw_menu_button(
        framebuffer,
        "VOLVER AL MENU",
        buttons_x,
        buttons_y,
        button_width,
        button_height,
        selected_option == VictoryMenuOption::MainMenu,
    );

    draw_menu_button(
        framebuffer,
        "SALIR",
        buttons_x + button_width + button_gap,
        buttons_y,
        button_width,
        button_height,
        selected_option == VictoryMenuOption::Exit,
    );

    draw_centered_text(
        framebuffer,
        "IZQ / DER - ELEGIR",
        content_y + 350,
        2,
        0xAAAAAA,
    );

    draw_centered_text(
        framebuffer,
        "ENTER - CONFIRMAR",
        content_y + 395,
        2,
        0xFFFF00,
    );
}

fn draw_menu_button(
    framebuffer: &mut Framebuffer,
    label: &str,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    selected: bool,
) {
    let border_color = if selected { 0xFFFF00 } else { 0x777777 };

    let text_color = if selected { 0xFFFF00 } else { 0xAAAAAA };

    fill_rect(framebuffer, x, y, width, height, border_color);

    fill_rect(framebuffer, x + 4, y + 4, width - 8, height - 8, 0x111118);

    let scale = 2;

    let text_width = label.chars().count() * 9 * scale;

    let text_height = 8 * scale;

    let text_x = x + width.saturating_sub(text_width) / 2;

    let text_y = y + height.saturating_sub(text_height) / 2;

    draw_text(framebuffer, label, text_x, text_y, scale, text_color);
}

pub fn render_stamina_bar(
    framebuffer: &mut Framebuffer,
    stamina_ratio: f32,
    sprint_exhausted: bool,
) {
    let ratio = stamina_ratio.clamp(0.0, 1.0);

    let bar_width: usize = 360;
    let bar_height: usize = 28;
    let border_size: usize = 4;
    let bottom_margin: usize = 40;

    let bar_x = framebuffer.width.saturating_sub(bar_width) / 2;

    let bar_y = framebuffer
        .height
        .saturating_sub(bar_height + bottom_margin);

    let border_color = if sprint_exhausted { 0xFF5555 } else { 0xFFFFFF };

    fill_rect(
        framebuffer,
        bar_x,
        bar_y,
        bar_width,
        bar_height,
        border_color,
    );

    let inner_x = bar_x + border_size;

    let inner_y = bar_y + border_size;

    let inner_width = bar_width - border_size * 2;

    let inner_height = bar_height - border_size * 2;

    fill_rect(
        framebuffer,
        inner_x,
        inner_y,
        inner_width,
        inner_height,
        0x18181F,
    );

    let filled_width = (inner_width as f32 * ratio) as usize;

    let fill_color = if sprint_exhausted {
        0xFF5555
    } else if ratio > 0.50 {
        0x55DD77
    } else if ratio > 0.20 {
        0xFFCC33
    } else {
        0xFF5555
    };

    if filled_width > 0 {
        fill_rect(
            framebuffer,
            inner_x,
            inner_y,
            filled_width,
            inner_height,
            fill_color,
        );
    }

    if sprint_exhausted {
        draw_centered_text(
            framebuffer,
            "STAMINA AGOTADA - SUELTA SHIFT",
            bar_y.saturating_sub(35),
            2,
            0xFF7777,
        );
    } else {
        draw_centered_text(
            framebuffer,
            "STAMINA",
            bar_y.saturating_sub(20),
            1,
            0xCCCCCC,
        );
    }
}
