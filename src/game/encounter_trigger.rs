//! Disparo automático de encuentros por proximidad.
//!
//! Vive aparte del encuentro en sí: aquí solo se decide **cuándo**
//! empieza, no qué ocurre dentro.

use nalgebra_glm::Vec2;

use crate::caster::cast_ray;
use crate::maze::Maze;
use crate::player::Player;

/// Margen en unidades de mundo al comparar el impacto del rayo con
/// la distancia al objetivo. Mismo criterio que usa `scp173`.
const LINE_OF_SIGHT_MARGIN: f32 = 1.0;

/// Por qué se abrió el encuentro actual.
///
/// Distinguirlo importa porque F6 es una herramienta de depuración:
/// puede retirar lo que él mismo abrió, nunca un encuentro que el
/// juego provocó.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterOrigin {
    Proximity,
    Debug,
}

/// Si F6 puede cerrar el encuentro en curso.
///
/// Solo los encuentros que abrió el propio F6, y solo mientras la
/// muerte no esté comprometida.
pub fn can_close_debug_encounter(origin: Option<EncounterOrigin>, lethal_locked: bool) -> bool {
    origin == Some(EncounterOrigin::Debug) && !lethal_locked
}

/// SCP-173 solo existe en el primer sector, así que su encuentro
/// tampoco puede dispararse en los demás.
pub fn scp_173_trigger_enabled(level_number: usize) -> bool {
    level_number == 1
}

/// Dispara una sola vez cuando el jugador se acerca lo suficiente a
/// un objetivo y hay línea física libre entre ambos.
///
/// No mira hacia dónde apunta la cámara: el encuentro debe poder
/// empezar aunque la entidad se aproxime por detrás.
pub struct ProximityEncounterTrigger {
    radius_cells: f32,
    fired: bool,
}

impl ProximityEncounterTrigger {
    /// # Pánico
    ///
    /// Si el radio no es un número finito mayor que cero. Es un
    /// valor de configuración del programador, así que conviene
    /// detectarlo al arrancar y no con un trigger que nunca dispara.
    pub fn new(radius_cells: f32) -> Self {
        assert!(
            radius_cells.is_finite() && radius_cells > 0.0,
            "El radio del trigger debe ser finito y mayor que cero, y llegó {radius_cells}",
        );

        Self {
            radius_cells,
            fired: false,
        }
    }

    pub fn has_fired(&self) -> bool {
        self.fired
    }

    /// Vuelve a armar el trigger para un intento nuevo.
    pub fn reset(&mut self) {
        self.fired = false;
    }

    /// Evalúa el frame actual. Devuelve `true` solo en el momento en
    /// que el encuentro debe empezar.
    ///
    /// Los datos inválidos —`block_size` cero, coordenadas no
    /// finitas— devuelven `false` sin consumir el disparo y sin
    /// provocar un pánico.
    pub fn update(
        &mut self,
        maze: &Maze,
        player: &Player,
        target: Vec2,
        block_size: usize,
    ) -> bool {
        if self.fired || block_size == 0 {
            return false;
        }

        if !player.pos.x.is_finite()
            || !player.pos.y.is_finite()
            || !target.x.is_finite()
            || !target.y.is_finite()
        {
            return false;
        }

        let offset_x = target.x - player.pos.x;

        let offset_y = target.y - player.pos.y;

        let distance = offset_x.hypot(offset_y);

        if !distance.is_finite() {
            return false;
        }

        // El radio se configura en celdas para que no dependa de
        // `BLOCK_SIZE`; aquí se lleva a unidades de mundo.
        if distance > self.radius_cells * block_size as f32 {
            return false;
        }

        if !has_line_of_sight(maze, player, offset_x, offset_y, distance, block_size) {
            return false;
        }

        // Una pared o una distancia insuficiente no gastan el
        // disparo: solo lo consume un encuentro real.
        self.fired = true;

        true
    }
}

/// Línea física libre entre el jugador y el objetivo.
///
/// Reutiliza el DDA del raycaster en lugar de duplicarlo: se lanza
/// un rayo hacia el objetivo y se compara la distancia del impacto.
fn has_line_of_sight(
    maze: &Maze,
    player: &Player,
    offset_x: f32,
    offset_y: f32,
    distance: f32,
    block_size: usize,
) -> bool {
    // Ambos en el mismo punto: no cabe una pared en medio.
    if distance <= f32::EPSILON {
        return true;
    }

    let angle = offset_y.atan2(offset_x);

    match cast_ray(maze, player, angle, block_size) {
        Some(hit) => hit.distance + LINE_OF_SIGHT_MARGIN >= distance,

        // El rayo salió del mapa sin chocar: nada se interpone.
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EncounterOrigin, ProximityEncounterTrigger, can_close_debug_encounter,
        scp_173_trigger_enabled,
    };
    use crate::maze::Maze;
    use crate::player::Player;
    use nalgebra_glm::Vec2;

    const BLOCK_SIZE: usize = 100;
    const RADIUS_CELLS: f32 = 1.25;

    /// Sala de 7x7 con borde sólido y el interior despejado.
    fn open_room() -> Maze {
        let mut maze: Maze = vec![vec![' '; 7]; 7];

        for cell in maze[0].iter_mut() {
            *cell = '|';
        }

        for cell in maze[6].iter_mut() {
            *cell = '|';
        }

        for row in maze.iter_mut() {
            row[0] = '|';
            row[6] = '|';
        }

        maze
    }

    /// Jugador en el centro de la sala, mirando al este.
    fn centered_player() -> Player {
        Player::new(Vec2::new(350.0, 350.0), 0.0)
    }

    fn trigger() -> ProximityEncounterTrigger {
        ProximityEncounterTrigger::new(RADIUS_CELLS)
    }

    // ----- Configuración -----

    #[test]
    #[should_panic(expected = "finito y mayor que cero")]
    fn a_zero_radius_fails_clearly() {
        let _ = ProximityEncounterTrigger::new(0.0);
    }

    #[test]
    #[should_panic(expected = "finito y mayor que cero")]
    fn a_negative_radius_fails_clearly() {
        let _ = ProximityEncounterTrigger::new(-2.0);
    }

    #[test]
    #[should_panic(expected = "finito y mayor que cero")]
    fn a_nan_radius_fails_clearly() {
        let _ = ProximityEncounterTrigger::new(f32::NAN);
    }

    #[test]
    #[should_panic(expected = "finito y mayor que cero")]
    fn an_infinite_radius_fails_clearly() {
        let _ = ProximityEncounterTrigger::new(f32::INFINITY);
    }

    // ----- Radio -----

    #[test]
    fn a_target_beyond_the_radius_does_not_fire() {
        let maze = open_room();
        let player = centered_player();
        let mut trigger = trigger();

        // 1.30 celdas: apenas fuera del radio de 1.25.
        let target = Vec2::new(350.0 + 130.0, 350.0);

        assert!(!trigger.update(&maze, &player, target, BLOCK_SIZE));

        assert!(!trigger.has_fired());
    }

    #[test]
    fn a_target_exactly_at_the_radius_fires() {
        let maze = open_room();
        let player = centered_player();
        let mut trigger = trigger();

        // 1.25 celdas exactas: 125 unidades de mundo.
        let target = Vec2::new(350.0 + 125.0, 350.0);

        assert!(trigger.update(&maze, &player, target, BLOCK_SIZE));

        assert!(trigger.has_fired());
    }

    #[test]
    fn a_close_target_with_a_clear_line_fires() {
        let maze = open_room();
        let player = centered_player();
        let mut trigger = trigger();

        let target = Vec2::new(430.0, 350.0);

        assert!(trigger.update(&maze, &player, target, BLOCK_SIZE));
    }

    // ----- Paredes -----

    #[test]
    fn a_wall_between_them_blocks_the_trigger() {
        let mut maze = open_room();

        // Pared en la celda contigua hacia el este.
        maze[3][4] = '|';

        let player = centered_player();
        let mut trigger = trigger();

        let target = Vec2::new(450.0, 350.0);

        assert!(!trigger.update(&maze, &player, target, BLOCK_SIZE));

        assert!(!trigger.has_fired());
    }

    #[test]
    fn a_blocked_attempt_does_not_consume_the_trigger() {
        let mut maze = open_room();

        maze[3][4] = '|';

        let player = centered_player();
        let mut trigger = trigger();

        // Bloqueado hacia el este.
        assert!(!trigger.update(&maze, &player, Vec2::new(450.0, 350.0), BLOCK_SIZE));

        assert!(!trigger.has_fired());

        // El mismo trigger dispara al aparecer en una línea libre.
        assert!(trigger.update(&maze, &player, Vec2::new(350.0, 450.0), BLOCK_SIZE));

        assert!(trigger.has_fired());
    }

    #[test]
    fn a_distant_but_visible_target_does_not_fire() {
        let maze = open_room();
        let player = centered_player();
        let mut trigger = trigger();

        // Tres celdas al este, sin nada en medio.
        let target = Vec2::new(650.0, 350.0);

        assert!(!trigger.update(&maze, &player, target, BLOCK_SIZE));
    }

    // ----- Un solo disparo -----

    #[test]
    fn the_trigger_only_fires_once() {
        let maze = open_room();
        let player = centered_player();
        let mut trigger = trigger();

        let target = Vec2::new(430.0, 350.0);

        assert!(trigger.update(&maze, &player, target, BLOCK_SIZE));

        for _ in 0..30 {
            assert!(!trigger.update(&maze, &player, target, BLOCK_SIZE));
        }
    }

    #[test]
    fn resetting_arms_the_trigger_again() {
        let maze = open_room();
        let player = centered_player();
        let mut trigger = trigger();

        let target = Vec2::new(430.0, 350.0);

        assert!(trigger.update(&maze, &player, target, BLOCK_SIZE));

        assert!(!trigger.update(&maze, &player, target, BLOCK_SIZE));

        trigger.reset();

        assert!(!trigger.has_fired());

        assert!(trigger.update(&maze, &player, target, BLOCK_SIZE));
    }

    // ----- Datos inválidos -----

    #[test]
    fn a_zero_block_size_does_not_fire() {
        let maze = open_room();
        let player = centered_player();
        let mut trigger = trigger();

        assert!(!trigger.update(&maze, &player, Vec2::new(360.0, 350.0), 0));

        assert!(!trigger.has_fired());
    }

    #[test]
    fn non_finite_coordinates_do_not_fire() {
        let maze = open_room();
        let mut trigger = trigger();

        for target in [
            Vec2::new(f32::NAN, 350.0),
            Vec2::new(350.0, f32::NAN),
            Vec2::new(f32::INFINITY, 350.0),
            Vec2::new(350.0, f32::NEG_INFINITY),
        ] {
            assert!(!trigger.update(&open_room(), &centered_player(), target, BLOCK_SIZE));
        }

        // Y tampoco si el que está roto es el jugador.
        for position in [Vec2::new(f32::NAN, 350.0), Vec2::new(350.0, f32::INFINITY)] {
            let broken = Player::new(position, 0.0);

            assert!(!trigger.update(&maze, &broken, Vec2::new(360.0, 350.0), BLOCK_SIZE));
        }

        assert!(!trigger.has_fired());
    }

    // ----- Independencia del ángulo -----

    #[test]
    fn the_trigger_ignores_where_the_camera_points() {
        let maze = open_room();

        let target = Vec2::new(430.0, 350.0);

        // El objetivo está al este; se prueban ocho orientaciones.
        for step in 0..8 {
            let angle = step as f32 * std::f32::consts::FRAC_PI_4;

            let player = Player::new(Vec2::new(350.0, 350.0), angle);

            let mut trigger = trigger();

            assert!(
                trigger.update(&maze, &player, target, BLOCK_SIZE),
                "el trigger dependió del ángulo {angle}",
            );
        }
    }

    #[test]
    fn a_target_behind_the_player_fires() {
        let maze = open_room();

        // Mirando al este, con el objetivo al oeste.
        let player = Player::new(Vec2::new(350.0, 350.0), 0.0);

        let mut trigger = trigger();

        let target = Vec2::new(250.0, 350.0);

        assert!(trigger.update(&maze, &player, target, BLOCK_SIZE));
    }

    // ----- Situación real del nivel 1 -----

    #[test]
    fn the_real_spawns_are_safely_separated() {
        let maze = crate::maze::load_maze("./levels/level_01.txt", BLOCK_SIZE).0;

        // Posiciones reales de arranque. Lo que importa no es la
        // distancia exacta, sino que el encuentro no se dispare al
        // desplegar y que el trigger siga pendiente.
        let player = Player::new(Vec2::new(150.0, 150.0), 0.0);

        let scp_position = Vec2::new(750.0, 750.0);

        let distance = (scp_position - player.pos).norm();

        assert!(
            distance > RADIUS_CELLS * BLOCK_SIZE as f32,
            "el encuentro no debe dispararse al cargar",
        );

        let mut trigger = trigger();

        assert!(
            !trigger.update(&maze, &player, scp_position, BLOCK_SIZE),
            "el encuentro no debe dispararse al cargar el nivel",
        );

        assert!(!trigger.has_fired());
    }

    // ----- Nivel -----

    #[test]
    fn the_trigger_only_applies_to_the_first_level() {
        assert!(scp_173_trigger_enabled(1));

        for level in [0, 2, 3, 99] {
            assert!(
                !scp_173_trigger_enabled(level),
                "el nivel {level} no debe abrir el encuentro de SCP-173",
            );
        }
    }

    // ----- Origen del encuentro -----

    #[test]
    fn only_a_debug_encounter_can_be_closed_with_f6() {
        assert!(can_close_debug_encounter(
            Some(EncounterOrigin::Debug),
            false
        ));

        // Ni siquiera el de depuración una vez comprometida la muerte.
        assert!(!can_close_debug_encounter(
            Some(EncounterOrigin::Debug),
            true
        ));

        // Los automáticos nunca.
        assert!(!can_close_debug_encounter(
            Some(EncounterOrigin::Proximity),
            false
        ));

        assert!(!can_close_debug_encounter(
            Some(EncounterOrigin::Proximity),
            true
        ));

        // Y sin encuentro no hay nada que cerrar.
        assert!(!can_close_debug_encounter(None, false));

        assert!(!can_close_debug_encounter(None, true));
    }
}
