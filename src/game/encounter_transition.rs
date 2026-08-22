//! Transición visual al entrar en un encuentro.

use crate::game::GameState;

/// Si este frame entra en un encuentro.
///
/// Compara el estado anterior con el actual, así que detecta la
/// entrada una sola vez y da igual cómo se abrió: por proximidad o
/// con F6. Audio y presentación comparten esta decisión para que el
/// sonido y el destello arranquen en el mismo frame.
pub fn entered_encounter(previous_state: Option<GameState>, current_state: GameState) -> bool {
    current_state == GameState::Encounter && previous_state != Some(GameState::Encounter)
}

/// Tope de avance por frame.
///
/// Un frame largo no puede consumir el destello entero: siempre
/// quedan varios frames de transición visible.
const MAX_TRANSITION_DELTA: f32 = 0.10;

/// Qué hizo la transición en el frame actual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterTransitionUpdate {
    Idle,
    Running,
    Finished,
}

/// Destello blanco que cubre la pantalla al empezar un encuentro.
///
/// No guarda ningún `Instant`: avanza solo con `delta_time`, así que
/// se puede probar frame a frame sin reloj real.
pub struct EncounterTransition {
    duration: f32,
    solid_duration: f32,
    remaining: f32,
    active: bool,
}

impl EncounterTransition {
    /// # Pánico
    ///
    /// Si la configuración no es utilizable. Son constantes del
    /// programador, así que conviene detectarlo al arrancar y no con
    /// un destello que nunca se ve.
    pub fn new(duration: f32, solid_duration: f32) -> Self {
        assert!(
            duration.is_finite() && duration > 0.0,
            "La duración del destello debe ser finita y mayor que cero, y llegó {duration}",
        );

        assert!(
            solid_duration.is_finite() && solid_duration >= 0.0 && solid_duration <= duration,
            "El blanco total debe durar entre cero y {duration} segundos, y llegó {solid_duration}",
        );

        Self {
            duration,
            solid_duration,
            remaining: 0.0,
            active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn remaining(&self) -> f32 {
        self.remaining
    }

    /// Arranca el destello desde blanco total. Llamarla durante otra
    /// transición la reinicia.
    pub fn start(&mut self) {
        self.active = true;

        self.remaining = self.duration;
    }

    /// Retira el destello de inmediato.
    pub fn cancel(&mut self) {
        self.active = false;

        self.remaining = 0.0;
    }

    /// Cuánto blanco se mezcla sobre la imagen, de 1 a 0.
    pub fn intensity(&self) -> f32 {
        if !self.active {
            return 0.0;
        }

        let elapsed = self.duration - self.remaining;

        if elapsed <= self.solid_duration {
            return 1.0;
        }

        let fade_span = self.duration - self.solid_duration;

        // Sin margen de desvanecimiento el destello se mantiene
        // blanco hasta que termina, en vez de dividir entre cero.
        if fade_span <= 0.0 {
            return 1.0;
        }

        let intensity = 1.0 - (elapsed - self.solid_duration) / fade_span;

        if intensity.is_finite() {
            intensity.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Avanza el destello un frame.
    ///
    /// Un `delta_time` roto —negativo o no finito— no mueve el
    /// temporizador, pero la transición sigue activa.
    pub fn update(&mut self, delta_time: f32) -> EncounterTransitionUpdate {
        if !self.active {
            return EncounterTransitionUpdate::Idle;
        }

        if !delta_time.is_finite() || delta_time < 0.0 {
            return EncounterTransitionUpdate::Running;
        }

        self.remaining -= delta_time.min(MAX_TRANSITION_DELTA);

        if self.remaining <= 0.0 {
            self.remaining = 0.0;

            self.active = false;

            return EncounterTransitionUpdate::Finished;
        }

        EncounterTransitionUpdate::Running
    }
}

#[cfg(test)]
mod tests {
    use super::{EncounterTransition, EncounterTransitionUpdate, entered_encounter};
    use crate::game::GameState;

    const DURATION: f32 = 1.0;
    const SOLID: f32 = 0.15;
    const FRAME: f32 = 1.0 / 60.0;

    fn transition() -> EncounterTransition {
        EncounterTransition::new(DURATION, SOLID)
    }

    /// Transición ya arrancada.
    fn started() -> EncounterTransition {
        let mut transition = transition();

        transition.start();

        transition
    }

    // ----- Detector de entrada -----

    #[test]
    fn entering_from_playing_is_detected() {
        assert!(entered_encounter(
            Some(GameState::Playing),
            GameState::Encounter
        ));
    }

    #[test]
    fn entering_without_a_previous_state_is_detected() {
        assert!(entered_encounter(None, GameState::Encounter));
    }

    #[test]
    fn entering_from_paused_is_detected() {
        assert!(entered_encounter(
            Some(GameState::Paused),
            GameState::Encounter
        ));
    }

    #[test]
    fn staying_inside_the_encounter_is_not_an_entry() {
        assert!(!entered_encounter(
            Some(GameState::Encounter),
            GameState::Encounter
        ));
    }

    #[test]
    fn leaving_the_encounter_is_not_an_entry() {
        assert!(!entered_encounter(
            Some(GameState::Encounter),
            GameState::Playing
        ));
    }

    #[test]
    fn staying_outside_is_not_an_entry() {
        assert!(!entered_encounter(
            Some(GameState::Playing),
            GameState::Playing
        ));
    }

    #[test]
    fn leaving_and_coming_back_is_detected_again() {
        let sequence = [
            (Some(GameState::Playing), GameState::Encounter, true),
            (Some(GameState::Encounter), GameState::Encounter, false),
            (Some(GameState::Encounter), GameState::Playing, false),
            (Some(GameState::Playing), GameState::Encounter, true),
        ];

        for (previous, current, expected) in sequence {
            assert_eq!(
                entered_encounter(previous, current),
                expected,
                "falló la transición {previous:?} -> {current:?}",
            );
        }
    }

    // ----- Configuración -----

    #[test]
    #[should_panic(expected = "duración del destello")]
    fn a_zero_duration_fails_clearly() {
        let _ = EncounterTransition::new(0.0, 0.0);
    }

    #[test]
    #[should_panic(expected = "duración del destello")]
    fn a_negative_duration_fails_clearly() {
        let _ = EncounterTransition::new(-1.0, 0.0);
    }

    #[test]
    #[should_panic(expected = "duración del destello")]
    fn a_nan_duration_fails_clearly() {
        let _ = EncounterTransition::new(f32::NAN, 0.0);
    }

    #[test]
    #[should_panic(expected = "blanco total")]
    fn a_negative_hold_fails_clearly() {
        let _ = EncounterTransition::new(DURATION, -0.1);
    }

    #[test]
    #[should_panic(expected = "blanco total")]
    fn an_infinite_hold_fails_clearly() {
        let _ = EncounterTransition::new(DURATION, f32::INFINITY);
    }

    #[test]
    #[should_panic(expected = "blanco total")]
    fn a_hold_longer_than_the_flash_fails_clearly() {
        let _ = EncounterTransition::new(DURATION, DURATION + 0.01);
    }

    // ----- Estado inicial -----

    #[test]
    fn a_new_transition_is_inactive_and_invisible() {
        let transition = transition();

        assert!(!transition.is_active());

        assert_eq!(transition.remaining(), 0.0);

        assert_eq!(transition.intensity(), 0.0);
    }

    #[test]
    fn an_inactive_transition_stays_idle() {
        let mut transition = transition();

        for _ in 0..30 {
            assert_eq!(transition.update(FRAME), EncounterTransitionUpdate::Idle);

            assert_eq!(transition.intensity(), 0.0);
        }
    }

    // ----- Arranque y blanco total -----

    #[test]
    fn starting_turns_the_screen_fully_white() {
        let transition = started();

        assert!(transition.is_active());

        assert_eq!(transition.intensity(), 1.0);

        assert_eq!(transition.remaining(), DURATION);
    }

    #[test]
    fn the_screen_stays_white_during_the_hold() {
        let mut transition = started();

        let hold_frames = (SOLID / FRAME).floor() as usize;

        for frame in 0..hold_frames {
            transition.update(FRAME);

            assert_eq!(
                transition.intensity(),
                1.0,
                "el blanco se rompió en el frame {frame}",
            );
        }
    }

    #[test]
    fn the_flash_fades_after_the_hold() {
        let mut transition = started();

        // Se pasa del tiempo de blanco total.
        let hold_frames = (SOLID / FRAME).ceil() as usize + 1;

        for _ in 0..hold_frames {
            transition.update(FRAME);
        }

        assert!(
            transition.intensity() < 1.0,
            "el destello no empezó a desvanecerse: {}",
            transition.intensity(),
        );
    }

    #[test]
    fn the_intensity_never_grows_while_it_runs() {
        let mut transition = started();

        let mut previous = transition.intensity();

        loop {
            let update = transition.update(FRAME);

            let current = transition.intensity();

            assert!(
                current <= previous,
                "la intensidad subió de {previous} a {current}",
            );

            previous = current;

            if update == EncounterTransitionUpdate::Finished {
                break;
            }
        }
    }

    #[test]
    fn the_flash_ends_at_exactly_zero() {
        let mut transition = started();

        while transition.update(FRAME) != EncounterTransitionUpdate::Finished {}

        assert_eq!(transition.intensity(), 0.0);

        assert_eq!(transition.remaining(), 0.0);

        assert!(!transition.is_active());
    }

    #[test]
    fn finishing_is_reported_only_once() {
        let mut transition = started();

        let mut finishes = 0;

        for _ in 0..300 {
            if transition.update(FRAME) == EncounterTransitionUpdate::Finished {
                finishes += 1;
            }
        }

        assert_eq!(finishes, 1);
    }

    #[test]
    fn the_transition_goes_idle_after_finishing() {
        let mut transition = started();

        while transition.update(FRAME) != EncounterTransitionUpdate::Finished {}

        assert_eq!(transition.update(FRAME), EncounterTransitionUpdate::Idle);
    }

    #[test]
    fn the_flash_lasts_about_its_configured_duration() {
        let mut transition = started();

        let mut frames = 0;

        while transition.update(FRAME) != EncounterTransitionUpdate::Finished {
            frames += 1;
        }

        let seconds = (frames + 1) as f32 * FRAME;

        assert!(
            (seconds - DURATION).abs() < 0.05,
            "el destello duró {seconds} s en lugar de {DURATION}",
        );
    }

    // ----- Cancelar y reiniciar -----

    #[test]
    fn cancelling_stops_the_flash_immediately() {
        let mut transition = started();

        transition.update(FRAME);

        transition.cancel();

        assert!(!transition.is_active());

        assert_eq!(transition.intensity(), 0.0);

        assert_eq!(transition.remaining(), 0.0);

        assert_eq!(transition.update(FRAME), EncounterTransitionUpdate::Idle);
    }

    #[test]
    fn starting_again_restarts_from_full_white() {
        let mut transition = started();

        // Se deja avanzar hasta que ya se está desvaneciendo.
        for _ in 0..30 {
            transition.update(FRAME);
        }

        assert!(transition.intensity() < 1.0);

        transition.start();

        assert_eq!(transition.intensity(), 1.0);

        assert_eq!(transition.remaining(), DURATION);
    }

    // ----- Deltas hostiles -----

    #[test]
    fn hostile_deltas_do_not_advance_the_flash() {
        for delta in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut transition = started();

            for _ in 0..50 {
                assert_eq!(
                    transition.update(delta),
                    EncounterTransitionUpdate::Running,
                    "un delta de {delta} cambió el resultado",
                );
            }

            assert_eq!(transition.remaining(), DURATION);

            assert_eq!(transition.intensity(), 1.0);
        }
    }

    #[test]
    fn a_huge_delta_cannot_skip_the_whole_flash() {
        let mut transition = started();

        assert_eq!(transition.update(10.0), EncounterTransitionUpdate::Running);

        assert!(
            transition.is_active(),
            "un frame enorme se saltó el destello entero",
        );

        // Con el tope de 0.10 s por frame, un destello de 1.0 s
        // necesita exactamente diez avances por muy largos que sean.
        let mut updates = 1;

        loop {
            let update = transition.update(10.0);

            updates += 1;

            if update == EncounterTransitionUpdate::Finished {
                break;
            }

            assert!(updates < 100, "el destello no terminó");
        }

        assert_eq!(updates, 10, "hicieron falta {updates} avances");
    }

    // ----- Rango -----

    #[test]
    fn the_intensity_always_stays_inside_the_unit_range() {
        let mut transition = started();

        for _ in 0..400 {
            transition.update(FRAME);

            let intensity = transition.intensity();

            assert!(
                (0.0..=1.0).contains(&intensity),
                "intensidad fuera de rango: {intensity}",
            );
        }
    }

    // ----- Sin desvanecimiento -----

    #[test]
    fn a_hold_equal_to_the_duration_stays_white_until_the_end() {
        let mut transition = EncounterTransition::new(DURATION, DURATION);

        transition.start();

        while transition.update(FRAME) != EncounterTransitionUpdate::Finished {
            assert_eq!(
                transition.intensity(),
                1.0,
                "sin desvanecimiento debe seguir blanco",
            );
        }

        // Y al terminar desaparece de golpe.
        assert_eq!(transition.intensity(), 0.0);
    }
}

/// El destello no solo se ve: mientras dura, el encuentro no acepta
/// nada. Estas pruebas reproducen la misma rama que corre `main`
/// para que la garantía quede fijada aquí y no solo en el bucle.
#[cfg(test)]
mod transition_input_gate_tests {
    use super::EncounterTransition;
    use crate::game::encounter::{
        EncounterInput, EncounterPhase, EncounterSession, SCP_173_ENCOUNTER,
    };

    const DURATION: f32 = 1.0;
    const SOLID: f32 = 0.15;
    const FRAME: f32 = 1.0 / 60.0;

    fn navigation_input() -> EncounterInput {
        EncounterInput {
            next_down: true,
            previous_down: false,
            confirm_down: false,
        }
    }

    fn released_input() -> EncounterInput {
        EncounterInput {
            next_down: false,
            previous_down: false,
            confirm_down: false,
        }
    }

    /// Misma precedencia que la rama `GameState::Encounter` de `main`.
    fn frame(
        transition: &mut EncounterTransition,
        session: &mut EncounterSession,
        input: EncounterInput,
        delta_time: f32,
    ) {
        transition.update(delta_time);

        if transition.is_active() {
            session.seed_input_state(input);
        } else {
            session.update(input);
        }
    }

    #[test]
    fn an_active_transition_blocks_every_interaction() {
        let mut transition = EncounterTransition::new(DURATION, SOLID);

        let mut session = EncounterSession::new(SCP_173_ENCOUNTER);

        transition.start();

        let confirm_and_navigate = EncounterInput {
            next_down: true,
            previous_down: false,
            confirm_down: true,
        };

        // Medio segundo pulsando todo a la vez.
        for _ in 0..30 {
            frame(&mut transition, &mut session, confirm_and_navigate, FRAME);
        }

        assert!(transition.is_active());

        assert_eq!(session.selected_index(), 0, "la selección se movió");

        assert_eq!(
            session.phase(),
            EncounterPhase::PlayerChoice,
            "el turno avanzó durante el destello",
        );

        assert_eq!(session.turn_count(), 0);
    }

    #[test]
    fn a_finished_transition_lets_the_encounter_run_again() {
        let mut transition = EncounterTransition::new(DURATION, SOLID);

        let mut session = EncounterSession::new(SCP_173_ENCOUNTER);

        transition.start();

        // Sin tocar nada hasta que el destello termina.
        while transition.is_active() {
            frame(&mut transition, &mut session, released_input(), FRAME);
        }

        frame(&mut transition, &mut session, navigation_input(), FRAME);

        assert_eq!(session.selected_index(), 1);
    }

    #[test]
    fn a_key_held_through_the_flash_stays_seeded() {
        let mut transition = EncounterTransition::new(DURATION, SOLID);

        let mut session = EncounterSession::new(SCP_173_ENCOUNTER);

        transition.start();

        // La tecla ya venía pulsada desde antes de entrar.
        while transition.is_active() {
            frame(&mut transition, &mut session, navigation_input(), FRAME);
        }

        // Sigue pulsada al revelarse el encuentro: no hay flanco.
        for _ in 0..20 {
            frame(&mut transition, &mut session, navigation_input(), FRAME);
        }

        assert_eq!(
            session.selected_index(),
            0,
            "la tecla sostenida atravesó el destello",
        );

        // Soltar y volver a pulsar sí cuenta.
        frame(&mut transition, &mut session, released_input(), FRAME);

        frame(&mut transition, &mut session, navigation_input(), FRAME);

        assert_eq!(session.selected_index(), 1);
    }

    #[test]
    fn the_reveal_never_skips_the_first_option() {
        let mut transition = EncounterTransition::new(DURATION, SOLID);

        let mut session = EncounterSession::new(SCP_173_ENCOUNTER);

        transition.start();

        // Cada frame del destello siembra el estado, así que da igual
        // en qué momento exacto termine: al revelar sigue en la
        // primera acción.
        for step in 0..40 {
            let delta_time = if step % 3 == 0 { FRAME } else { FRAME * 2.0 };

            frame(
                &mut transition,
                &mut session,
                navigation_input(),
                delta_time,
            );

            assert_eq!(
                session.selected_index(),
                0,
                "la selección saltó en el paso {step}",
            );
        }
    }
}
