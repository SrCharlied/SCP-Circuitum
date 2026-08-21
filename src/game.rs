pub mod encounter;
pub mod encounter_trigger;
pub mod scp173;

use std::time::Duration;

const FPS_OPTIONS: [u32; 3] = [60, 120, 144];

/// Expediente de un sector: ruta del mapa y los datos que muestra
/// el terminal de despliegue.
pub struct LevelInfo {
    pub path: &'static str,
    pub sector: &'static str,
    pub status: &'static str,
    pub risk: &'static str,
    pub note: &'static str,
}

/// Catálogo único de niveles. La ruta vive aquí junto a sus
/// metadatos, así que no existe una segunda lista que mantener
/// sincronizada.
const LEVELS: [LevelInfo; 2] = [
    LevelInfo {
        path: "./levels/level_01.txt",
        sector: "BLOQUE A - CONTENCIÓN",
        status: "COMPROMETIDO",
        risk: "CRÍTICO",
        note: "Actividad anómala detectada.",
    },
    LevelInfo {
        path: "./levels/level_02.txt",
        sector: "BLOQUE B - EVACUACIÓN",
        status: "INESTABLE",
        risk: "ELEVADO",
        note: "La señal exterior sigue activa.",
    },
];

/// Cantidad de sectores disponibles en el terminal.
pub fn level_count() -> usize {
    LEVELS.len()
}

/// Expediente de un sector por índice. Devuelve `None` si el índice
/// no existe, en lugar de indexar sin validar.
pub fn level_info(index: usize) -> Option<&'static LevelInfo> {
    LEVELS.get(index)
}

pub struct GameSettings {
    selected_fps_index: usize,
    pub show_fps: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            selected_fps_index: 0,
            show_fps: true,
        }
    }
}

impl GameSettings {
    pub fn target_fps(&self) -> u32 {
        FPS_OPTIONS[self.selected_fps_index]
    }

    pub fn target_frame_time(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.target_fps() as f64)
    }

    pub fn select_next_fps(&mut self) {
        self.selected_fps_index = (self.selected_fps_index + 1) % FPS_OPTIONS.len();
    }

    pub fn select_previous_fps(&mut self) {
        if self.selected_fps_index == 0 {
            self.selected_fps_index = FPS_OPTIONS.len() - 1;
        } else {
            self.selected_fps_index -= 1;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameState {
    Welcome,
    LevelSelection,
    Playing,
    Paused,
    LevelTransition,
    LevelSuccess,
    Defeat,
    Encounter,
    Victory,
}

impl GameState {
    pub fn toggle_pause(&mut self) {
        *self = match *self {
            GameState::Playing => GameState::Paused,

            GameState::Paused => GameState::Playing,

            other => other,
        };
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VictoryMenuOption {
    MainMenu,
    Exit,
}

impl Default for VictoryMenuOption {
    fn default() -> Self {
        Self::MainMenu
    }
}

impl VictoryMenuOption {
    pub fn select_next(&mut self) {
        *self = match *self {
            Self::MainMenu => Self::Exit,
            Self::Exit => Self::MainMenu,
        };
    }

    pub fn select_previous(&mut self) {
        self.select_next();
    }
}

/// Cursor del terminal de despliegue. Envuelve en ambos sentidos y
/// nunca puede apuntar fuera del catálogo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LevelSelectionMenu {
    selected_index: usize,
}

impl LevelSelectionMenu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn select_next(&mut self) {
        self.selected_index = (self.selected_index + 1) % level_count();
    }

    pub fn select_previous(&mut self) {
        self.selected_index = if self.selected_index == 0 {
            level_count() - 1
        } else {
            self.selected_index - 1
        };
    }
}

/// Opciones del informe de éxito.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LevelSuccessOption {
    /// Continuar al siguiente sector o, en el último, finalizar
    /// el protocolo. La etiqueta la decide la pantalla.
    #[default]
    Advance,

    BackToTerminal,
}

impl LevelSuccessOption {
    pub fn select_next(&mut self) {
        *self = match *self {
            Self::Advance => Self::BackToTerminal,

            Self::BackToTerminal => Self::Advance,
        };
    }

    pub fn select_previous(&mut self) {
        self.select_next();
    }
}

/// Resultado de confirmar una opción del informe de éxito.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelSuccessOutcome {
    ContinueToNextLevel,
    FinishProtocol,
    ReturnToTerminal,
}

/// Traduce la opción elegida según haya o no un sector siguiente.
///
/// Es pura: el bucle principal solo ejecuta lo que aquí se decide.
pub fn confirm_level_success(
    option: LevelSuccessOption,
    has_next_level: bool,
) -> LevelSuccessOutcome {
    match option {
        LevelSuccessOption::BackToTerminal => LevelSuccessOutcome::ReturnToTerminal,

        LevelSuccessOption::Advance if has_next_level => LevelSuccessOutcome::ContinueToNextLevel,

        LevelSuccessOption::Advance => LevelSuccessOutcome::FinishProtocol,
    }
}

/// Estado al que se pasa al pisar la meta.
///
/// Alcanzar la meta **no** avanza de nivel: eso queda en manos del
/// informe, que espera una confirmación explícita.
pub fn state_after_reaching_goal() -> GameState {
    GameState::LevelSuccess
}

pub struct GameSession {
    current_level_index: usize,
}

impl Default for GameSession {
    fn default() -> Self {
        Self {
            current_level_index: 0,
        }
    }
}

impl GameSession {
    pub fn current_level_path(&self) -> &'static str {
        self.current_level_info().path
    }

    /// Expediente del sector en curso. El índice se mantiene siempre
    /// dentro del catálogo, así que el `unwrap_or` es una red de
    /// seguridad y no una ruta esperada.
    pub fn current_level_info(&self) -> &'static LevelInfo {
        level_info(self.current_level_index).unwrap_or(&LEVELS[0])
    }

    /// Salta directamente a un sector del terminal. Un índice fuera
    /// del catálogo se rechaza sin modificar la sesión.
    pub fn select_level(&mut self, index: usize) -> bool {
        if level_info(index).is_none() {
            return false;
        }

        self.current_level_index = index;

        true
    }

    pub fn has_next_level(&self) -> bool {
        self.current_level_index + 1 < level_count()
    }

    pub fn advance_level(&mut self) -> bool {
        let next_level_index = self.current_level_index + 1;

        if level_info(next_level_index).is_none() {
            return false;
        }

        self.current_level_index = next_level_index;

        true
    }

    pub fn reset(&mut self) {
        self.current_level_index = 0;
    }

    pub fn current_level_number(&self) -> usize {
        self.current_level_index + 1
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GameSession, GameState, LevelSelectionMenu, LevelSuccessOption, LevelSuccessOutcome,
        confirm_level_success, level_count, level_info, state_after_reaching_goal,
    };

    #[test]
    fn the_catalogue_exposes_every_level() {
        assert_eq!(level_count(), 2);

        for index in 0..level_count() {
            let info = level_info(index).expect("todo índice del catálogo debe existir");

            assert!(!info.path.is_empty());
            assert!(!info.sector.is_empty());
            assert!(!info.status.is_empty());
            assert!(!info.risk.is_empty());
            assert!(!info.note.is_empty());
        }
    }

    #[test]
    fn selection_wraps_in_both_directions() {
        let mut menu = LevelSelectionMenu::new();

        assert_eq!(menu.selected_index(), 0);

        menu.select_next();
        assert_eq!(menu.selected_index(), 1);

        // Del último vuelve al primero.
        menu.select_next();
        assert_eq!(menu.selected_index(), 0);

        // Y hacia atrás desde el primero llega al último.
        menu.select_previous();
        assert_eq!(menu.selected_index(), level_count() - 1);
    }

    #[test]
    fn selecting_a_valid_index_updates_the_session() {
        let mut session = GameSession::default();

        assert!(session.select_level(1));

        assert_eq!(session.current_level_number(), 2);
        assert_eq!(session.current_level_number(), 2);
        assert_eq!(session.current_level_path(), "./levels/level_02.txt");

        // Y también se puede volver al primero directamente.
        assert!(session.select_level(0));
        assert_eq!(session.current_level_number(), 1);
    }

    #[test]
    fn an_invalid_index_is_rejected_without_panicking() {
        let mut session = GameSession::default();

        assert!(session.select_level(1));

        assert!(!session.select_level(level_count()));
        assert!(!session.select_level(usize::MAX));

        // La sesión no se movió.
        assert_eq!(session.current_level_number(), 2);

        assert!(level_info(level_count()).is_none());
    }

    #[test]
    fn reaching_the_goal_leads_to_the_success_report() {
        assert_eq!(state_after_reaching_goal(), GameState::LevelSuccess);
    }

    #[test]
    fn the_success_report_does_not_advance_on_its_own() {
        let mut session = GameSession::default();

        let state = state_after_reaching_goal();

        // Entrar al informe no toca la sesión: hace falta confirmar.
        assert_eq!(state, GameState::LevelSuccess);
        assert_eq!(session.current_level_number(), 1);

        // Y solo la confirmación explícita avanza.
        assert_eq!(
            confirm_level_success(LevelSuccessOption::Advance, session.has_next_level()),
            LevelSuccessOutcome::ContinueToNextLevel,
        );

        assert_eq!(session.current_level_number(), 1);

        assert!(session.advance_level());
        assert_eq!(session.current_level_number(), 2);
    }

    #[test]
    fn continuing_advances_to_the_next_level() {
        let mut session = GameSession::default();

        assert!(session.has_next_level());

        assert_eq!(
            confirm_level_success(LevelSuccessOption::Advance, session.has_next_level()),
            LevelSuccessOutcome::ContinueToNextLevel,
        );

        assert!(session.advance_level());

        // En el último sector, continuar pasa a finalizar protocolo.
        assert!(!session.has_next_level());

        assert_eq!(
            confirm_level_success(LevelSuccessOption::Advance, session.has_next_level()),
            LevelSuccessOutcome::FinishProtocol,
        );

        assert!(!session.advance_level());
    }

    #[test]
    fn returning_goes_back_to_the_terminal() {
        for has_next_level in [true, false] {
            assert_eq!(
                confirm_level_success(LevelSuccessOption::BackToTerminal, has_next_level),
                LevelSuccessOutcome::ReturnToTerminal,
            );
        }
    }

    #[test]
    fn the_success_options_alternate() {
        let mut option = LevelSuccessOption::default();

        assert_eq!(option, LevelSuccessOption::Advance);

        option.select_next();
        assert_eq!(option, LevelSuccessOption::BackToTerminal);

        option.select_next();
        assert_eq!(option, LevelSuccessOption::Advance);

        option.select_previous();
        assert_eq!(option, LevelSuccessOption::BackToTerminal);
    }
}
