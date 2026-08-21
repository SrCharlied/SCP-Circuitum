//! Modelo de encuentros por turnos.
//!
//! Aquí vive el **contenido** (definiciones estáticas) y el
//! **estado/input** (fase, contadores y detección de flancos). El
//! dibujo es responsabilidad de `renderer`, que recibe datos y no
//! conoce ni las fases ni la narrativa.

/// Acción que el jugador puede tomar en su turno.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerAction {
    Attack,
    Item,
    Flee,
    MaintainGaze,
}

/// Momento del turno en que se encuentra el encuentro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterPhase {
    /// El jugador elige entre las acciones disponibles.
    PlayerChoice,

    /// Se muestra el resultado de la acción elegida.
    PlayerResolution,

    /// Responde la entidad.
    EnemyResolution,
}

impl EncounterPhase {
    /// Título del panel de acciones en esta fase.
    ///
    /// Vive aquí, junto a la fase, para que el renderer solo reciba
    /// una cadena y no tenga que conocer `EncounterPhase`.
    pub fn actions_title(&self) -> &'static str {
        match self {
            Self::PlayerChoice => "ACCIONES",

            Self::PlayerResolution => "RESULTADO",

            Self::EnemyResolution => "TURNO ENEMIGO",
        }
    }
}

/// Una opción presentada al jugador.
///
/// `action` es `None` en la indicación de continuar, que no es una
/// acción de combate sino un simple avance de fase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncounterChoice {
    pub label: &'static str,
    pub action: Option<PlayerAction>,
}

/// Texto que produce una acción del jugador.
///
/// El de ataque admite el marcador `{damage}`, que se sustituye por
/// el daño calculado en ese turno.
#[derive(Clone, Copy, Debug)]
pub struct ActionOutcome {
    pub action: PlayerAction,
    pub text: &'static str,
}

/// Un momento del encuentro: qué se narra y qué se puede responder.
#[derive(Clone, Copy, Debug)]
pub struct EncounterNode {
    pub text: &'static str,
    pub choices: &'static [EncounterChoice],
}

/// Un encuentro completo. Todo el texto es dato: ni el renderer ni
/// la sesión llevan narrativa escrita dentro.
#[derive(Clone, Copy, Debug)]
pub struct EncounterDefinition {
    pub entity_name: &'static str,
    pub nodes: &'static [EncounterNode],
    pub start_node: usize,

    /// Resultado de cada acción del jugador.
    pub outcomes: &'static [ActionOutcome],

    /// Respuesta de la entidad por turno. El último entrada se
    /// repite en los turnos siguientes.
    pub enemy_texts: &'static [&'static str],
}

/// Marcador sustituido por el daño en el texto de ataque.
const DAMAGE_PLACEHOLDER: &str = "{damage}";

/// Indicación mostrada mientras se resuelve un turno.
static CONTINUE_PROMPT: [EncounterChoice; 1] = [EncounterChoice {
    label: "CONTINUAR",
    action: None,
}];

/// Encuentro provisional con SCP-173.
pub const SCP_173_ENCOUNTER: EncounterDefinition = EncounterDefinition {
    entity_name: "SCP-173",
    start_node: 0,
    nodes: &[EncounterNode {
        text: "La figura permanece inmóvil al final del corredor. \
               No respira, no parpadea, y aun así la sensación de \
               ser observado no proviene de ella.",
        choices: &[
            EncounterChoice {
                label: "ATACAR",
                action: Some(PlayerAction::Attack),
            },
            EncounterChoice {
                label: "OBJETO",
                action: Some(PlayerAction::Item),
            },
            EncounterChoice {
                label: "HUIR",
                action: Some(PlayerAction::Flee),
            },
            EncounterChoice {
                label: "MANTENER LA MIRADA",
                action: Some(PlayerAction::MaintainGaze),
            },
        ],
    }],
    outcomes: &[
        ActionOutcome {
            action: PlayerAction::Attack,
            text: "Atacas a SCP-173.\n\nDaño infligido: {damage}.\n\nLa figura no reacciona.",
        },
        ActionOutcome {
            action: PlayerAction::Item,
            text: "Revisas tu equipo.\n\nNo llevas nada capaz de contener a la figura.",
        },
        ActionOutcome {
            action: PlayerAction::Flee,
            text: "Tus piernas se tensan.\n\nDarle la espalda no parece una opción.",
        },
        ActionOutcome {
            action: PlayerAction::MaintainGaze,
            text: "Mantienes la vista fija.\n\nTus ojos comienzan a arder.",
        },
    ],
    enemy_texts: &[
        "La figura permanece completamente inmóvil.",
        "Sientes cómo la extraña figura te observa atentamente.",
        "No se mueve.\n\nEl ardor en tus ojos continúa creciendo.",
    ],
};

/// Detector de flanco: convierte una tecla mantenida en un único
/// disparo.
///
/// Sin esto, sostener Enter confirmaría en cada frame. La tecla debe
/// soltarse antes de volver a valer.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeTrigger {
    was_down: bool,
}

impl EdgeTrigger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Devuelve `true` solo en el frame en que la tecla pasa de
    /// suelta a pulsada.
    pub fn update(&mut self, is_down: bool) -> bool {
        let fired = is_down && !self.was_down;

        self.was_down = is_down;

        fired
    }

    /// Fija el estado inicial de la tecla sin disparar.
    ///
    /// Se usa al abrir el encuentro con el estado **real** del
    /// teclado: una tecla sostenida queda bloqueada hasta soltarse,
    /// y una suelta responde a su primera pulsación.
    pub fn seed(&mut self, is_down: bool) {
        self.was_down = is_down;
    }
}

/// Estado de las teclas relevantes en el frame actual.
#[derive(Clone, Copy, Debug, Default)]
pub struct EncounterInput {
    pub next_down: bool,
    pub previous_down: bool,
    pub confirm_down: bool,
}

impl EncounterInput {
    /// Si alguna tecla del encuentro sigue pulsada.
    pub fn any_down(&self) -> bool {
        self.next_down || self.previous_down || self.confirm_down
    }
}

/// Qué puede hacer el gameplay en el frame actual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameplayStep {
    /// Nada se actualiza: el input del encuentro todavía manda.
    Blocked,

    /// Las teclas acaban de soltarse. Sigue bloqueado este frame,
    /// pero hay que reiniciar el reloj antes de reanudar.
    Released,

    /// El gameplay corre con normalidad.
    Running,
}

/// Compuerta que impide que el input de un encuentro se filtre al
/// gameplay al volver al mundo.
///
/// Comprobar solo `game_state == Playing` no basta: una tecla que
/// siga sostenida al cerrar movería al jugador en el mismo frame.
/// La compuerta se arma al salir del encuentro —por F6 hoy, por
/// `ReturnToWorld` más adelante— y no se abre hasta que todas las
/// teclas del encuentro se han soltado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameplayGate {
    /// El gameplay corre.
    #[default]
    Open,

    /// Esperando a que se suelten las teclas del encuentro.
    WaitingForRelease,
}

impl GameplayGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cierra la compuerta al abandonar un encuentro.
    pub fn arm(&mut self) {
        *self = Self::WaitingForRelease;
    }

    /// Avanza la compuerta con el estado del teclado de este frame.
    ///
    /// El frame en que se sueltan las teclas devuelve `Released`, no
    /// `Running`: el gameplay se reanuda hasta el frame siguiente.
    pub fn update(&mut self, input: EncounterInput) -> GameplayStep {
        match self {
            Self::Open => GameplayStep::Running,

            Self::WaitingForRelease => {
                if input.any_down() {
                    GameplayStep::Blocked
                } else {
                    *self = Self::Open;

                    GameplayStep::Released
                }
            }
        }
    }
}

/// Lo que produjo el encuentro en este frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterUpdate {
    /// Nada que reportar.
    Idle,

    /// Cambió la opción resaltada.
    SelectionChanged,

    /// El jugador confirmó una acción y empieza su resolución.
    ActionTaken(PlayerAction),

    /// Se avanzó de fase con CONTINUAR.
    PhaseAdvanced(EncounterPhase),
}

/// Encuentro en curso: fase, contadores, opción resaltada, texto
/// visible y las compuertas de flanco de cada tecla.
#[derive(Clone, Debug)]
pub struct EncounterSession {
    definition: EncounterDefinition,
    node_index: usize,
    phase: EncounterPhase,
    turn_count: usize,
    attack_count: usize,
    selected_index: usize,
    current_text: String,
    next_trigger: EdgeTrigger,
    previous_trigger: EdgeTrigger,
    confirm_trigger: EdgeTrigger,
}

impl EncounterSession {
    pub fn new(definition: EncounterDefinition) -> Self {
        let node_index = definition
            .start_node
            .min(definition.nodes.len().saturating_sub(1));

        let current_text = definition
            .nodes
            .get(node_index)
            .map(|node| node.text.to_string())
            .unwrap_or_default();

        Self {
            definition,
            node_index,
            phase: EncounterPhase::PlayerChoice,
            turn_count: 0,
            attack_count: 0,
            selected_index: 0,
            current_text,
            next_trigger: EdgeTrigger::new(),
            previous_trigger: EdgeTrigger::new(),
            confirm_trigger: EdgeTrigger::new(),
        }
    }

    pub fn entity_name(&self) -> &'static str {
        self.definition.entity_name
    }

    pub fn phase(&self) -> EncounterPhase {
        self.phase
    }

    pub fn turn_count(&self) -> usize {
        self.turn_count
    }

    pub fn attack_count(&self) -> usize {
        self.attack_count
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Texto visible en el panel izquierdo.
    pub fn current_text(&self) -> &str {
        &self.current_text
    }

    /// Título del panel derecho para la fase actual.
    pub fn actions_title(&self) -> &'static str {
        self.phase.actions_title()
    }

    /// Nodo actual. `None` solo si la definición viniera vacía.
    pub fn node(&self) -> Option<&'static EncounterNode> {
        self.definition.nodes.get(self.node_index)
    }

    /// Opciones visibles. Fuera de `PlayerChoice` solo se ofrece
    /// continuar.
    pub fn choices(&self) -> &'static [EncounterChoice] {
        match self.phase {
            EncounterPhase::PlayerChoice => self.node().map(|node| node.choices).unwrap_or(&[]),

            _ => &CONTINUE_PROMPT,
        }
    }

    /// Inicializa los detectores con el estado real del teclado al
    /// abrir el encuentro.
    ///
    /// Solo quedan bloqueadas las teclas que de verdad estaban
    /// sostenidas; las que estaban sueltas responden a su primera
    /// pulsación.
    pub fn seed_input_state(&mut self, input: EncounterInput) {
        self.next_trigger.seed(input.next_down);
        self.previous_trigger.seed(input.previous_down);
        self.confirm_trigger.seed(input.confirm_down);
    }

    pub fn select_next(&mut self) {
        let count = self.choices().len();

        if count == 0 {
            return;
        }

        self.selected_index = (self.selected_index + 1) % count;
    }

    pub fn select_previous(&mut self) {
        let count = self.choices().len();

        if count == 0 {
            return;
        }

        self.selected_index = if self.selected_index == 0 {
            count - 1
        } else {
            self.selected_index - 1
        };
    }

    /// Daño del ataque de este turno.
    ///
    /// Alterna 0 y 1 de forma determinista: el primer ataque hace 0,
    /// el segundo 1, y así sucesivamente.
    fn attack_damage(&self) -> usize {
        self.attack_count % 2
    }

    fn outcome_text(&self, action: PlayerAction) -> String {
        let template = self
            .definition
            .outcomes
            .iter()
            .find(|outcome| outcome.action == action)
            .map(|outcome| outcome.text)
            .unwrap_or_default();

        if action == PlayerAction::Attack {
            template.replace(DAMAGE_PLACEHOLDER, &self.attack_damage().to_string())
        } else {
            template.to_string()
        }
    }

    /// Respuesta de la entidad en el turno indicado. A partir del
    /// último texto definido, se repite.
    fn enemy_text(&self, turn: usize) -> String {
        let texts = self.definition.enemy_texts;

        texts
            .get(turn.saturating_sub(1))
            .or(texts.last())
            .map(|text| text.to_string())
            .unwrap_or_default()
    }

    /// Resuelve la acción elegida y pasa a mostrar su resultado.
    fn take_action(&mut self, action: PlayerAction) {
        // Toda acción consume un turno.
        self.turn_count += 1;

        self.current_text = self.outcome_text(action);

        if action == PlayerAction::Attack {
            self.attack_count += 1;
        }

        self.phase = EncounterPhase::PlayerResolution;

        self.selected_index = 0;
    }

    /// Consume el input del frame. Toda tecla pasa por su detector de
    /// flanco, así que una confirmación mantenida no atraviesa dos
    /// fases.
    pub fn update(&mut self, input: EncounterInput) -> EncounterUpdate {
        let confirm_fired = self.confirm_trigger.update(input.confirm_down);

        let next_fired = self.next_trigger.update(input.next_down);

        let previous_fired = self.previous_trigger.update(input.previous_down);

        if confirm_fired {
            return self.confirm();
        }

        // Navegar solo tiene sentido mientras se elige acción.
        if self.phase != EncounterPhase::PlayerChoice {
            return EncounterUpdate::Idle;
        }

        let mut moved = false;

        if next_fired {
            self.select_next();

            moved = true;
        }

        if previous_fired {
            self.select_previous();

            moved = true;
        }

        if moved {
            EncounterUpdate::SelectionChanged
        } else {
            EncounterUpdate::Idle
        }
    }

    fn confirm(&mut self) -> EncounterUpdate {
        match self.phase {
            EncounterPhase::PlayerChoice => {
                let Some(action) = self
                    .choices()
                    .get(self.selected_index)
                    .and_then(|choice| choice.action)
                else {
                    return EncounterUpdate::Idle;
                };

                self.take_action(action);

                EncounterUpdate::ActionTaken(action)
            }

            EncounterPhase::PlayerResolution => {
                self.phase = EncounterPhase::EnemyResolution;

                self.current_text = self.enemy_text(self.turn_count);

                self.selected_index = 0;

                EncounterUpdate::PhaseAdvanced(self.phase)
            }

            EncounterPhase::EnemyResolution => {
                self.phase = EncounterPhase::PlayerChoice;

                self.selected_index = 0;

                // El texto de la entidad sigue visible hasta que el
                // jugador elija su siguiente acción.
                EncounterUpdate::PhaseAdvanced(self.phase)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EncounterInput, EncounterPhase, EncounterSession, EncounterUpdate, PlayerAction,
        SCP_173_ENCOUNTER,
    };

    fn session() -> EncounterSession {
        EncounterSession::new(SCP_173_ENCOUNTER)
    }

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

    const PREVIOUS: EncounterInput = EncounterInput {
        next_down: false,
        previous_down: true,
        confirm_down: false,
    };

    /// Confirma una vez, soltando la tecla despues.
    fn confirm_once(session: &mut EncounterSession) -> EncounterUpdate {
        let update = session.update(CONFIRM);

        session.update(RELEASED);

        update
    }

    /// Mueve la seleccion hasta el indice pedido.
    fn select(session: &mut EncounterSession, index: usize) {
        while session.selected_index() != index {
            session.update(NEXT);

            session.update(RELEASED);
        }
    }

    /// Ejecuta un turno completo con la accion indicada y regresa a
    /// `PlayerChoice`.
    fn play_turn(session: &mut EncounterSession, index: usize) {
        select(session, index);

        confirm_once(session);

        confirm_once(session);

        confirm_once(session);
    }

    #[test]
    fn the_encounter_starts_in_player_choice_with_four_actions() {
        let session = session();

        assert_eq!(session.phase(), EncounterPhase::PlayerChoice);

        let labels: Vec<&str> = session
            .choices()
            .iter()
            .map(|choice| choice.label)
            .collect();

        assert_eq!(
            labels,
            vec!["ATACAR", "OBJETO", "HUIR", "MANTENER LA MIRADA"],
        );

        assert_eq!(session.selected_index(), 0);

        assert_eq!(session.entity_name(), "SCP-173");

        assert_eq!(session.turn_count(), 0);

        assert_eq!(session.attack_count(), 0);

        assert_eq!(session.actions_title(), "ACCIONES");
    }

    #[test]
    fn every_action_consumes_exactly_one_turn() {
        for index in 0..4 {
            let mut session = session();

            select(&mut session, index);

            confirm_once(&mut session);

            assert_eq!(
                session.turn_count(),
                1,
                "la accion {index} no consumio exactamente un turno",
            );

            // Y las fases de resolucion no vuelven a contarlo.
            confirm_once(&mut session);
            assert_eq!(session.turn_count(), 1);

            confirm_once(&mut session);
            assert_eq!(session.turn_count(), 1);
        }
    }

    #[test]
    fn attacking_alternates_zero_and_one_damage() {
        let mut session = session();

        let expected = [0, 1, 0, 1, 0, 1];

        for (turn, damage) in expected.iter().enumerate() {
            select(&mut session, 0);

            confirm_once(&mut session);

            assert_eq!(
                session.attack_count(),
                turn + 1,
                "attack_count no avanzo en el ataque {turn}",
            );

            assert!(
                session
                    .current_text()
                    .contains(&format!("Daño infligido: {damage}.")),
                "el ataque {turn} debia infligir {damage}: {}",
                session.current_text(),
            );

            confirm_once(&mut session);
            confirm_once(&mut session);
        }
    }

    #[test]
    fn attacking_does_not_change_the_other_counters_out_of_step() {
        let mut session = session();

        play_turn(&mut session, 1);

        // Un objeto no cuenta como ataque.
        assert_eq!(session.attack_count(), 0);
        assert_eq!(session.turn_count(), 1);

        play_turn(&mut session, 0);

        assert_eq!(session.attack_count(), 1);
        assert_eq!(session.turn_count(), 2);
    }

    #[test]
    fn the_item_action_reports_an_empty_inventory() {
        let mut session = session();

        select(&mut session, 1);

        let update = confirm_once(&mut session);

        assert_eq!(update, EncounterUpdate::ActionTaken(PlayerAction::Item));

        assert!(session.current_text().starts_with("Revisas tu equipo."));

        assert!(session.current_text().contains("No llevas nada capaz"));
    }

    #[test]
    fn the_flee_action_refuses_to_turn_around() {
        let mut session = session();

        select(&mut session, 2);

        let update = confirm_once(&mut session);

        assert_eq!(update, EncounterUpdate::ActionTaken(PlayerAction::Flee));

        assert!(session.current_text().starts_with("Tus piernas se tensan."));

        assert!(session.current_text().contains("Darle la espalda"));
    }

    #[test]
    fn the_maintain_gaze_action_burns_the_eyes() {
        let mut session = session();

        select(&mut session, 3);

        let update = confirm_once(&mut session);

        assert_eq!(
            update,
            EncounterUpdate::ActionTaken(PlayerAction::MaintainGaze),
        );

        assert!(
            session
                .current_text()
                .starts_with("Mantienes la vista fija.")
        );

        assert!(session.current_text().contains("comienzan a arder"));
    }

    #[test]
    fn the_resolution_phase_only_offers_continue() {
        let mut session = session();

        confirm_once(&mut session);

        assert_eq!(session.phase(), EncounterPhase::PlayerResolution);

        assert_eq!(session.choices().len(), 1);

        assert_eq!(session.choices()[0].label, "CONTINUAR");

        assert!(session.choices()[0].action.is_none());

        assert_eq!(session.actions_title(), "RESULTADO");
    }

    #[test]
    fn confirming_the_resolution_moves_to_the_enemy_turn() {
        let mut session = session();

        confirm_once(&mut session);

        let update = confirm_once(&mut session);

        assert_eq!(
            update,
            EncounterUpdate::PhaseAdvanced(EncounterPhase::EnemyResolution),
        );

        assert_eq!(session.phase(), EncounterPhase::EnemyResolution);

        assert_eq!(session.choices().len(), 1);

        assert_eq!(session.choices()[0].label, "CONTINUAR");

        assert_eq!(session.actions_title(), "TURNO ENEMIGO");
    }

    #[test]
    fn the_enemy_text_matches_the_turn() {
        let mut session = session();

        let expected = [
            "La figura permanece completamente inmóvil.",
            "Sientes cómo la extraña figura te observa atentamente.",
            "No se mueve.",
            "No se mueve.",
            "No se mueve.",
        ];

        for (turn, text) in expected.iter().enumerate() {
            select(&mut session, 3);

            confirm_once(&mut session);

            confirm_once(&mut session);

            assert_eq!(session.turn_count(), turn + 1);

            assert!(
                session.current_text().starts_with(text),
                "turno {}: se esperaba '{text}' y hubo '{}'",
                turn + 1,
                session.current_text(),
            );

            confirm_once(&mut session);
        }
    }

    #[test]
    fn confirming_the_enemy_turn_returns_to_player_choice() {
        let mut session = session();

        select(&mut session, 2);

        confirm_once(&mut session);
        confirm_once(&mut session);

        let enemy_text = session.current_text().to_string();

        let update = confirm_once(&mut session);

        assert_eq!(
            update,
            EncounterUpdate::PhaseAdvanced(EncounterPhase::PlayerChoice),
        );

        assert_eq!(session.phase(), EncounterPhase::PlayerChoice);

        // La seleccion vuelve al principio.
        assert_eq!(session.selected_index(), 0);

        // Los contadores se conservan.
        assert_eq!(session.turn_count(), 1);

        // El texto de la entidad sigue visible.
        assert_eq!(session.current_text(), enemy_text);

        // Y vuelven a estar las cuatro acciones.
        assert_eq!(session.choices().len(), 4);
    }

    #[test]
    fn the_selection_returns_to_zero_after_a_full_turn() {
        let mut session = session();

        select(&mut session, 3);

        assert_eq!(session.selected_index(), 3);

        confirm_once(&mut session);
        assert_eq!(session.selected_index(), 0);

        confirm_once(&mut session);
        assert_eq!(session.selected_index(), 0);

        confirm_once(&mut session);
        assert_eq!(session.selected_index(), 0);
    }

    #[test]
    fn a_held_confirm_does_not_cross_two_phases() {
        let mut session = session();

        // Primera pulsacion: se resuelve la accion.
        assert_eq!(
            session.update(CONFIRM),
            EncounterUpdate::ActionTaken(PlayerAction::Attack),
        );

        assert_eq!(session.phase(), EncounterPhase::PlayerResolution);

        // Sostenerla no avanza a la fase siguiente.
        for _ in 0..60 {
            assert_eq!(session.update(CONFIRM), EncounterUpdate::Idle);

            assert_eq!(session.phase(), EncounterPhase::PlayerResolution);
        }

        // Hace falta soltar.
        session.update(RELEASED);

        assert_eq!(
            session.update(CONFIRM),
            EncounterUpdate::PhaseAdvanced(EncounterPhase::EnemyResolution),
        );
    }

    #[test]
    fn navigation_does_nothing_outside_player_choice() {
        let mut session = session();

        confirm_once(&mut session);

        assert_eq!(session.phase(), EncounterPhase::PlayerResolution);

        for input in [NEXT, PREVIOUS] {
            assert_eq!(session.update(input), EncounterUpdate::Idle);

            session.update(RELEASED);

            assert_eq!(session.selected_index(), 0);
        }

        confirm_once(&mut session);

        assert_eq!(session.phase(), EncounterPhase::EnemyResolution);

        for input in [NEXT, PREVIOUS] {
            assert_eq!(session.update(input), EncounterUpdate::Idle);

            session.update(RELEASED);

            assert_eq!(session.selected_index(), 0);
        }
    }

    #[test]
    fn moving_down_advances_the_selection() {
        let mut session = session();

        session.select_next();
        assert_eq!(session.selected_index(), 1);

        session.select_next();
        assert_eq!(session.selected_index(), 2);
    }

    #[test]
    fn the_selection_wraps_in_both_directions() {
        let mut session = session();

        let last = session.choices().len() - 1;

        for _ in 0..last {
            session.select_next();
        }
        assert_eq!(session.selected_index(), last);

        session.select_next();
        assert_eq!(session.selected_index(), 0);

        session.select_previous();
        assert_eq!(session.selected_index(), last);

        session.select_previous();
        assert_eq!(session.selected_index(), last - 1);
    }

    #[test]
    fn navigation_through_input_needs_a_release_between_steps() {
        let mut session = session();

        assert_eq!(session.update(NEXT), EncounterUpdate::SelectionChanged);
        assert_eq!(session.selected_index(), 1);

        for _ in 0..30 {
            assert_eq!(session.update(NEXT), EncounterUpdate::Idle);
        }
        assert_eq!(session.selected_index(), 1);

        session.update(RELEASED);

        assert_eq!(session.update(NEXT), EncounterUpdate::SelectionChanged);
        assert_eq!(session.selected_index(), 2);
    }

    #[test]
    fn confirming_reports_the_highlighted_action() {
        let mut session = session();

        select(&mut session, 1);

        assert_eq!(
            session.update(CONFIRM),
            EncounterUpdate::ActionTaken(PlayerAction::Item),
        );
    }

    #[test]
    fn an_edge_trigger_fires_once_per_press() {
        use super::EdgeTrigger;

        let mut trigger = EdgeTrigger::new();

        assert!(trigger.update(true));
        assert!(!trigger.update(true));
        assert!(!trigger.update(true));

        assert!(!trigger.update(false));

        assert!(trigger.update(true));
    }
}

#[cfg(test)]
mod gate_tests {
    use super::{
        EncounterInput, EncounterSession, EncounterUpdate, GameplayGate, GameplayStep,
        SCP_173_ENCOUNTER,
    };
    use crate::maze::Maze;
    use crate::player::Player;
    use crate::scp173::Scp173;
    use nalgebra_glm::Vec2;

    const BLOCK_SIZE: usize = 100;

    const NOTHING: EncounterInput = EncounterInput {
        next_down: false,
        previous_down: false,
        confirm_down: false,
    };

    const HOLDING_NEXT: EncounterInput = EncounterInput {
        next_down: true,
        previous_down: false,
        confirm_down: false,
    };

    const HOLDING_CONFIRM: EncounterInput = EncounterInput {
        next_down: false,
        previous_down: false,
        confirm_down: true,
    };

    /// Compuerta ya armada, como al cerrar el encuentro con F6.
    fn armed() -> GameplayGate {
        let mut gate = GameplayGate::new();

        gate.arm();

        gate
    }

    fn open_room() -> Maze {
        vec![
            vec!['|', '|', '|', '|', '|'],
            vec!['|', ' ', ' ', ' ', '|'],
            vec!['|', ' ', ' ', ' ', '|'],
            vec!['|', ' ', ' ', ' ', '|'],
            vec!['|', '|', '|', '|', '|'],
        ]
    }

    #[test]
    fn closing_the_encounter_blocks_gameplay_on_the_same_frame() {
        let mut gate = armed();

        assert_eq!(gate.update(HOLDING_NEXT), GameplayStep::Blocked);
    }

    #[test]
    fn a_held_key_keeps_the_gate_closed_for_many_frames() {
        let mut gate = armed();

        for frame in 0..120 {
            assert_eq!(
                gate.update(HOLDING_NEXT),
                GameplayStep::Blocked,
                "la compuerta se abrió en el frame {frame} con la tecla sostenida",
            );
        }
    }

    #[test]
    fn releasing_the_keys_does_not_run_gameplay_on_that_frame() {
        let mut gate = armed();

        gate.update(HOLDING_NEXT);

        // El frame de la soltada todavía no corre gameplay.
        assert_eq!(gate.update(NOTHING), GameplayStep::Released);
    }

    #[test]
    fn gameplay_resumes_on_the_following_frame() {
        let mut gate = armed();

        gate.update(HOLDING_NEXT);

        assert_eq!(gate.update(NOTHING), GameplayStep::Released);

        // Y a partir de aquí corre con normalidad.
        assert_eq!(gate.update(NOTHING), GameplayStep::Running);

        assert_eq!(gate.update(HOLDING_NEXT), GameplayStep::Running);
    }

    #[test]
    fn every_encounter_key_holds_the_gate() {
        for input in [
            HOLDING_NEXT,
            HOLDING_CONFIRM,
            EncounterInput {
                next_down: false,
                previous_down: true,
                confirm_down: false,
            },
        ] {
            let mut gate = armed();

            assert_eq!(gate.update(input), GameplayStep::Blocked);

            assert_eq!(gate.update(NOTHING), GameplayStep::Released);
        }
    }

    /// Simula el bucle principal: el jugador solo avanza cuando la
    /// compuerta lo permite.
    #[test]
    fn closing_with_the_key_held_does_not_move_the_player() {
        let mut gate = armed();

        let mut player = Player::new(Vec2::new(250.0, 250.0), 0.0);

        let start = player.pos;

        // Diez frames con S sostenida tras cerrar el encuentro.
        for _ in 0..10 {
            if gate.update(HOLDING_NEXT) == GameplayStep::Running {
                // Desplazamiento que haría `process_events`.
                player.pos.x += 5.0;
            }
        }

        assert_eq!(
            player.pos, start,
            "el jugador se movió con la compuerta cerrada"
        );

        // El frame de la soltada tampoco lo mueve.
        assert_eq!(gate.update(NOTHING), GameplayStep::Released);
        assert_eq!(player.pos, start);

        // El siguiente sí.
        if gate.update(NOTHING) == GameplayStep::Running {
            player.pos.x += 5.0;
        }

        assert_ne!(player.pos, start, "el gameplay no se reanudó");
    }

    #[test]
    fn scp_173_does_not_update_while_the_gate_is_closed() {
        let maze = open_room();

        let player = Player::new(Vec2::new(350.0, 250.0), 0.0);

        let mut scp = Scp173::new(Vec2::new(150.0, 250.0), 120.0, 1.0);

        let start = scp.pos;

        let mut gate = armed();

        for _ in 0..30 {
            if gate.update(HOLDING_NEXT) == GameplayStep::Running {
                scp.update(&maze, &player, BLOCK_SIZE, false, 1.0 / 60.0);
            }
        }

        assert_eq!(scp.pos, start, "SCP-173 avanzó con la compuerta cerrada");

        // Tras soltar y esperar un frame, vuelve a moverse.
        gate.update(NOTHING);

        assert_eq!(gate.update(NOTHING), GameplayStep::Running);

        scp.update(&maze, &player, BLOCK_SIZE, false, 1.0 / 60.0);

        assert_ne!(scp.pos, start, "SCP-173 no reanudó su movimiento");
    }

    #[test]
    fn a_key_held_when_opening_stays_blocked_until_released() {
        let mut session = EncounterSession::new(SCP_173_ENCOUNTER);

        // El jugador abrió el encuentro con S sostenida.
        session.seed_input_state(HOLDING_NEXT);

        for _ in 0..20 {
            assert_eq!(session.update(HOLDING_NEXT), EncounterUpdate::Idle);
        }

        assert_eq!(session.selected_index(), 0);

        // Al soltar y volver a pulsar, responde.
        session.update(NOTHING);

        assert_eq!(
            session.update(HOLDING_NEXT),
            EncounterUpdate::SelectionChanged,
        );
        assert_eq!(session.selected_index(), 1);
    }

    #[test]
    fn a_key_released_when_opening_works_on_its_first_press() {
        let mut session = EncounterSession::new(SCP_173_ENCOUNTER);

        // Se abrió con S sostenida, pero Enter estaba suelta.
        session.seed_input_state(HOLDING_NEXT);

        // Enter responde de inmediato porque no estaba pulsada.
        assert!(matches!(
            session.update(HOLDING_CONFIRM),
            EncounterUpdate::ActionTaken(_),
        ));
    }

    #[test]
    fn seeding_with_nothing_held_leaves_every_key_responsive() {
        let mut session = EncounterSession::new(SCP_173_ENCOUNTER);

        session.seed_input_state(NOTHING);

        assert_eq!(
            session.update(HOLDING_NEXT),
            EncounterUpdate::SelectionChanged,
        );

        session.update(NOTHING);

        assert!(matches!(
            session.update(HOLDING_CONFIRM),
            EncounterUpdate::ActionTaken(_),
        ));
    }

    /// Confirmar y cerrar con Enter sostenida no puede atravesar al
    /// gameplay: la compuerta sigue cerrada hasta soltarla.
    #[test]
    fn a_held_confirm_does_not_cross_into_gameplay() {
        let mut session = EncounterSession::new(SCP_173_ENCOUNTER);

        session.seed_input_state(NOTHING);

        assert!(matches!(
            session.update(HOLDING_CONFIRM),
            EncounterUpdate::ActionTaken(_),
        ));

        // Sigue sostenida y el jugador cierra el encuentro.
        let mut gate = armed();

        for _ in 0..15 {
            assert_eq!(gate.update(HOLDING_CONFIRM), GameplayStep::Blocked);
        }

        assert_eq!(gate.update(NOTHING), GameplayStep::Released);
        assert_eq!(gate.update(NOTHING), GameplayStep::Running);
    }
}
