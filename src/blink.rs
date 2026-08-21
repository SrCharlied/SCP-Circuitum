//! Ciclo físico de parpadeo del sujeto.
//!
//! Mientras los ojos están cerrados, SCP-173 deja de estar observado
//! aunque siga dentro del campo de visión. La geometría de la
//! observación no cambia: se combina con este estado aparte.

/// Estado de los párpados.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlinkPhase {
    Open,
    Closed,
}

/// Qué ocurrió con el parpadeo en el frame actual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlinkUpdate {
    Idle,
    Started,
    Ended,
}

/// Tope de avance por frame.
///
/// Un frame muy largo no puede saltarse la ventana cerrada entera:
/// con este límite siempre quedan varios frames dentro de ella.
const MAX_BLINK_DELTA: f32 = 0.10;

/// Observación real de la entidad.
///
/// La geometría por sí sola no basta: con los ojos cerrados nadie
/// está mirando, por mucho que la figura caiga dentro del FOV.
pub fn effective_observation(geometrically_observed: bool, eyes_closed: bool) -> bool {
    geometrically_observed && !eyes_closed
}

/// El parpadeo solo importa donde existe SCP-173.
pub fn blink_enabled(level_number: usize) -> bool {
    level_number == 1
}

pub struct BlinkSystem {
    open_duration: f32,
    closed_duration: f32,
    open_remaining: f32,
    closed_remaining: f32,
    phase: BlinkPhase,
    manual_was_down: bool,
}

impl BlinkSystem {
    /// # Pánico
    ///
    /// Si alguna duración no es un número finito mayor que cero. Son
    /// parámetros estáticos del programador, así que conviene
    /// detectarlo al arrancar y no con un ciclo que nunca avanza.
    pub fn new(open_duration: f32, closed_duration: f32) -> Self {
        assert!(
            open_duration.is_finite() && open_duration > 0.0,
            "El intervalo entre parpadeos debe ser finito y mayor que cero, y llegó {open_duration}",
        );

        assert!(
            closed_duration.is_finite() && closed_duration > 0.0,
            "La duración del parpadeo debe ser finita y mayor que cero, y llegó {closed_duration}",
        );

        Self {
            open_duration,
            closed_duration,
            open_remaining: open_duration,
            closed_remaining: 0.0,
            phase: BlinkPhase::Open,
            manual_was_down: false,
        }
    }

    pub fn phase(&self) -> BlinkPhase {
        self.phase
    }

    pub fn is_closed(&self) -> bool {
        self.phase == BlinkPhase::Closed
    }

    /// Cuánto queda del intervalo, de 1 recién abierto a 0 al
    /// parpadear.
    pub fn meter_ratio(&self) -> f32 {
        match self.phase {
            BlinkPhase::Closed => 0.0,

            BlinkPhase::Open => sanitized_ratio(self.open_remaining / self.open_duration),
        }
    }

    /// Cuánto cubren los párpados: 0 al empezar a cerrarse, 1 en el
    /// punto más cerrado y 0 al volver a abrirse.
    pub fn closure_ratio(&self) -> f32 {
        if self.phase != BlinkPhase::Closed {
            return 0.0;
        }

        let progress = 1.0 - self.closed_remaining / self.closed_duration;

        // Triangular: sube hasta la mitad del cierre y vuelve a bajar.
        sanitized_ratio(1.0 - (2.0 * progress - 1.0).abs())
    }

    /// Registra el estado de la tecla sin tocar el ciclo.
    ///
    /// Se usa mientras el gameplay está detenido: así una pulsación
    /// hecha fuera de la partida no queda encolada para disparar un
    /// parpadeo en cuanto se reanude.
    pub fn sync_manual_key(&mut self, manual_down: bool) {
        self.manual_was_down = manual_down;
    }

    /// Reinicia el ciclo completo para un intento nuevo.
    pub fn reset(&mut self, manual_down: bool) {
        self.phase = BlinkPhase::Open;

        self.open_remaining = self.open_duration;

        self.closed_remaining = 0.0;

        self.manual_was_down = manual_down;
    }

    /// Avanza el ciclo un frame.
    ///
    /// Un `delta_time` no finito no mueve los temporizadores, pero sí
    /// sincroniza la tecla, para que un frame roto no deje encolado
    /// un parpadeo manual.
    pub fn update(&mut self, delta_time: f32, manual_down: bool) -> BlinkUpdate {
        if !delta_time.is_finite() {
            self.manual_was_down = manual_down;

            return BlinkUpdate::Idle;
        }

        let safe_delta = delta_time.clamp(0.0, MAX_BLINK_DELTA);

        let manual_fired = manual_down && !self.manual_was_down;

        self.manual_was_down = manual_down;

        match self.phase {
            BlinkPhase::Open => {
                // El parpadeo voluntario se adelanta al automático,
                // así que ambos juntos producen un solo cierre.
                if manual_fired {
                    self.begin_closing();

                    return BlinkUpdate::Started;
                }

                self.open_remaining -= safe_delta;

                if self.open_remaining <= 0.0 {
                    self.begin_closing();

                    return BlinkUpdate::Started;
                }

                BlinkUpdate::Idle
            }

            // Pulsar Espacio aquí no alarga ni reinicia el cierre.
            BlinkPhase::Closed => {
                self.closed_remaining -= safe_delta;

                if self.closed_remaining <= 0.0 {
                    self.phase = BlinkPhase::Open;

                    // El intervalo arranca entero, venga de donde
                    // venga el parpadeo.
                    self.open_remaining = self.open_duration;

                    self.closed_remaining = 0.0;

                    return BlinkUpdate::Ended;
                }

                BlinkUpdate::Idle
            }
        }
    }

    /// Entra en la ventana cerrada sin consumir tiempo de este frame:
    /// el sobrante no debe atravesar la fase entera.
    fn begin_closing(&mut self) {
        self.phase = BlinkPhase::Closed;

        self.closed_remaining = self.closed_duration;
    }
}

/// Acota una proporción a `0..=1` y convierte lo no finito en cero.
fn sanitized_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{BlinkPhase, BlinkSystem, BlinkUpdate, blink_enabled, effective_observation};

    const OPEN: f32 = 6.0;
    const CLOSED: f32 = 0.22;
    const FRAME: f32 = 1.0 / 60.0;

    fn system() -> BlinkSystem {
        BlinkSystem::new(OPEN, CLOSED)
    }

    /// Avanza frames sin pulsar nada hasta que ocurra algo.
    fn run_until(system: &mut BlinkSystem, wanted: BlinkUpdate, max_frames: usize) -> usize {
        for frame in 1..=max_frames {
            if system.update(FRAME, false) == wanted {
                return frame;
            }
        }

        panic!("no se alcanzó {wanted:?} en {max_frames} frames");
    }

    // ----- Configuración -----

    #[test]
    fn a_valid_configuration_is_accepted() {
        let system = system();

        assert_eq!(system.phase(), BlinkPhase::Open);
    }

    #[test]
    #[should_panic(expected = "intervalo entre parpadeos")]
    fn a_zero_interval_fails_clearly() {
        let _ = BlinkSystem::new(0.0, CLOSED);
    }

    #[test]
    #[should_panic(expected = "intervalo entre parpadeos")]
    fn a_negative_interval_fails_clearly() {
        let _ = BlinkSystem::new(-1.0, CLOSED);
    }

    #[test]
    #[should_panic(expected = "intervalo entre parpadeos")]
    fn a_nan_interval_fails_clearly() {
        let _ = BlinkSystem::new(f32::NAN, CLOSED);
    }

    #[test]
    #[should_panic(expected = "duración del parpadeo")]
    fn a_zero_closure_fails_clearly() {
        let _ = BlinkSystem::new(OPEN, 0.0);
    }

    #[test]
    #[should_panic(expected = "duración del parpadeo")]
    fn a_negative_closure_fails_clearly() {
        let _ = BlinkSystem::new(OPEN, -0.5);
    }

    #[test]
    #[should_panic(expected = "duración del parpadeo")]
    fn an_infinite_closure_fails_clearly() {
        let _ = BlinkSystem::new(OPEN, f32::INFINITY);
    }

    // ----- Estado inicial -----

    #[test]
    fn a_new_system_starts_open_with_a_full_meter() {
        let system = system();

        assert_eq!(system.phase(), BlinkPhase::Open);

        assert!(!system.is_closed());

        assert_eq!(system.meter_ratio(), 1.0);

        assert_eq!(system.closure_ratio(), 0.0);
    }

    // ----- Ciclo automático -----

    #[test]
    fn the_meter_drains_while_the_eyes_stay_open() {
        let mut system = system();

        let mut previous = system.meter_ratio();

        for _ in 0..30 {
            system.update(FRAME, false);

            let current = system.meter_ratio();

            assert!(current < previous, "el medidor no bajó: {current}");

            previous = current;
        }
    }

    #[test]
    fn the_cycle_closes_the_eyes_on_its_own() {
        let mut system = system();

        let frames = run_until(&mut system, BlinkUpdate::Started, 1_000);

        // Seis segundos a 60 FPS.
        assert!(
            (355..=365).contains(&frames),
            "el parpadeo llegó en el frame {frames}",
        );

        assert!(system.is_closed());

        assert_eq!(system.meter_ratio(), 0.0);
    }

    #[test]
    fn the_closed_window_ends_and_restarts_the_interval() {
        let mut system = system();

        run_until(&mut system, BlinkUpdate::Started, 1_000);

        let frames = run_until(&mut system, BlinkUpdate::Ended, 1_000);

        // 0.22 s a 60 FPS: unos catorce frames.
        assert!(
            (12..=16).contains(&frames),
            "el cierre duró {frames} frames",
        );

        assert_eq!(system.phase(), BlinkPhase::Open);

        assert_eq!(system.meter_ratio(), 1.0);

        assert_eq!(system.closure_ratio(), 0.0);
    }

    // ----- Parpadeo manual -----

    #[test]
    fn pressing_space_closes_the_eyes_immediately() {
        let mut system = system();

        assert_eq!(system.update(FRAME, true), BlinkUpdate::Started);

        assert!(system.is_closed());
    }

    #[test]
    fn holding_space_does_not_repeat_the_blink() {
        let mut system = system();

        assert_eq!(system.update(FRAME, true), BlinkUpdate::Started);

        // Se termina el cierre con la tecla aún pulsada.
        run_until_with(&mut system, BlinkUpdate::Ended, true, 100);

        // Y sostenerla no vuelve a cerrar los ojos.
        for _ in 0..200 {
            assert_ne!(system.update(FRAME, true), BlinkUpdate::Started);
        }

        assert!(!system.is_closed());
    }

    /// Igual que `run_until`, con la tecla en un estado fijo.
    fn run_until_with(
        system: &mut BlinkSystem,
        wanted: BlinkUpdate,
        manual_down: bool,
        max_frames: usize,
    ) -> usize {
        for frame in 1..=max_frames {
            if system.update(FRAME, manual_down) == wanted {
                return frame;
            }
        }

        panic!("no se alcanzó {wanted:?} en {max_frames} frames");
    }

    #[test]
    fn releasing_space_allows_another_manual_blink() {
        let mut system = system();

        assert_eq!(system.update(FRAME, true), BlinkUpdate::Started);

        run_until_with(&mut system, BlinkUpdate::Ended, true, 100);

        // Soltar y volver a pulsar sí dispara otro.
        system.update(FRAME, false);

        assert_eq!(system.update(FRAME, true), BlinkUpdate::Started);
    }

    #[test]
    fn pressing_space_while_closed_does_not_extend_the_window() {
        let mut held = system();
        let mut untouched = system();

        held.update(FRAME, true);
        untouched.update(FRAME, true);

        // En uno se machaca Espacio durante todo el cierre.
        let mut held_frames = 0;
        let mut released_frames = 0;

        loop {
            held_frames += 1;

            let pressed = held_frames % 2 == 0;

            if held.update(FRAME, pressed) == BlinkUpdate::Ended {
                break;
            }
        }

        loop {
            released_frames += 1;

            if untouched.update(FRAME, false) == BlinkUpdate::Ended {
                break;
            }
        }

        assert_eq!(
            held_frames, released_frames,
            "pulsar Espacio alteró la duración del cierre",
        );
    }

    #[test]
    fn a_manual_and_automatic_blink_only_start_once() {
        let mut system = system();

        // Se lleva el intervalo justo al borde.
        while system.meter_ratio() > 0.005 {
            system.update(FRAME, false);
        }

        // Y en ese frame se pulsa Espacio: un solo cierre.
        assert_eq!(system.update(FRAME, true), BlinkUpdate::Started);

        assert_ne!(system.update(FRAME, true), BlinkUpdate::Started);

        assert!(system.is_closed());
    }

    // ----- Sincronización y reinicio -----

    #[test]
    fn syncing_the_key_does_not_move_the_timers() {
        let mut system = system();

        for _ in 0..60 {
            system.update(FRAME, false);
        }

        let before = system.meter_ratio();

        for _ in 0..600 {
            system.sync_manual_key(true);
        }

        assert_eq!(system.meter_ratio(), before);

        assert_eq!(system.phase(), BlinkPhase::Open);

        // Y la tecla ya sostenida no dispara al reanudar.
        assert_ne!(system.update(FRAME, true), BlinkUpdate::Started);
    }

    #[test]
    fn resetting_restores_the_whole_cycle() {
        let mut system = system();

        system.update(FRAME, true);

        assert!(system.is_closed());

        system.reset(false);

        assert_eq!(system.phase(), BlinkPhase::Open);

        assert_eq!(system.meter_ratio(), 1.0);

        assert_eq!(system.closure_ratio(), 0.0);
    }

    #[test]
    fn resetting_with_space_held_requires_a_release_first() {
        let mut system = system();

        system.reset(true);

        // Sostenida desde antes: no cuenta como pulsación nueva.
        for _ in 0..30 {
            assert_ne!(system.update(FRAME, true), BlinkUpdate::Started);
        }

        // Hace falta soltar.
        system.update(FRAME, false);

        assert_eq!(system.update(FRAME, true), BlinkUpdate::Started);
    }

    // ----- Deltas hostiles -----

    #[test]
    fn a_negative_delta_does_not_advance_the_cycle() {
        let mut system = system();

        let before = system.meter_ratio();

        for _ in 0..100 {
            assert_eq!(system.update(-1.0, false), BlinkUpdate::Idle);
        }

        assert_eq!(system.meter_ratio(), before);
    }

    #[test]
    fn non_finite_deltas_do_not_advance_the_cycle() {
        for delta in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut system = system();

            for _ in 0..100 {
                assert_eq!(system.update(delta, false), BlinkUpdate::Idle);
            }

            assert_eq!(system.meter_ratio(), 1.0);

            assert_eq!(system.phase(), BlinkPhase::Open);
        }
    }

    #[test]
    fn a_huge_delta_cannot_skip_the_closed_window() {
        let mut system = system();

        // Un frame enorme abre el parpadeo...
        run_until_with(&mut system, BlinkUpdate::Started, false, 1_000);

        assert!(system.is_closed());

        // ...y otro igual de largo no lo atraviesa de golpe.
        assert_eq!(system.update(10.0, false), BlinkUpdate::Idle);

        assert!(
            system.is_closed(),
            "un frame enorme se saltó la ventana cerrada",
        );
    }

    // ----- Rangos -----

    #[test]
    fn both_ratios_stay_inside_the_unit_range() {
        let mut system = system();

        let mut highest_closure: f32 = 0.0;

        for frame in 0..2_000 {
            // Se alterna la tecla para recorrer también los manuales.
            system.update(FRAME, frame % 500 == 0);

            let meter = system.meter_ratio();

            let closure = system.closure_ratio();

            assert!(
                (0.0..=1.0).contains(&meter),
                "medidor fuera de rango: {meter}",
            );

            assert!(
                (0.0..=1.0).contains(&closure),
                "cierre fuera de rango: {closure}",
            );

            highest_closure = highest_closure.max(closure);
        }

        // Y el cierre llega casi al máximo en la mitad de la ventana.
        assert!(
            highest_closure > 0.85,
            "el cierre nunca se acercó a 1: {highest_closure}",
        );
    }

    #[test]
    fn the_closure_peaks_near_the_middle_of_the_window() {
        let mut system = system();

        system.update(FRAME, true);

        assert!(system.is_closed());

        // Justo al empezar, los párpados no cubren nada.
        assert!(system.closure_ratio() < 0.2);

        // A mitad del cierre, cubren casi todo.
        let half_frames = ((CLOSED / 2.0) / FRAME).round() as usize;

        for _ in 0..half_frames {
            system.update(FRAME, false);
        }

        assert!(
            system.closure_ratio() > 0.85,
            "en la mitad solo cubría {}",
            system.closure_ratio(),
        );
    }

    // ----- Observación y nivel -----

    #[test]
    fn closing_the_eyes_cancels_the_observation() {
        assert!(effective_observation(true, false));

        assert!(!effective_observation(true, true));

        assert!(!effective_observation(false, false));

        assert!(!effective_observation(false, true));
    }

    #[test]
    fn blinking_only_matters_on_the_first_level() {
        assert!(blink_enabled(1));

        for level in [0, 2, 3, 99] {
            assert!(!blink_enabled(level), "el nivel {level} no tiene parpadeo");
        }
    }
}
