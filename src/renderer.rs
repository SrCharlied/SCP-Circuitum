use crate::caster::{WallSide, cast_ray};
use crate::framebuffer::Framebuffer;
use crate::game::VictoryMenuOption;
use crate::maze::Maze;
use crate::player::Player;
use crate::texture::Texture;
use crate::{BLOCK_SIZE, FOV};
use font8x8::{BASIC_FONTS, UnicodeFonts};

const MINIMAP_CELL_SIZE: usize = 8;
const MINIMAP_MARGIN: usize = 20;

/// Cantidad de rayos que se lanzan en abanico para formar el campo de visión.
const NUM_RAYS: usize = 5;

/// Rayos utilizados para representar la
/// visión del jugador en el minimapa.
const MINIMAP_VISION_RAYS: usize = 128;

/// Radio en píxeles utilizado para cerrar
/// pequeños huecos de rasterización.
const VISIBILITY_MASK_RADIUS: i32 = 1;

/// Alcance de visión expresado en cantidad
/// de celdas del laberinto.
const MINIMAP_VISION_RANGE_CELLS: f32 = 5.0;

pub fn cell_color(cell: char) -> u32 {
    match cell {
        '+' => 0x00AAFF,       // columnas
        '-' => 0xFF5555,       // paredes horizontales
        '|' => 0xFF5555,       // paredes verticales
        'g' | 'G' => 0x00FF00, // meta
        _ => 0xFFDDDD,         // cualquier otra cosa
    }
}

fn darken_color(color: u32) -> u32 {
    let red = ((color >> 16) & 0xFF) * 3 / 4;

    let green = ((color >> 8) & 0xFF) * 3 / 4;

    let blue = (color & 0xFF) * 3 / 4;

    (red << 16) | (green << 8) | blue
}

pub fn render_3d(
    framebuffer: &mut Framebuffer,
    maze: &Maze,
    player: &Player,
    wall_texture: &Texture,
) {
    let width = framebuffer.width;
    let height = framebuffer.height;

    let horizon = height as f32 / 2.0;

    let proyeccion_distancia = (width as f32 / 2.0) / (FOV / 2.0).tan();

    let delta_beta = FOV / (width - 1) as f32;

    for i in 0..width {
        let beta = -FOV / 2.0 + delta_beta * i as f32;

        let ray_angle = player.a + beta;

        if let Some(hit) = cast_ray(maze, player, ray_angle, BLOCK_SIZE) {
            let distancia_corregida = hit.distance * beta.cos();

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

            // Dibujar la pared usando el PNG
            // cargado en memoria.
            for y in top_clamped..=bottom_clamped {
                let texture_v = ((y as f32 - top) / wall_height).clamp(0.0, 0.999_999);

                let texture_color = if matches!(hit.cell, 'g' | 'G') {
                    cell_color(hit.cell)
                } else {
                    wall_texture.sample(hit.texture_u, texture_v)
                };

                let wall_color = match hit.side {
                    WallSide::Vertical => texture_color,

                    WallSide::Horizontal => darken_color(texture_color),
                };

                framebuffer.set_current_color(wall_color);

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

fn mark_visibility_line(
    visibility_mask: &mut [bool],
    mask_width: usize,
    mask_height: usize,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
) {
    let mut x = start_x;
    let mut y = start_y;

    let delta_x = (end_x - start_x).abs();

    let step_x = if start_x < end_x { 1 } else { -1 };

    let delta_y = -(end_y - start_y).abs();

    let step_y = if start_y < end_y { 1 } else { -1 };

    let mut error = delta_x + delta_y;

    loop {
        for offset_y in -VISIBILITY_MASK_RADIUS..=VISIBILITY_MASK_RADIUS {
            for offset_x in -VISIBILITY_MASK_RADIUS..=VISIBILITY_MASK_RADIUS {
                let mask_x = x + offset_x;

                let mask_y = y + offset_y;

                if mask_x >= 0
                    && mask_y >= 0
                    && mask_x < mask_width as i32
                    && mask_y < mask_height as i32
                {
                    let index = mask_y as usize * mask_width + mask_x as usize;

                    visibility_mask[index] = true;
                }
            }
        }

        if x == end_x && y == end_y {
            break;
        }

        let doubled_error = error * 2;

        if doubled_error >= delta_y {
            error += delta_y;
            x += step_x;
        }

        if doubled_error <= delta_x {
            error += delta_x;
            y += step_y;
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

    if maze_columns == 0 {
        return;
    }

    let minimap_width = maze_columns * MINIMAP_CELL_SIZE;

    let minimap_height = maze.len() * MINIMAP_CELL_SIZE;

    let offset_x = framebuffer
        .width
        .saturating_sub(minimap_width + MINIMAP_MARGIN);

    let offset_y = MINIMAP_MARGIN;

    let panel_x = offset_x.saturating_sub(4);

    let panel_y = offset_y.saturating_sub(4);

    // Marco exterior.
    fill_rect(
        framebuffer,
        panel_x,
        panel_y,
        minimap_width + 8,
        minimap_height + 8,
        0x3A3D46,
    );

    // Oscuridad interior.
    fill_rect(
        framebuffer,
        offset_x,
        offset_y,
        minimap_width,
        minimap_height,
        0x050609,
    );

    let minimap_scale = MINIMAP_CELL_SIZE as f32 / BLOCK_SIZE as f32;

    // Coordenadas locales al minimapa.
    let player_local_x = (player.pos.x * minimap_scale) as i32;

    let player_local_y = (player.pos.y * minimap_scale) as i32;

    let mut visibility_mask = vec![false; minimap_width * minimap_height];

    let max_vision_distance = BLOCK_SIZE as f32 * MINIMAP_VISION_RANGE_CELLS;

    for ray_index in 0..MINIMAP_VISION_RAYS {
        let ray_fraction = ray_index as f32 / (MINIMAP_VISION_RAYS - 1) as f32;

        let ray_angle = player.a - FOV / 2.0 + FOV * ray_fraction;

        let ray_hit = cast_ray(maze, player, ray_angle, BLOCK_SIZE);

        let (ray_end_world_x, ray_end_world_y) = match ray_hit {
            Some(hit) if hit.distance <= max_vision_distance => (hit.hit_x, hit.hit_y),

            _ => (
                player.pos.x + max_vision_distance * ray_angle.cos(),
                player.pos.y + max_vision_distance * ray_angle.sin(),
            ),
        };

        let ray_end_local_x = (ray_end_world_x * minimap_scale) as i32;

        let ray_end_local_y = (ray_end_world_y * minimap_scale) as i32;

        mark_visibility_line(
            &mut visibility_mask,
            minimap_width,
            minimap_height,
            player_local_x,
            player_local_y,
            ray_end_local_x,
            ray_end_local_y,
        );
    }

    // Dibujar cada celda únicamente en
    // los píxeles que marca la máscara.
    for (row, line) in maze.iter().enumerate() {
        for (col, &cell) in line.iter().enumerate() {
            let color = match cell {
                ' ' => 0x292933,
                'g' | 'G' => 0x00FF00,
                _ => cell_color(cell),
            };

            framebuffer.set_current_color(color);

            let cell_local_x = col * MINIMAP_CELL_SIZE;

            let cell_local_y = row * MINIMAP_CELL_SIZE;

            for pixel_y in 0..MINIMAP_CELL_SIZE {
                let mask_y = cell_local_y + pixel_y;

                if mask_y >= minimap_height {
                    continue;
                }

                for pixel_x in 0..MINIMAP_CELL_SIZE {
                    let mask_x = cell_local_x + pixel_x;

                    if mask_x >= minimap_width {
                        continue;
                    }

                    let mask_index = mask_y * minimap_width + mask_x;

                    if visibility_mask[mask_index] {
                        framebuffer.point(offset_x + mask_x, offset_y + mask_y);
                    }
                }
            }
        }
    }

    // El jugador siempre se muestra,
    // aunque la máscara tenga algún
    // borde irregular cerca del origen.
    let player_x = offset_x + player_local_x.max(0) as usize;

    let player_y = offset_y + player_local_y.max(0) as usize;

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

        if let Some(hit) = cast_ray(maze, player, angle, BLOCK_SIZE) {
            draw_debug_ray(framebuffer, player, angle, hit.distance);
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

pub fn render_level_transition(
    framebuffer: &mut Framebuffer,
    next_level_number: usize,
    progress: f32,
) {
    let progress = progress.clamp(0.0, 1.0);

    let width = framebuffer.width;
    let height = framebuffer.height;

    // Parpadeo determinista basado en el
    // progreso, no en el número de frames.
    let flicker_tick = (progress * 36.0) as usize;

    let old_light_on = progress < 0.50 && flicker_tick % 7 != 5 && flicker_tick % 7 != 6;

    // Hay un pequeño periodo oscuro entre
    // la lámpara vieja y la luz roja.
    let red_emergency = progress >= 0.60;

    let cabin_color = if red_emergency {
        0x26090C
    } else if old_light_on {
        0x202329
    } else {
        0x08090C
    };

    let door_color = if red_emergency {
        0x3A1014
    } else if old_light_on {
        0x343942
    } else {
        0x111318
    };

    let door_detail_color = if red_emergency { 0x641D22 } else { 0x555C68 };

    // Interior completo de la cabina.
    fill_rect(framebuffer, 0, 0, width, height, cabin_color);

    // Aura superior de la iluminación.
    let upper_light_color = if red_emergency {
        0x42080D
    } else if old_light_on {
        0x303138
    } else {
        0x050609
    };

    fill_rect(framebuffer, 0, 0, width, 115, upper_light_color);

    // Dimensiones del conjunto de puertas.
    let doors_width = width / 2;

    let doors_x = width.saturating_sub(doors_width) / 2;

    let doors_y: usize = 120;

    let doors_height = height.saturating_sub(doors_y + 105);

    let half_door_width = doors_width / 2;

    // Puerta izquierda.
    fill_rect(
        framebuffer,
        doors_x,
        doors_y,
        half_door_width.saturating_sub(2),
        doors_height,
        door_color,
    );

    // Puerta derecha.
    fill_rect(
        framebuffer,
        doors_x + half_door_width + 2,
        doors_y,
        half_door_width.saturating_sub(2),
        doors_height,
        door_color,
    );

    // Marco exterior.
    let frame_size: usize = 18;

    fill_rect(
        framebuffer,
        doors_x.saturating_sub(frame_size),
        doors_y.saturating_sub(frame_size),
        doors_width + frame_size * 2,
        frame_size,
        door_detail_color,
    );

    fill_rect(
        framebuffer,
        doors_x.saturating_sub(frame_size),
        doors_y + doors_height,
        doors_width + frame_size * 2,
        frame_size,
        door_detail_color,
    );

    fill_rect(
        framebuffer,
        doors_x.saturating_sub(frame_size),
        doors_y,
        frame_size,
        doors_height,
        door_detail_color,
    );

    fill_rect(
        framebuffer,
        doors_x + doors_width,
        doors_y,
        frame_size,
        doors_height,
        door_detail_color,
    );

    // Separación central de las puertas.
    fill_rect(
        framebuffer,
        doors_x + half_door_width.saturating_sub(2),
        doors_y,
        4,
        doors_height,
        0x090A0D,
    );

    // Relieves verticales de metal.
    let left_groove_x = doors_x + doors_width / 5;

    let right_groove_x = doors_x + doors_width - doors_width / 5;

    fill_rect(
        framebuffer,
        left_groove_x,
        doors_y + 20,
        5,
        doors_height.saturating_sub(40),
        door_detail_color,
    );

    fill_rect(
        framebuffer,
        right_groove_x,
        doors_y + 20,
        5,
        doors_height.saturating_sub(40),
        door_detail_color,
    );

    // Ventana reforzada en las puertas.
    let window_width = doors_width * 2 / 3;

    let window_height: usize = 260;

    let window_x = doors_x + doors_width.saturating_sub(window_width) / 2;

    let window_y = doors_y + 110;

    let window_frame: usize = 14;

    fill_rect(
        framebuffer,
        window_x.saturating_sub(window_frame),
        window_y.saturating_sub(window_frame),
        window_width + window_frame * 2,
        window_height + window_frame * 2,
        door_detail_color,
    );

    // Interior oscuro del pozo.
    fill_rect(
        framebuffer,
        window_x,
        window_y,
        window_width,
        window_height,
        0x07090D,
    );

    // Rieles vistos a través de la ventana.
    let shaft_rail_margin: usize = 35;
    let shaft_rail_width: usize = 8;

    let left_rail_x = window_x + shaft_rail_margin;

    let right_rail_x = window_x + window_width.saturating_sub(shaft_rail_margin + shaft_rail_width);

    let rail_color = if red_emergency { 0x68242A } else { 0x454C58 };

    fill_rect(
        framebuffer,
        left_rail_x,
        window_y,
        shaft_rail_width,
        window_height,
        rail_color,
    );

    fill_rect(
        framebuffer,
        right_rail_x,
        window_y,
        shaft_rail_width,
        window_height,
        rail_color,
    );

    // Movimiento del pozo hacia arriba.
    let travel_distance = window_height as f32 * 4.0;

    let vertical_offset = (progress * travel_distance) as i32;

    let band_spacing: i32 = 55;

    let vertical_cycle = window_height as i32 + band_spacing;

    let band_count = window_height / band_spacing as usize + 4;

    let beam_x = window_x + shaft_rail_margin;

    let beam_width = window_width.saturating_sub(shaft_rail_margin * 2);

    for band_index in 0..band_count {
        let base_y = band_index as i32 * band_spacing;

        let wrapped_y = (base_y - vertical_offset).rem_euclid(vertical_cycle);

        if wrapped_y >= window_height as i32 {
            continue;
        }

        let beam_y = window_y + wrapped_y as usize;

        let remaining_height = window_y + window_height - beam_y;

        let beam_height = 5_usize.min(remaining_height);

        fill_rect(
            framebuffer,
            beam_x,
            beam_y,
            beam_width,
            beam_height,
            rail_color,
        );

        if band_index % 3 == 0 {
            let light_color = if red_emergency { 0xFF3A42 } else { 0xB7C39B };

            let light_height = 10_usize.min(remaining_height);

            fill_rect(
                framebuffer,
                left_rail_x.saturating_sub(4),
                beam_y,
                16,
                light_height,
                light_color,
            );

            fill_rect(
                framebuffer,
                right_rail_x.saturating_sub(4),
                beam_y,
                16,
                light_height,
                light_color,
            );
        }
    }

    // Reflejos en el vidrio.
    let reflection_color = if red_emergency { 0x5D171D } else { 0x26313A };

    fill_rect(
        framebuffer,
        window_x + 25,
        window_y + 15,
        3,
        window_height.saturating_sub(30),
        reflection_color,
    );

    fill_rect(
        framebuffer,
        window_x + window_width.saturating_sub(32),
        window_y + 35,
        2,
        window_height.saturating_sub(70),
        reflection_color,
    );

    // Lámpara superior.
    let lamp_width: usize = 190;
    let lamp_height: usize = 24;

    let lamp_x = width.saturating_sub(lamp_width) / 2;

    let lamp_y: usize = 48;

    fill_rect(
        framebuffer,
        lamp_x.saturating_sub(8),
        lamp_y.saturating_sub(8),
        lamp_width + 16,
        lamp_height + 16,
        0x111318,
    );

    let lamp_color = if red_emergency {
        0xFF2630
    } else if old_light_on {
        0xD2CFB6
    } else {
        0x242429
    };

    fill_rect(
        framebuffer,
        lamp_x,
        lamp_y,
        lamp_width,
        lamp_height,
        lamp_color,
    );

    // Panel de control a la derecha.
    let panel_width: usize = 135;
    let panel_height: usize = 275;

    let panel_x = doors_x + doors_width + 55;

    let panel_y = doors_y + 120;

    let panel_border_color = if red_emergency { 0x74242A } else { 0x666D78 };

    fill_rect(
        framebuffer,
        panel_x,
        panel_y,
        panel_width,
        panel_height,
        panel_border_color,
    );

    fill_rect(
        framebuffer,
        panel_x + 6,
        panel_y + 6,
        panel_width - 12,
        panel_height - 12,
        0x0A0B0E,
    );

    // Pantalla del panel.
    fill_rect(
        framebuffer,
        panel_x + 18,
        panel_y + 20,
        panel_width - 36,
        80,
        0x020303,
    );

    let indicator_color = if red_emergency { 0xFF3B43 } else { 0xD9E5A9 };

    draw_text(
        framebuffer,
        "V",
        panel_x + 55,
        panel_y + 30,
        3,
        indicator_color,
    );

    let level_text = format!("NIVEL {}", next_level_number,);

    draw_text(
        framebuffer,
        &level_text,
        panel_x + 27,
        panel_y + 72,
        1,
        indicator_color,
    );

    // Botones del panel.
    for button_index in 0..3 {
        let button_y = panel_y + 125 + button_index * 42;

        fill_rect(framebuffer, panel_x + 45, button_y, 44, 26, 0x343840);

        fill_rect(
            framebuffer,
            panel_x + 51,
            button_y + 5,
            32,
            16,
            if red_emergency && button_index == 2 {
                0xFF3038
            } else {
                0x777C83
            },
        );
    }

    // Mensaje inferior.
    draw_centered_text(
        framebuffer,
        "DESCENDIENDO...",
        height.saturating_sub(68),
        2,
        if red_emergency { 0xFF6870 } else { 0xD8D9DC },
    );
}
