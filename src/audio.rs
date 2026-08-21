use std::fs::File;
use std::io::{BufReader, Cursor};
use std::sync::Arc;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

use crate::encounter::{EncounterPhase, EncounterUpdate};
use crate::game::GameState;
use crate::player::PlayerMotion;

/// Pista ambiental que acompaña la exploración.
const AMBIENT_MUSIC_PATH: &str = "./assets/audio/minor_terror.ogg";

/// Volumen inicial moderado para dejar espacio a
/// futuros efectos de sonido.
const AMBIENT_MUSIC_VOLUME: f32 = 0.25;

/// Efecto de paso del jugador.
const FOOTSTEP_PATH: &str = "./assets/audio/footstep.ogg";

/// Por encima de la música ambiental (0.25) para que se escuche con
/// claridad, sin llegar a taparla.
const FOOTSTEP_VOLUME: f32 = 0.55;

/// Segundos entre pasos caminando.
const WALKING_FOOTSTEP_INTERVAL: f32 = 0.45;

/// Segundos entre pasos corriendo. La proporción respecto al
/// intervalo de caminata sigue a la de las velocidades del jugador
/// (500 contra 350), así que la zancada se mantiene coherente.
const RUNNING_FOOTSTEP_INTERVAL: f32 = 0.32;

/// Tope de pasos sonando a la vez. Sin él, un efecto largo podría
/// acumularse indefinidamente mientras el jugador camina.
const MAX_CONCURRENT_FOOTSTEPS: usize = 4;

/// Impacto letal de SCP-173.
const CRACK_PATH: &str = "./assets/audio/crack.ogg";

/// Por encima de los pasos: es el desenlace del encuentro.
const CRACK_VOLUME: f32 = 0.75;

/// Lee un efecto del disco una sola vez, al arrancar.
///
/// Si falta, el juego continúa sin él: devuelve `None` junto al
/// indicador de desactivado, y el aviso se imprime una única vez.
/// Cada efecto se carga por separado, así que un archivo ausente no
/// arrastra a los demás.
fn load_effect(path: &str, missing_description: &str) -> (Option<Arc<[u8]>>, bool) {
    match std::fs::read(path) {
        Ok(bytes) => (Some(Arc::from(bytes.into_boxed_slice())), false),

        Err(error) => {
            eprintln!(
                "Audio: no se pudo abrir '{path}': {error}. \
                 La partida continúa sin {missing_description}."
            );

            (None, true)
        }
    }
}

/// Si este resultado del encuentro debe disparar el impacto.
///
/// El sonido pertenece al instante en que aparece "CRACK.", no al
/// cierre del encuentro: por eso solo la entrada en `DeathBeat` lo
/// dispara, y lo hace una sola vez porque esa transición ocurre en
/// un único frame.
pub fn should_play_crack(update: EncounterUpdate) -> bool {
    matches!(
        update,
        EncounterUpdate::PhaseAdvanced(EncounterPhase::DeathBeat)
    )
}

/// Qué debe ocurrir con la música ambiental en el frame actual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmbientMusicAction {
    Start,
    Stop,
    Leave,
}

/// La música ambiental pertenece a la partida en curso: suena
/// mientras se explora, se pausa, se cambia de nivel o se lee el
/// informe del sector, y calla cuando todavía no empezó o cuando la
/// sesión ya terminó.
fn music_belongs_to(state: GameState) -> bool {
    matches!(
        state,
        GameState::Playing
            | GameState::Paused
            | GameState::LevelTransition
            | GameState::LevelSuccess
            | GameState::Encounter
    )
}

/// Decide la acción del frame comparando el estado del juego con
/// lo que ya está sonando.
///
/// Es una función pura y no toca el dispositivo de audio, así que
/// puede probarse sin hardware. Concentra aquí la regla que impide
/// arrancar la pista dos veces.
pub fn ambient_music_action(state: GameState, already_playing: bool) -> AmbientMusicAction {
    match (music_belongs_to(state), already_playing) {
        (true, false) => AmbientMusicAction::Start,

        (false, true) => AmbientMusicAction::Stop,

        _ => AmbientMusicAction::Leave,
    }
}

/// Cada cuánto suena un paso según cómo se mueva el jugador.
/// `None` significa que no debe sonar ninguno.
pub fn footstep_interval(motion: PlayerMotion) -> Option<f32> {
    match motion {
        PlayerMotion::Still => None,

        PlayerMotion::Walking => Some(WALKING_FOOTSTEP_INTERVAL),

        PlayerMotion::Running => Some(RUNNING_FOOTSTEP_INTERVAL),
    }
}

/// Temporizador que reparte los pasos en el tiempo.
///
/// Es lógica pura basada en `delta_time`, sin dispositivo de audio,
/// así que la cadencia se puede probar sin hardware.
#[derive(Debug, Default)]
pub struct FootstepCadence {
    time_until_next: f32,
}

impl FootstepCadence {
    pub fn new() -> Self {
        Self::default()
    }

    /// Olvida el tiempo acumulado. El próximo paso volverá a sonar
    /// de inmediato al reanudar la marcha.
    pub fn reset(&mut self) {
        self.time_until_next = 0.0;
    }

    /// Avanza el temporizador y devuelve `true` cuando toca un paso.
    ///
    /// Detenerse reinicia la cadencia, así que arrancar a caminar
    /// suena al instante en lugar de esperar medio intervalo.
    pub fn advance(&mut self, motion: PlayerMotion, delta_time: f32) -> bool {
        let Some(interval) = footstep_interval(motion) else {
            self.reset();

            return false;
        };

        if self.time_until_next <= 0.0 {
            self.time_until_next = interval;

            return true;
        }

        self.time_until_next -= delta_time;

        if self.time_until_next <= 0.0 {
            self.time_until_next = interval;

            return true;
        }

        false
    }
}

pub struct AudioManager {
    /// Debe permanecer vivo mientras suene la pista: al soltarlo se
    /// cierra el stream del dispositivo y el audio se detiene.
    device_sink: MixerDeviceSink,

    /// `Some` únicamente mientras la pista está sonando.
    ambient_music: Option<Player>,

    /// Tras un fallo al abrir o decodificar la pista dejamos de
    /// reintentar, para no repetir el error en cada frame.
    ambient_music_disabled: bool,

    /// El OGG del paso se lee del disco una sola vez. Cada paso
    /// construye un decodificador sobre estos bytes, nunca vuelve a
    /// tocar el sistema de archivos.
    footstep_sound: Option<Arc<[u8]>>,

    /// Pasos sonando ahora mismo. Se podan los terminados en cada
    /// disparo, así que la lista no crece sin límite.
    footstep_players: Vec<Player>,

    footstep_cadence: FootstepCadence,

    footsteps_disabled: bool,

    /// El OGG del impacto, también residente en memoria.
    crack_sound: Option<Arc<[u8]>>,

    /// El impacto en curso. Solo puede haber uno.
    crack_player: Option<Player>,

    crack_disabled: bool,
}

impl AudioManager {
    pub fn new() -> Result<Self, String> {
        let mut device_sink = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("no se pudo abrir el dispositivo de audio: {error}"))?;

        // Al cerrarse, rodio avisa por stderr que el audio se
        // detendrá. Es ruido que parece un error cuando el jugador
        // simplemente cierra la ventana.
        device_sink.log_on_drop(false);

        // Los efectos se cargan una sola vez al inicio, y por
        // separado: que falte uno no deja al juego sin los otros.
        let (footstep_sound, footsteps_disabled) = load_effect(FOOTSTEP_PATH, "pasos");

        let (crack_sound, crack_disabled) = load_effect(CRACK_PATH, "el impacto");

        Ok(Self {
            device_sink,
            ambient_music: None,
            ambient_music_disabled: false,
            footstep_sound,
            footstep_players: Vec::new(),
            footstep_cadence: FootstepCadence::new(),
            footsteps_disabled,
            crack_sound,
            crack_player: None,
            crack_disabled,
        })
    }

    pub fn is_ambient_music_playing(&self) -> bool {
        self.ambient_music.is_some()
    }

    /// Arranca la pista ambiental en loop. No hace nada si ya está
    /// sonando, de modo que llamarla de más nunca la duplica.
    pub fn play_ambient_music(&mut self) -> Result<(), String> {
        if self.is_ambient_music_playing() {
            return Ok(());
        }

        let file = File::open(AMBIENT_MUSIC_PATH)
            .map_err(|error| format!("no se pudo abrir '{AMBIENT_MUSIC_PATH}': {error}"))?;

        // El loop lo resuelve el decodificador de rodio, que vuelve
        // al inicio de la pista sin intervención del bucle principal.
        let looped_music = Decoder::new_looped(BufReader::new(file))
            .map_err(|error| format!("no se pudo decodificar '{AMBIENT_MUSIC_PATH}': {error}"))?;

        // Un Player nuevo por reproducción. Reutilizar uno detenido
        // haría que `append` esperara a que se vacíe la cola, y eso
        // bloquearía el hilo del juego.
        let player = Player::connect_new(self.device_sink.mixer());

        player.set_volume(AMBIENT_MUSIC_VOLUME);

        player.append(looped_music);

        self.ambient_music = Some(player);

        Ok(())
    }

    /// Detiene la pista. `Drop` de `Player` marca la fuente como
    /// detenida sin esperar al hilo de audio.
    pub fn stop_ambient_music(&mut self) {
        self.ambient_music = None;
    }

    /// Único punto desde el que el bucle principal toca el audio.
    /// Llamarlo cada frame es seguro: la acción se deriva del estado.
    pub fn update_for_state(&mut self, state: GameState) {
        match ambient_music_action(state, self.is_ambient_music_playing()) {
            AmbientMusicAction::Start => {
                if self.ambient_music_disabled {
                    return;
                }

                if let Err(error) = self.play_ambient_music() {
                    eprintln!("Audio: {error}. La partida continúa sin música.");

                    self.ambient_music_disabled = true;
                }
            }

            AmbientMusicAction::Stop => self.stop_ambient_music(),

            AmbientMusicAction::Leave => {}
        }
    }

    /// Reproduce un paso. Cada uno usa su propio `Player`, que es la
    /// forma de que suenen a la vez en lugar de encolarse uno
    /// detrás de otro.
    fn play_footstep(&mut self) -> Result<(), String> {
        let Some(footstep_sound) = self.footstep_sound.as_ref() else {
            return Err(format!("'{FOOTSTEP_PATH}' no está cargado"));
        };

        // Retirar los que ya terminaron antes de añadir otro.
        self.footstep_players.retain(|player| !player.empty());

        // Si aun así se llegó al tope, cae el más antiguo. Preferimos
        // cortar un paso viejo a dejar crecer la lista.
        while self.footstep_players.len() >= MAX_CONCURRENT_FOOTSTEPS {
            self.footstep_players.remove(0);
        }

        // `Arc` se clona barato: el decodificador lee de los bytes ya
        // residentes en memoria, sin volver al disco.
        let footstep = Decoder::new(Cursor::new(Arc::clone(footstep_sound)))
            .map_err(|error| format!("no se pudo decodificar '{FOOTSTEP_PATH}': {error}"))?;

        let player = Player::connect_new(self.device_sink.mixer());

        player.set_volume(FOOTSTEP_VOLUME);

        player.append(footstep);

        self.footstep_players.push(player);

        Ok(())
    }

    /// Reproduce el impacto letal una vez.
    ///
    /// Devuelve `true` si llegó a sonar. Cualquier fallo deja el
    /// efecto desactivado y la partida continúa: perder el sonido no
    /// puede interrumpir el encuentro.
    pub fn play_crack(&mut self) -> bool {
        if self.crack_disabled {
            return false;
        }

        let Some(crack_sound) = self.crack_sound.as_ref() else {
            return false;
        };

        // Solo hay un impacto: si quedaba uno sonando, se descarta.
        self.crack_player = None;

        // `Arc` se clona barato: el decodificador lee de los bytes ya
        // residentes en memoria, sin volver al disco.
        let crack = match Decoder::new(Cursor::new(Arc::clone(crack_sound))) {
            Ok(decoder) => decoder,

            Err(error) => {
                eprintln!(
                    "Audio: no se pudo decodificar '{CRACK_PATH}': {error}. \
                     La partida continúa sin el impacto."
                );

                self.crack_disabled = true;

                return false;
            }
        };

        // Un Player propio sobre el mixer que ya existe: no se abre
        // otro dispositivo ni otro stream.
        let player = Player::connect_new(self.device_sink.mixer());

        player.set_volume(CRACK_VOLUME);

        player.append(crack);

        // Mantenerlo vivo es lo que deja que el efecto termine.
        self.crack_player = Some(player);

        true
    }

    /// Corta el impacto. Se usa al reintentar o al volver al
    /// terminal, para que un archivo largo no invada la partida
    /// siguiente.
    pub fn stop_crack(&mut self) {
        self.crack_player = None;
    }

    /// Detiene los pasos en curso y reinicia la cadencia.
    pub fn stop_footsteps(&mut self) {
        self.footstep_players.clear();

        self.footstep_cadence.reset();
    }

    /// Punto único desde el que el bucle principal maneja los pasos.
    ///
    /// `motion` describe desplazamiento real: si el jugador empuja
    /// una pared llega como `Still` y no suena nada.
    pub fn update_footsteps(
        &mut self,
        state: GameState,
        motion: PlayerMotion,
        delta_time: f32,
    ) -> bool {
        // Fuera de la exploración no hay pasos, y lo que quedara
        // sonando se corta para que pausar no deje audio colgado.
        if state != GameState::Playing {
            self.stop_footsteps();

            return false;
        }

        if !self.footstep_cadence.advance(motion, delta_time) {
            return false;
        }

        if self.footsteps_disabled {
            return false;
        }

        if let Err(error) = self.play_footstep() {
            eprintln!("Audio: {error}. La partida continúa sin pasos.");

            self.footsteps_disabled = true;

            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::{AmbientMusicAction, ambient_music_action, music_belongs_to};
    use crate::game::GameState;

    #[test]
    fn welcome_screen_stays_silent() {
        assert_eq!(
            ambient_music_action(GameState::Welcome, false),
            AmbientMusicAction::Leave,
        );
    }

    #[test]
    fn music_starts_once_when_the_run_begins() {
        assert_eq!(
            ambient_music_action(GameState::Playing, false),
            AmbientMusicAction::Start,
        );

        // Los frames siguientes ya la encuentran sonando y no
        // vuelven a arrancarla.
        assert_eq!(
            ambient_music_action(GameState::Playing, true),
            AmbientMusicAction::Leave,
        );
    }

    #[test]
    fn music_survives_pause_and_level_change() {
        assert_eq!(
            ambient_music_action(GameState::Paused, true),
            AmbientMusicAction::Leave,
        );

        assert_eq!(
            ambient_music_action(GameState::LevelTransition, true),
            AmbientMusicAction::Leave,
        );
    }

    #[test]
    fn defeat_stops_the_music_and_the_footsteps() {
        // Al morir, la música no continúa.
        assert_eq!(
            ambient_music_action(GameState::Defeat, true),
            AmbientMusicAction::Stop,
        );

        assert_eq!(
            ambient_music_action(GameState::Defeat, false),
            AmbientMusicAction::Leave,
        );

        // Y los pasos tampoco: solo suenan en Playing.
        assert!(!music_belongs_to(GameState::Defeat));
    }

    #[test]
    fn music_stops_when_the_run_ends() {
        assert_eq!(
            ambient_music_action(GameState::Victory, true),
            AmbientMusicAction::Stop,
        );

        assert_eq!(
            ambient_music_action(GameState::Welcome, true),
            AmbientMusicAction::Stop,
        );
    }

    #[test]
    fn a_finished_run_is_not_stopped_twice() {
        assert_eq!(
            ambient_music_action(GameState::Victory, false),
            AmbientMusicAction::Leave,
        );
    }

    #[test]
    fn a_new_run_can_start_the_music_again() {
        // Tras volver al menú principal la pista quedó detenida.
        assert_eq!(
            ambient_music_action(GameState::Welcome, false),
            AmbientMusicAction::Leave,
        );

        assert_eq!(
            ambient_music_action(GameState::Playing, false),
            AmbientMusicAction::Start,
        );
    }
}

#[cfg(test)]
mod footstep_tests {
    use super::{
        FOOTSTEP_PATH, FootstepCadence, RUNNING_FOOTSTEP_INTERVAL, WALKING_FOOTSTEP_INTERVAL,
        footstep_interval,
    };
    use crate::player::PlayerMotion;

    /// Cuenta los pasos disparados al mantener `motion` durante
    /// `seconds`, simulando frames de 60 FPS.
    fn steps_over(motion: PlayerMotion, seconds: f32) -> usize {
        let mut cadence = FootstepCadence::new();

        let delta_time = 1.0 / 60.0;

        let frames = (seconds / delta_time).round() as usize;

        (0..frames)
            .filter(|_| cadence.advance(motion, delta_time))
            .count()
    }

    #[test]
    fn standing_still_never_plays_a_footstep() {
        assert_eq!(steps_over(PlayerMotion::Still, 5.0), 0);

        assert_eq!(footstep_interval(PlayerMotion::Still), None);
    }

    #[test]
    fn walking_plays_one_footstep_per_interval() {
        let seconds = 4.5;

        let expected = (seconds / WALKING_FOOTSTEP_INTERVAL).floor() as usize;

        let steps = steps_over(PlayerMotion::Walking, seconds);

        // El primer paso suena de inmediato, así que hay uno más que
        // intervalos completos.
        assert!(
            steps == expected || steps == expected + 1,
            "se esperaban ~{expected} pasos y hubo {steps}",
        );
    }

    #[test]
    fn running_uses_a_shorter_interval_than_walking() {
        assert!(RUNNING_FOOTSTEP_INTERVAL < WALKING_FOOTSTEP_INTERVAL);

        let seconds = 4.5;

        assert!(
            steps_over(PlayerMotion::Running, seconds) > steps_over(PlayerMotion::Walking, seconds),
        );
    }

    #[test]
    fn a_footstep_does_not_fire_on_every_frame() {
        let seconds = 2.0;

        let frames = (seconds * 60.0) as usize;

        let steps = steps_over(PlayerMotion::Running, seconds);

        assert!(steps > 0);

        assert!(steps < frames, "sonaron {steps} pasos en {frames} frames");
    }

    #[test]
    fn the_first_step_after_starting_to_walk_is_immediate() {
        let mut cadence = FootstepCadence::new();

        assert!(cadence.advance(PlayerMotion::Walking, 1.0 / 60.0));

        // El siguiente frame ya no dispara.
        assert!(!cadence.advance(PlayerMotion::Walking, 1.0 / 60.0));
    }

    #[test]
    fn stopping_resets_the_cadence() {
        let mut cadence = FootstepCadence::new();

        cadence.advance(PlayerMotion::Walking, 1.0 / 60.0);

        // Detenerse limpia el temporizador acumulado.
        assert!(!cadence.advance(PlayerMotion::Still, 1.0 / 60.0));

        // Y al reanudar vuelve a sonar de inmediato.
        assert!(cadence.advance(PlayerMotion::Walking, 1.0 / 60.0));
    }

    #[test]
    fn an_explicit_reset_makes_the_next_step_immediate() {
        let mut cadence = FootstepCadence::new();

        cadence.advance(PlayerMotion::Walking, 1.0 / 60.0);

        assert!(!cadence.advance(PlayerMotion::Walking, 1.0 / 60.0));

        cadence.reset();

        assert!(cadence.advance(PlayerMotion::Walking, 1.0 / 60.0));
    }

    #[test]
    fn the_footstep_asset_decodes_from_memory() {
        // No abre dispositivo de audio: solo comprueba que rodio
        // entiende el archivo que se cargará en memoria.
        let bytes = std::fs::read(FOOTSTEP_PATH).expect("el efecto de paso debe existir");

        assert!(!bytes.is_empty());

        rodio::Decoder::new(std::io::Cursor::new(bytes))
            .expect("rodio debe poder decodificar el efecto de paso");
    }
}

#[cfg(test)]
mod crack_tests {
    use super::{CRACK_PATH, should_play_crack};
    use crate::encounter::{
        EncounterInput, EncounterPhase, EncounterSession, EncounterUpdate, PlayerAction,
        SCP_173_ENCOUNTER,
    };

    const RELEASED: EncounterInput = EncounterInput {
        next_down: false,
        previous_down: false,
        confirm_down: false,
    };

    const CONFIRM: EncounterInput = EncounterInput {
        next_down: false,
        previous_down: false,
        confirm_down: true,
    };

    const NEXT: EncounterInput = EncounterInput {
        next_down: true,
        previous_down: false,
        confirm_down: false,
    };

    const FLEE: usize = 2;
    const GAZE: usize = 3;

    fn session() -> EncounterSession {
        EncounterSession::new(SCP_173_ENCOUNTER)
    }

    fn select(session: &mut EncounterSession, index: usize) {
        while session.selected_index() != index {
            session.update(NEXT);

            session.update(RELEASED);
        }
    }

    // ----- El asset -----

    #[test]
    fn the_crack_asset_exists_and_can_be_read() {
        let bytes = std::fs::read(CRACK_PATH).expect("el efecto de impacto debe existir");

        assert!(!bytes.is_empty());

        // Contenedor OGG.
        assert_eq!(&bytes[..4], b"OggS");
    }

    #[test]
    fn rodio_decodes_the_crack_asset_from_memory() {
        use rodio::Source;

        // No abre dispositivo de audio: solo comprueba que rodio
        // entiende el archivo que se cargará en memoria.
        let bytes = std::fs::read(CRACK_PATH).expect("el efecto de impacto debe existir");

        let decoder = rodio::Decoder::new(std::io::Cursor::new(bytes))
            .expect("rodio debe poder decodificar el impacto");

        let channels = decoder.channels().get();

        let sample_rate = decoder.sample_rate().get();

        let duration = decoder.total_duration();

        println!("CRACK canales={channels} sample_rate={sample_rate} duracion={duration:?}");

        // Y produce audio de verdad, no un flujo vacío.
        let samples = decoder.count();

        assert!(samples > 0, "el impacto no produjo ninguna muestra");

        println!(
            "CRACK muestras={samples} segundos={:.3}",
            samples as f64 / (sample_rate as f64 * channels as f64),
        );
    }

    // ----- El disparo -----

    #[test]
    fn only_entering_the_death_beat_plays_the_crack() {
        assert!(should_play_crack(EncounterUpdate::PhaseAdvanced(
            EncounterPhase::DeathBeat
        )));

        // Ningún otro resultado lo dispara.
        assert!(!should_play_crack(EncounterUpdate::Idle));

        assert!(!should_play_crack(EncounterUpdate::SelectionChanged));

        assert!(!should_play_crack(EncounterUpdate::PlayerDeath));

        for action in [
            PlayerAction::Attack,
            PlayerAction::Item,
            PlayerAction::Flee,
            PlayerAction::MaintainGaze,
        ] {
            assert!(!should_play_crack(EncounterUpdate::ActionTaken(action)));
        }

        for phase in [
            EncounterPhase::PlayerChoice,
            EncounterPhase::PlayerResolution,
            EncounterPhase::ForcedSequence,
            EncounterPhase::EnemyResolution,
        ] {
            assert!(
                !should_play_crack(EncounterUpdate::PhaseAdvanced(phase)),
                "la fase {phase:?} no debe disparar el impacto",
            );
        }
    }

    /// Recorre un encuentro completo contando disparos del impacto.
    fn triggers_along(mut session: EncounterSession, first_choice: usize) -> usize {
        select(&mut session, first_choice);

        let mut triggers = 0;

        for _ in 0..40 {
            let update = session.update(CONFIRM);

            session.update(RELEASED);

            if should_play_crack(update) {
                triggers += 1;
            }

            if update == EncounterUpdate::PlayerDeath {
                break;
            }
        }

        triggers
    }

    #[test]
    fn both_lethal_routes_trigger_the_crack_exactly_once() {
        // Huir: resolución, ataque letal, impacto.
        assert_eq!(triggers_along(session(), FLEE), 1);

        // Cuarto turno: secuencia forzada, ataque letal, impacto.
        assert_eq!(triggers_along(session(), GAZE), 1);
    }

    #[test]
    fn entering_the_death_beat_reports_exactly_that_phase() {
        let mut session = session();

        select(&mut session, FLEE);

        session.update(CONFIRM);
        session.update(RELEASED);

        session.update(CONFIRM);
        session.update(RELEASED);

        let update = session.update(CONFIRM);

        assert_eq!(
            update,
            EncounterUpdate::PhaseAdvanced(EncounterPhase::DeathBeat),
        );

        assert!(should_play_crack(update));
    }

    #[test]
    fn holding_confirm_in_the_death_beat_does_not_retrigger() {
        let mut session = session();

        select(&mut session, FLEE);

        session.update(CONFIRM);
        session.update(RELEASED);

        session.update(CONFIRM);
        session.update(RELEASED);

        // Entrada al impacto.
        assert!(should_play_crack(session.update(CONFIRM)));

        // Sostener la tecla no vuelve a producir el evento.
        for _ in 0..60 {
            let update = session.update(CONFIRM);

            assert_eq!(update, EncounterUpdate::Idle);

            assert!(!should_play_crack(update));
        }
    }

    #[test]
    fn confirming_after_release_reports_the_death_without_replaying() {
        let mut session = session();

        select(&mut session, FLEE);

        session.update(CONFIRM);
        session.update(RELEASED);

        session.update(CONFIRM);
        session.update(RELEASED);

        session.update(CONFIRM);
        session.update(RELEASED);

        assert_eq!(session.phase(), EncounterPhase::DeathBeat);

        let update = session.update(CONFIRM);

        assert_eq!(update, EncounterUpdate::PlayerDeath);

        // Cerrar el encuentro no vuelve a sonar.
        assert!(!should_play_crack(update));
    }
}
