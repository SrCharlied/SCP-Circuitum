use std::fs::File;
use std::io::BufReader;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

use crate::game::GameState;

/// Pista ambiental que acompaña la exploración.
const AMBIENT_MUSIC_PATH: &str = "./assets/audio/minor_terror.ogg";

/// Volumen inicial moderado para dejar espacio a
/// futuros efectos de sonido.
const AMBIENT_MUSIC_VOLUME: f32 = 0.25;

/// Qué debe ocurrir con la música ambiental en el frame actual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmbientMusicAction {
    Start,
    Stop,
    Leave,
}

/// La música ambiental pertenece a la partida en curso: suena
/// mientras se explora, se pausa o se cambia de nivel, y calla
/// cuando todavía no empezó o cuando la sesión ya terminó.
fn music_belongs_to(state: GameState) -> bool {
    matches!(
        state,
        GameState::Playing | GameState::Paused | GameState::LevelTransition
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

pub struct AudioManager {
    /// Debe permanecer vivo mientras suene la pista: al soltarlo se
    /// cierra el stream del dispositivo y el audio se detiene.
    device_sink: MixerDeviceSink,

    /// `Some` únicamente mientras la pista está sonando.
    ambient_music: Option<Player>,

    /// Tras un fallo al abrir o decodificar la pista dejamos de
    /// reintentar, para no repetir el error en cada frame.
    ambient_music_disabled: bool,
}

impl AudioManager {
    pub fn new() -> Result<Self, String> {
        let mut device_sink = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("no se pudo abrir el dispositivo de audio: {error}"))?;

        // Al cerrarse, rodio avisa por stderr que el audio se
        // detendrá. Es ruido que parece un error cuando el jugador
        // simplemente cierra la ventana.
        device_sink.log_on_drop(false);

        Ok(Self {
            device_sink,
            ambient_music: None,
            ambient_music_disabled: false,
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
}

#[cfg(test)]
mod tests {
    use super::{AmbientMusicAction, ambient_music_action};
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
