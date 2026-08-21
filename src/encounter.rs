//! Modelo de encuentros por turnos.
//!
//! Aquí vive el **contenido** (definiciones estáticas) y el
//! **estado/input** (sesión y detección de flancos). El dibujo es
//! responsabilidad de `renderer`, que recibe datos y no conoce la
//! narrativa.

/// Una opción presentada al jugador.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncounterChoice {
    pub label: &'static str,
}

/// Un momento del encuentro: quién habla, qué dice y qué se puede
/// responder.
#[derive(Clone, Copy, Debug)]
pub struct EncounterNode {
    pub text: &'static str,
    pub choices: &'static [EncounterChoice],
}

/// Un encuentro completo. Los nodos se referencian por índice para
/// que más adelante puedan enlazarse sin cambiar el modelo.
#[derive(Clone, Copy, Debug)]
pub struct EncounterDefinition {
    pub entity_name: &'static str,
    pub nodes: &'static [EncounterNode],
    pub start_node: usize,
}

/// Encuentro de demostración. Todavía no representa a ninguna
/// entidad del juego.
pub const DEMO_ENCOUNTER: EncounterDefinition = EncounterDefinition {
    entity_name: "SUJETO NO IDENTIFICADO",
    start_node: 0,
    nodes: &[EncounterNode {
        text: "La figura permanece inmóvil al final del corredor. \
               No respira, no parpadea, y aun así la sensación de \
               ser observado no proviene de ella. Los registros de \
               contención no mencionan nada en este sector.",
        choices: &[
            EncounterChoice { label: "ATACAR" },
            EncounterChoice { label: "OBJETO" },
            EncounterChoice { label: "HUIR" },
        ],
    }],
};

/// Detector de flanco: convierte una tecla mantenida en un único
/// disparo.
///
/// Sin esto, sostener Enter confirmaría una opción en cada frame.
/// La tecla debe soltarse antes de volver a valer.
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

    /// El jugador confirmó una opción.
    Confirmed(EncounterChoice),
}

/// Encuentro en curso: nodo actual, opción resaltada y las compuertas
/// de flanco de cada tecla.
#[derive(Clone, Debug)]
pub struct EncounterSession {
    definition: EncounterDefinition,
    node_index: usize,
    selected_index: usize,
    next_trigger: EdgeTrigger,
    previous_trigger: EdgeTrigger,
    confirm_trigger: EdgeTrigger,
}

impl EncounterSession {
    pub fn new(definition: EncounterDefinition) -> Self {
        let node_index = definition
            .start_node
            .min(definition.nodes.len().saturating_sub(1));

        Self {
            definition,
            node_index,
            selected_index: 0,
            next_trigger: EdgeTrigger::new(),
            previous_trigger: EdgeTrigger::new(),
            confirm_trigger: EdgeTrigger::new(),
        }
    }

    pub fn entity_name(&self) -> &'static str {
        self.definition.entity_name
    }

    /// Nodo actual. `None` solo si la definición viniera vacía.
    pub fn node(&self) -> Option<&'static EncounterNode> {
        self.definition.nodes.get(self.node_index)
    }

    pub fn choices(&self) -> &'static [EncounterChoice] {
        self.node().map(|node| node.choices).unwrap_or(&[])
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
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

    /// Consume el input del frame. Toda tecla pasa por su detector de
    /// flanco, así que mantenerla pulsada no repite la acción.
    pub fn update(&mut self, input: EncounterInput) -> EncounterUpdate {
        let confirm_fired = self.confirm_trigger.update(input.confirm_down);

        let next_fired = self.next_trigger.update(input.next_down);

        let previous_fired = self.previous_trigger.update(input.previous_down);

        if confirm_fired && let Some(choice) = self.choices().get(self.selected_index).copied() {
            return EncounterUpdate::Confirmed(choice);
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
}

#[cfg(test)]
mod tests {
    use super::{DEMO_ENCOUNTER, EdgeTrigger, EncounterInput, EncounterSession, EncounterUpdate};

    fn session() -> EncounterSession {
        EncounterSession::new(DEMO_ENCOUNTER)
    }

    fn press(confirm: bool, next: bool, previous: bool) -> EncounterInput {
        EncounterInput {
            confirm_down: confirm,
            next_down: next,
            previous_down: previous,
        }
    }

    const RELEASED: EncounterInput = EncounterInput {
        next_down: false,
        previous_down: false,
        confirm_down: false,
    };

    #[test]
    fn the_demo_encounter_offers_the_three_expected_choices() {
        let session = session();

        let labels: Vec<&str> = session
            .choices()
            .iter()
            .map(|choice| choice.label)
            .collect();

        assert_eq!(labels, vec!["ATACAR", "OBJETO", "HUIR"]);

        assert_eq!(session.selected_index(), 0);

        assert!(!session.entity_name().is_empty());
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

        // Del último vuelve al primero.
        session.select_next();
        session.select_next();
        assert_eq!(session.selected_index(), 2);

        session.select_next();
        assert_eq!(session.selected_index(), 0);

        // Y hacia atrás desde el primero llega al último.
        session.select_previous();
        assert_eq!(session.selected_index(), 2);

        session.select_previous();
        assert_eq!(session.selected_index(), 1);
    }

    #[test]
    fn navigation_through_input_needs_a_release_between_steps() {
        let mut session = session();

        // Primer frame con la tecla pulsada: avanza una vez.
        assert_eq!(
            session.update(press(false, true, false)),
            EncounterUpdate::SelectionChanged,
        );
        assert_eq!(session.selected_index(), 1);

        // Mantenerla no vuelve a avanzar por muchos frames que pasen.
        for _ in 0..30 {
            assert_eq!(
                session.update(press(false, true, false)),
                EncounterUpdate::Idle,
            );
        }
        assert_eq!(session.selected_index(), 1);

        // Al soltar y volver a pulsar, avanza de nuevo.
        session.update(RELEASED);

        assert_eq!(
            session.update(press(false, true, false)),
            EncounterUpdate::SelectionChanged,
        );
        assert_eq!(session.selected_index(), 2);
    }

    #[test]
    fn a_held_confirmation_only_fires_once() {
        let mut session = session();

        let first = session.update(press(true, false, false));

        assert!(
            matches!(first, EncounterUpdate::Confirmed(choice) if choice.label == "ATACAR"),
            "la primera confirmación debe dispararse: {first:?}",
        );

        // Sostener Enter no confirma una y otra vez.
        for _ in 0..60 {
            assert_eq!(
                session.update(press(true, false, false)),
                EncounterUpdate::Idle
            );
        }

        // Hace falta soltar para volver a confirmar.
        session.update(RELEASED);

        assert!(matches!(
            session.update(press(true, false, false)),
            EncounterUpdate::Confirmed(_),
        ));
    }

    #[test]
    fn confirming_reports_the_highlighted_choice() {
        let mut session = session();

        session.select_next();

        let update = session.update(press(true, false, false));

        assert!(
            matches!(update, EncounterUpdate::Confirmed(choice) if choice.label == "OBJETO"),
            "se confirmó la opción equivocada: {update:?}",
        );
    }

    #[test]
    fn an_edge_trigger_fires_once_per_press() {
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
        DEMO_ENCOUNTER, EncounterInput, EncounterSession, EncounterUpdate, GameplayGate,
        GameplayStep,
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
        let mut session = EncounterSession::new(DEMO_ENCOUNTER);

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
        let mut session = EncounterSession::new(DEMO_ENCOUNTER);

        // Se abrió con S sostenida, pero Enter estaba suelta.
        session.seed_input_state(HOLDING_NEXT);

        // Enter responde de inmediato porque no estaba pulsada.
        assert!(matches!(
            session.update(HOLDING_CONFIRM),
            EncounterUpdate::Confirmed(_),
        ));
    }

    #[test]
    fn seeding_with_nothing_held_leaves_every_key_responsive() {
        let mut session = EncounterSession::new(DEMO_ENCOUNTER);

        session.seed_input_state(NOTHING);

        assert_eq!(
            session.update(HOLDING_NEXT),
            EncounterUpdate::SelectionChanged,
        );

        session.update(NOTHING);

        assert!(matches!(
            session.update(HOLDING_CONFIRM),
            EncounterUpdate::Confirmed(_),
        ));
    }

    /// Confirmar y cerrar con Enter sostenida no puede atravesar al
    /// gameplay: la compuerta sigue cerrada hasta soltarla.
    #[test]
    fn a_held_confirm_does_not_cross_into_gameplay() {
        let mut session = EncounterSession::new(DEMO_ENCOUNTER);

        session.seed_input_state(NOTHING);

        assert!(matches!(
            session.update(HOLDING_CONFIRM),
            EncounterUpdate::Confirmed(_),
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
