mod audio;
mod caster;
mod encounter;
mod framebuffer;
mod game;
mod maze;
mod mouse_capture;
mod player;
mod renderer;
mod scp173;
mod texture;

use minifb::{Key, KeyRepeat, MouseMode, Window, WindowOptions};
use nalgebra_glm::Vec2;
use std::f32::consts::PI;
use std::time::{Duration, Instant};

use crate::audio::AudioManager;
use crate::encounter::{
    EdgeTrigger, EncounterInput, EncounterSession, EncounterUpdate, GameplayGate, GameplayStep,
    SCP_173_ENCOUNTER,
};
use crate::framebuffer::Framebuffer;
use crate::game::{
    GameSession, GameSettings, GameState, LevelSelectionMenu, LevelSuccessOption,
    LevelSuccessOutcome, VictoryMenuOption, confirm_level_success, state_after_reaching_goal,
};
use crate::maze::load_maze;
use crate::mouse_capture::{MouseCapture, should_capture_cursor};
use crate::player::{MouseLook, PlayerMotion, process_events};
use crate::renderer::{
    PlaneTable, WorldSprite, draw_text, render_3d, render_defeat_screen, render_encounter,
    render_level_selection, render_level_success, render_level_transition, render_minimap,
    render_pause_menu, render_sprite, render_stamina_bar, render_victory_screen,
    render_welcome_screen,
};
use crate::scp173::Scp173;
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
        "./assets/textures/floor_industrial.png",
        "./assets/textures/ceiling_industrial.png",
    )
    .unwrap_or_else(|error| panic!("{error}"));

    let mut framebuffer = Framebuffer::new(framebuffer_width, framebuffer_height);
    framebuffer.set_background_color(0x333355);
    let mut depth_buffer = vec![f32::INFINITY; framebuffer_width];

    // Se reconstruye solo si cambian las dimensiones, no por frame.
    let mut plane_table = PlaneTable::new();

    let scp_173_texture = SpriteTexture::from_file("./assets/sprites/scp_173.png")
        .unwrap_or_else(|error| panic!("{error}"));

    let mut scp_173 = Scp173::new(
        Vec2::new(550.0, 150.0),
        120.0,
        scp_173_texture.width() as f32 / scp_173_texture.height() as f32,
    );
    // Si no hay dispositivo de audio el juego continúa en silencio.
    let mut audio = match AudioManager::new() {
        Ok(manager) => Some(manager),

        Err(error) => {
            eprintln!("Audio: {error}. El juego continúa sin sonido.");

            None
        }
    };

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

    // Enter queda bloqueado tras cada confirmación hasta que se
    // suelte la tecla: mantenerla pulsada no debe atravesar varias
    // pantallas seguidas.
    let mut enter_locked = false;

    let mut level_selection_menu = LevelSelectionMenu::new();

    let mut level_success_option = LevelSuccessOption::default();

    let mut mouse_look = MouseLook::new();

    // Su `Drop` libera el cursor pase lo que pase al salir de `main`.
    let mut mouse_capture = MouseCapture::new();

    let mut cursor_hidden = false;

    // Confirmación de la pantalla de derrota, con su propio flanco.
    let mut defeat_confirm = EdgeTrigger::new();

    // Encuentro de demostración: se abre y cierra con F6.
    let mut encounter_session = EncounterSession::new(SCP_173_ENCOUNTER);

    // Impide que el input del encuentro se filtre al gameplay al
    // volver al mundo. Servirá igual para ReturnToWorld.
    let mut gameplay_gate = GameplayGate::new();

    while window.is_open() {
        let frame_start = Instant::now();

        let delta_time = frame_start.duration_since(previous_frame).as_secs_f32();

        previous_frame = frame_start;

        // Enter se resuelve una sola vez por frame. Mientras siga
        // pulsado tras una confirmación no vuelve a valer, así que
        // no encadena pantallas.
        let enter_pressed = if enter_locked {
            if !window.is_key_down(Key::Enter) {
                enter_locked = false;
            }

            false
        } else {
            window.is_key_pressed(Key::Enter, KeyRepeat::No)
        };

        // Arriba y abajo comparten significado con W y S en los menús.
        let menu_next = window.is_key_pressed(Key::S, KeyRepeat::No)
            || window.is_key_pressed(Key::Down, KeyRepeat::No);

        let menu_previous = window.is_key_pressed(Key::W, KeyRepeat::No)
            || window.is_key_pressed(Key::Up, KeyRepeat::No);

        // Estado crudo de las teclas del encuentro. Se lee una sola
        // vez y sirve para tres cosas: sembrar los detectores al
        // abrir, alimentarlos mientras está abierto, y decidir
        // cuándo se puede reanudar el gameplay al cerrar.
        let encounter_input = EncounterInput {
            next_down: window.is_key_down(Key::S) || window.is_key_down(Key::Down),

            previous_down: window.is_key_down(Key::W) || window.is_key_down(Key::Up),

            confirm_down: window.is_key_down(Key::Enter) || window.is_key_down(Key::E),
        };

        match game_state {
            GameState::Welcome => {
                if enter_pressed {
                    game_state = GameState::LevelSelection;

                    level_selection_menu = LevelSelectionMenu::new();

                    enter_locked = true;
                }
            }

            GameState::LevelSelection => {
                if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
                    game_state = GameState::Welcome;
                } else {
                    if menu_next {
                        level_selection_menu.select_next();
                    }

                    if menu_previous {
                        level_selection_menu.select_previous();
                    }

                    if enter_pressed
                        && game_session.select_level(level_selection_menu.selected_index())
                    {
                        let (selected_maze, selected_player) =
                            load_maze(game_session.current_level_path(), BLOCK_SIZE);

                        maze = selected_maze;
                        player = selected_player;
                        scp_173.reset();

                        game_state = GameState::Playing;

                        enter_locked = true;

                        previous_frame = Instant::now();

                        println!("Despliegue en: {}", game_session.current_level_path(),);
                    }
                }
            }

            GameState::LevelSuccess => {
                if menu_next {
                    level_success_option.select_next();
                }

                if menu_previous {
                    level_success_option.select_previous();
                }

                if enter_pressed {
                    enter_locked = true;

                    match confirm_level_success(level_success_option, game_session.has_next_level())
                    {
                        LevelSuccessOutcome::ContinueToNextLevel => {
                            if game_session.advance_level() {
                                game_state = GameState::LevelTransition;

                                level_transition_remaining = LEVEL_TRANSITION_DURATION;

                                println!("Elevador hacia: {}", game_session.current_level_path(),);
                            }
                        }

                        LevelSuccessOutcome::FinishProtocol => {
                            game_state = GameState::Victory;

                            victory_menu_option = VictoryMenuOption::default();

                            println!("¡Último nivel completado! Victoria.");
                        }

                        LevelSuccessOutcome::ReturnToTerminal => {
                            game_state = GameState::LevelSelection;

                            level_selection_menu = LevelSelectionMenu::new();
                        }
                    }
                }
            }

            GameState::Playing => {
                if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
                    game_state.toggle_pause();
                } else if window.is_key_pressed(Key::F6, KeyRepeat::No) {
                    // Apertura provisional del encuentro de demostración.
                    game_state = GameState::Encounter;

                    encounter_session = EncounterSession::new(SCP_173_ENCOUNTER);

                    // Con el estado real del teclado: solo se bloquea
                    // lo que de verdad estaba sostenido.
                    encounter_session.seed_input_state(encounter_input);
                }
            }

            GameState::Defeat => {
                if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
                    // Volver al terminal descarta el encuentro.
                    game_state = GameState::LevelSelection;

                    level_selection_menu = LevelSelectionMenu::new();

                    encounter_session = EncounterSession::new(SCP_173_ENCOUNTER);

                    previous_frame = Instant::now();
                } else if defeat_confirm.update(encounter_input.confirm_down) {
                    // Reintentar el sector: todo vuelve a su estado
                    // inicial y el encuentro se descarta.
                    let (retry_maze, retry_player) =
                        load_maze(game_session.current_level_path(), BLOCK_SIZE);

                    maze = retry_maze;
                    player = retry_player;
                    scp_173.reset();

                    encounter_session = EncounterSession::new(SCP_173_ENCOUNTER);

                    mouse_look.reset();

                    if let Some(audio) = audio.as_mut() {
                        audio.stop_footsteps();
                    }

                    // El input que confirmó el reintento no debe
                    // filtrarse al gameplay.
                    gameplay_gate.arm();

                    enter_locked = true;

                    game_state = GameState::Playing;

                    previous_frame = Instant::now();

                    println!(
                        "Reintento del sector: {}",
                        game_session.current_level_path(),
                    );
                }
            }

            GameState::Encounter => {
                if window.is_key_pressed(Key::F6, KeyRepeat::No)
                    && !encounter_session.is_lethal_locked()
                {
                    game_state = GameState::Playing;

                    // El gameplay no se reanuda aquí: la compuerta lo
                    // retiene hasta que se suelten las teclas.
                    gameplay_gate.arm();
                } else {
                    match encounter_session.update(encounter_input) {
                        EncounterUpdate::ActionTaken(action) => {
                            println!(
                                "Encuentro: {action:?} | turno {} | ataques {} | fase {:?}",
                                encounter_session.turn_count(),
                                encounter_session.attack_count(),
                                encounter_session.phase(),
                            );
                        }

                        EncounterUpdate::PhaseAdvanced(phase) => {
                            println!(
                                "Encuentro: fase {phase:?} | enemigo {:?} | paso {}",
                                encounter_session.enemy_action(),
                                encounter_session.forced_step(),
                            );
                        }

                        EncounterUpdate::PlayerDeath => {
                            game_state = GameState::Defeat;

                            // Una confirmación todavía sostenida no
                            // debe reintentar de inmediato.
                            defeat_confirm.seed(encounter_input.confirm_down);

                            if let Some(audio) = audio.as_mut() {
                                audio.stop_footsteps();
                            }

                            // Descarta el tiempo acumulado.
                            previous_frame = Instant::now();

                            println!("Encuentro: sujeto eliminado.");
                        }

                        _ => {}
                    }
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

                if enter_pressed {
                    match victory_menu_option {
                        VictoryMenuOption::MainMenu => {
                            game_session.reset();

                            let (first_level_maze, first_level_player) =
                                load_maze(game_session.current_level_path(), BLOCK_SIZE);

                            maze = first_level_maze;
                            player = first_level_player;
                            scp_173.reset();

                            game_state = GameState::Welcome;

                            victory_menu_option = VictoryMenuOption::default();

                            enter_locked = true;

                            previous_frame = Instant::now();
                        }

                        VictoryMenuOption::Exit => {
                            break;
                        }
                    }
                }
            }
        }

        // Perder el foco libera el cursor: dejarlo confinado
        // atraparía el puntero fuera del juego tras un Alt+Tab.
        let window_is_active = window.is_active();

        let capture_requested = should_capture_cursor(game_state, window_is_active);

        // El mouse se consulta una sola vez por frame. `Discard`
        // devuelve `None` cuando el cursor sale de la ventana, y eso
        // descarta la referencia en lugar de arrastrar un salto.
        let mouse_x = window.get_mouse_pos(MouseMode::Discard).map(|(x, _)| x);

        let mouse_rotation_delta = if capture_requested {
            let was_capturing = mouse_capture.is_active();

            if mouse_capture.engage(window.get_window_handle()) {
                // Con el cursor confinado la referencia es el centro
                // del área cliente, no la muestra anterior: se mide
                // contra él y después se devuelve el cursor ahí, así
                // que el giro no tiene tope.
                let (window_width, _) = window.get_size();

                let client_center_x = window_width as f32 / 2.0;

                // La primera captura solo centra el cursor; medir
                // contra el centro sin haber centrado antes sería un
                // salto.
                let delta = if was_capturing {
                    mouse_x.map(|x| x - client_center_x).unwrap_or(0.0)
                } else {
                    0.0
                };

                // Recentrar después de leer, nunca antes: si no, el
                // movimiento programático se confundiría con el del
                // jugador.
                mouse_capture.recenter();

                // Este camino no usa la referencia relativa.
                mouse_look.reset();

                delta
            } else {
                // Sin captura —otra plataforma, o Win32 falló— se
                // conserva el comportamiento anterior.
                mouse_look.horizontal_delta(mouse_x)
            }
        } else {
            mouse_capture.release();

            mouse_look.reset();

            0.0
        };

        // Un solo punto decide la visibilidad del cursor, y solo se
        // llama al sistema cuando el valor realmente cambia. Se usa
        // `set_cursor_visibility` de minifb y nunca `ShowCursor` de
        // Win32, para no desbalancear su contador interno.
        let should_hide_cursor = capture_requested || game_state == GameState::LevelTransition;

        if should_hide_cursor != cursor_hidden {
            window.set_cursor_visibility(!should_hide_cursor);

            cursor_hidden = should_hide_cursor;
        }

        // Desplazamiento real del frame, ya resuelto por las
        // colisiones. Fuera de Playing el jugador no se mueve.
        let mut player_motion = PlayerMotion::Still;

        // La compuerta solo avanza en Playing: en cualquier otro
        // estado sigue esperando y no consume la soltada de teclas.
        let gameplay_step = if game_state == GameState::Playing {
            gameplay_gate.update(encounter_input)
        } else {
            GameplayStep::Blocked
        };

        if gameplay_step == GameplayStep::Released {
            // Reloj fresco: el primer frame de gameplay no arrastra
            // el tiempo transcurrido durante el encuentro.
            previous_frame = Instant::now();
        }

        if gameplay_step == GameplayStep::Running {
            player_motion = process_events(
                &window,
                &mut player,
                &maze,
                BLOCK_SIZE,
                delta_time,
                mouse_rotation_delta,
            );

            if game_session.current_level_number() == 1 {
                let scp_173_observed = scp_173.is_observed(&maze, &player, BLOCK_SIZE, FOV);

                scp_173.update(&maze, &player, BLOCK_SIZE, scp_173_observed, delta_time);
            }

            let map_x = player.pos.x as usize / BLOCK_SIZE;

            let map_y = player.pos.y as usize / BLOCK_SIZE;

            let current_cell = maze.get(map_y).and_then(|row| row.get(map_x)).copied();

            if matches!(current_cell, Some('g' | 'G')) {
                // Pisar la meta abre el informe; no avanza de nivel
                // por su cuenta. El avance lo confirma el jugador.
                game_state = state_after_reaching_goal();

                level_success_option = LevelSuccessOption::default();

                // Evita que un Enter que venga de antes confirme el
                // informe en el mismo instante en que aparece.
                enter_locked = true;

                println!("Sector completado: {}", game_session.current_level_path(),);
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

        // Punto único desde el que se toca el audio: la acción se
        // deriva del estado ya asentado en este frame, así que
        // llamarlo cada frame nunca reinicia ni duplica la pista.
        if let Some(audio) = audio.as_mut() {
            audio.update_for_state(game_state);

            audio.update_footsteps(game_state, player_motion, delta_time);
        }

        framebuffer.clear();

        match game_state {
            GameState::Welcome => {
                render_welcome_screen(&mut framebuffer);
            }

            GameState::LevelSelection => {
                render_level_selection(&mut framebuffer, level_selection_menu.selected_index());
            }

            GameState::LevelSuccess => {
                render_level_success(
                    &mut framebuffer,
                    game_session.current_level_number(),
                    game_session.current_level_info().sector,
                    level_success_option,
                    game_session.has_next_level(),
                );
            }

            GameState::Defeat => {
                render_defeat_screen(&mut framebuffer);
            }

            GameState::Victory => {
                render_victory_screen(&mut framebuffer, victory_menu_option);
            }

            GameState::Playing | GameState::Paused | GameState::Encounter => {
                let render_3d_start = Instant::now();

                render_3d(
                    &mut framebuffer,
                    &mut plane_table,
                    &mut depth_buffer,
                    &maze,
                    &player,
                    &textures,
                    game_session.current_level_number(),
                );

                if game_session.current_level_number() == 1 {
                    let scp_173_sprite = WorldSprite {
                        x: scp_173.pos.x,
                        y: scp_173.pos.y,
                        size: scp_173.height,
                    };

                    render_sprite(
                        &mut framebuffer,
                        &depth_buffer,
                        &player,
                        &scp_173_sprite,
                        &scp_173_texture,
                    );
                }

                let render_3d_elapsed = render_3d_start.elapsed();

                render_3d_total += render_3d_elapsed;

                render_3d_max = render_3d_max.max(render_3d_elapsed);

                render_3d_samples += 1;

                render_minimap(&mut framebuffer, &maze, &player);

                if settings.show_fps {
                    draw_text(&mut framebuffer, &fps_text, 20, 20, 2, 0xFFFFFF);
                    draw_text(&mut framebuffer, &render_3d_text, 20, 40, 2, 0xFFFFFF);
                }

                if game_session.current_level_number() == 1 {
                    let scp_173_observed = scp_173.is_observed(&maze, &player, BLOCK_SIZE, FOV);

                    let (observation_text, observation_color) = if scp_173_observed {
                        ("SCP-173: OBSERVADO", 0x55DD77)
                    } else {
                        ("SCP-173: NO OBSERVADO", 0xFF5555)
                    };

                    draw_text(
                        &mut framebuffer,
                        observation_text,
                        20,
                        64,
                        2,
                        observation_color,
                    );
                }

                render_stamina_bar(
                    &mut framebuffer,
                    player.stamina_ratio(),
                    player.is_sprint_exhausted(),
                );

                if game_state == GameState::Paused {
                    render_pause_menu(&mut framebuffer, settings.target_fps());
                }

                // La escena queda congelada detrás del panel: se
                // dibuja igual que en Playing y luego se atenúa.
                if game_state == GameState::Encounter {
                    render_encounter(
                        &mut framebuffer,
                        encounter_session.entity_name(),
                        encounter_session.current_text(),
                        encounter_session.actions_title(),
                        encounter_session.choices(),
                        encounter_session.selected_index(),
                    );
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
