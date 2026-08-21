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

/// Acción con la que responde la entidad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyAction {
    Observe,
    LethalAttack,
}

/// Respuesta de la entidad: qué hace y cómo se narra.
#[derive(Clone, Copy, Debug)]
pub struct EnemyTurn {
    pub action: EnemyAction,
    pub text: &'static str,
}

/// Momento del turno en que se encuentra el encuentro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterPhase {
    /// El jugador elige entre las acciones disponibles.
    PlayerChoice,

    /// Se muestra el resultado de la acción elegida.
    PlayerResolution,

    /// El cuerpo del sujeto deja de obedecer. No se puede cancelar.
    ForcedSequence,

    /// Responde la entidad.
    EnemyResolution,

    /// Instante del impacto. Deja el desenlace en pantalla antes de
    /// cerrar el encuentro, y marca el punto donde más adelante irá
    /// el sonido.
    DeathBeat,
}

impl EncounterPhase {
    /// Título del panel de acciones en esta fase.
    ///
    /// Vive aquí, junto a la fase, para que el renderer solo reciba
    /// una cadena y no tenga que conocer `EncounterPhase`.
    pub fn actions_title(&self) -> &'static str {
        match self {
            Self::PlayerChoice => "ACCIONES",

            Self::PlayerResolution | Self::ForcedSequence => "RESULTADO",

            Self::EnemyResolution => "ACCIÓN ENEMIGA",

            Self::DeathBeat => "IMPACTO",
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

    /// Respuesta de la entidad en los turnos normales. Su longitud
    /// define cuántos turnos hay antes de la secuencia forzada.
    pub enemy_turns: &'static [EnemyTurn],

    /// Respuesta cuando el jugador intenta huir.
    pub flee_response: EnemyTurn,

    /// Pasos de la secuencia de cansancio y parpadeo.
    pub forced_steps: &'static [&'static str],

    /// Respuesta de la entidad al terminar la secuencia forzada.
    pub forced_response: EnemyTurn,

    /// Texto que queda visible al morir.
    pub death_text: &'static str,
}

impl EncounterDefinition {
    /// Comprueba que la definición sea jugable de principio a fin.
    ///
    /// Las definiciones son datos estáticos escritos por el
    /// programador: un fallo aquí es un error de programación, no una
    /// situación que el jugador pueda provocar. Por eso conviene
    /// detectarlo al arrancar y con un mensaje concreto, en lugar de
    /// dejar que aparezca un panel vacío a mitad de partida.
    pub fn validate(&self) -> Result<(), String> {
        let name = self.entity_name;

        if self.nodes.is_empty() {
            return Err(format!("el encuentro '{name}' no define ningún nodo"));
        }

        if self.start_node >= self.nodes.len() {
            return Err(format!(
                "el encuentro '{name}' arranca en el nodo {} y solo hay {}",
                self.start_node,
                self.nodes.len(),
            ));
        }

        // Toda acción que se le ofrezca al jugador necesita su texto
        // de resultado, o el turno se resolvería en blanco.
        for node in self.nodes {
            for choice in node.choices {
                if let Some(action) = choice.action
                    && !self.outcomes.iter().any(|outcome| outcome.action == action)
                {
                    return Err(format!(
                        "el encuentro '{name}' ofrece '{}' pero no define el resultado de {action:?}",
                        choice.label,
                    ));
                }
            }
        }

        if self.forced_steps.is_empty() {
            return Err(format!(
                "el encuentro '{name}' no define la secuencia forzada",
            ));
        }

        if self.flee_response.action != EnemyAction::LethalAttack {
            return Err(format!(
                "la respuesta a huir de '{name}' debe ser un ataque letal",
            ));
        }

        if self.forced_response.action != EnemyAction::LethalAttack {
            return Err(format!(
                "la respuesta tras el parpadeo de '{name}' debe ser un ataque letal",
            ));
        }

        if self.death_text.trim().is_empty() {
            return Err(format!(
                "el encuentro '{name}' no define el texto de muerte",
            ));
        }

        Ok(())
    }
}

/// Marcador sustituido por el daño en el texto de ataque.
const DAMAGE_PLACEHOLDER: &str = "{damage}";

/// Indicación mostrada mientras se resuelve un turno.
static CONTINUE_PROMPT: [EncounterChoice; 1] = [EncounterChoice {
    label: "CONTINUAR",
    action: None,
}];

/// Franjas que anuncian la acción de la entidad. Ninguna es una
/// decisión del jugador: confirmar solo avanza la resolución.
static OBSERVE_PROMPT: [EncounterChoice; 1] = [EncounterChoice {
    label: "OBSERVAR",
    action: None,
}];

static ATTACK_PROMPT: [EncounterChoice; 1] = [EncounterChoice {
    label: "ATACAR",
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
    enemy_turns: &[
        EnemyTurn {
            action: EnemyAction::Observe,
            text: "La figura permanece completamente inmóvil.",
        },
        EnemyTurn {
            action: EnemyAction::Observe,
            text: "Sientes cómo la extraña figura te observa atentamente.",
        },
        EnemyTurn {
            action: EnemyAction::Observe,
            text: "No se mueve.\n\nEl ardor en tus ojos continúa creciendo.",
        },
    ],
    flee_response: EnemyTurn {
        action: EnemyAction::LethalAttack,
        text: "Apartas la mirada al intentar huir.\n\nAlgo se mueve detrás de ti.",
    },
    forced_steps: &[
        "El cansancio se ha apoderado de tu cuerpo.",
        "Tus párpados comienzan a cerrarse.",
        "Sin pensarlo, parpadeas.",
    ],
    forced_response: EnemyTurn {
        action: EnemyAction::LethalAttack,
        text: "Durante un instante, SCP-173 desaparece de tu vista.",
    },
    death_text: "CRACK.",
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

    /// El ataque letal se resolvió: el sujeto ha muerto.
    PlayerDeath,
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

    /// Acción con la que responde la entidad en la resolución
    /// enemiga en curso.
    enemy_action: Option<EnemyAction>,

    /// Paso actual de la secuencia forzada.
    forced_step: usize,

    /// Última acción elegida: decide si la respuesta es letal.
    last_action: Option<PlayerAction>,

    next_trigger: EdgeTrigger,
    previous_trigger: EdgeTrigger,
    confirm_trigger: EdgeTrigger,
}

impl EncounterSession {
    /// # Pánico
    ///
    /// Si la definición no supera [`EncounterDefinition::validate`].
    /// Es un fallo del contenido estático, así que interrumpir el
    /// arranque con un mensaje claro es preferible a arrastrar el
    /// problema hasta la partida.
    pub fn new(definition: EncounterDefinition) -> Self {
        if let Err(error) = definition.validate() {
            panic!("Definición de encuentro inválida: {error}");
        }

        // Validado justo arriba: el nodo inicial existe.
        let node_index = definition.start_node;

        let current_text = definition.nodes[node_index].text.to_string();

        Self {
            definition,
            node_index,
            phase: EncounterPhase::PlayerChoice,
            turn_count: 0,
            attack_count: 0,
            selected_index: 0,
            current_text,
            enemy_action: None,
            forced_step: 0,
            last_action: None,
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

            // La franja de la fase enemiga es informativa: anuncia
            // lo que hace la entidad, no una decisión del jugador.
            EncounterPhase::EnemyResolution => match self.enemy_action {
                Some(EnemyAction::Observe) => &OBSERVE_PROMPT,

                Some(EnemyAction::LethalAttack) => &ATTACK_PROMPT,

                None => &CONTINUE_PROMPT,
            },

            _ => &CONTINUE_PROMPT,
        }
    }

    pub fn enemy_action(&self) -> Option<EnemyAction> {
        self.enemy_action
    }

    pub fn forced_step(&self) -> usize {
        self.forced_step
    }

    /// Si la muerte ya es inevitable.
    ///
    /// Una vez dentro de la secuencia forzada o del ataque letal, el
    /// cierre provisional con F6 no puede cancelarla.
    pub fn is_lethal_locked(&self) -> bool {
        match self.phase {
            EncounterPhase::ForcedSequence | EncounterPhase::DeathBeat => true,

            EncounterPhase::EnemyResolution => self.enemy_action == Some(EnemyAction::LethalAttack),

            // El desenlace ya está comprometido antes de verse: tras
            // resolver una huida, o la acción que agota los turnos
            // normales, la única continuación posible es letal.
            EncounterPhase::PlayerResolution => {
                self.last_action == Some(PlayerAction::Flee)
                    || self.turn_count > self.definition.enemy_turns.len()
            }

            EncounterPhase::PlayerChoice => false,
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

    /// Entra en la fase enemiga con la respuesta indicada.
    fn begin_enemy_turn(&mut self, turn: EnemyTurn) {
        self.phase = EncounterPhase::EnemyResolution;

        self.enemy_action = Some(turn.action);

        self.current_text = turn.text.to_string();

        self.selected_index = 0;
    }

    /// Entra en la secuencia de cansancio y parpadeo.
    fn begin_forced_sequence(&mut self) {
        self.phase = EncounterPhase::ForcedSequence;

        self.enemy_action = None;

        self.forced_step = 0;

        self.current_text = self
            .definition
            .forced_steps
            .first()
            .map(|text| text.to_string())
            .unwrap_or_default();

        self.selected_index = 0;
    }

    /// Resuelve la acción elegida y pasa a mostrar su resultado.
    fn take_action(&mut self, action: PlayerAction) {
        // Toda acción consume un turno.
        self.turn_count += 1;

        self.current_text = self.outcome_text(action);

        if action == PlayerAction::Attack {
            self.attack_count += 1;
        }

        self.last_action = Some(action);

        self.phase = EncounterPhase::PlayerResolution;

        self.enemy_action = None;

        self.selected_index = 0;
    }

    /// Consume el input del frame. Toda tecla pasa por su detector de
    /// flanco, así que una confirmación mantenida no atraviesa dos
    /// fases.
    pub fn update(&mut self, input: EncounterInput) -> EncounterUpdate {
        let confirm_fired = self.confirm_trigger.update(input.confirm_down);

        let next_fired = self.next_trigger.update(input.next_down);

        let previous_fired = self.previous_trigger.update(input.previous_down);

        // Confirmar manda sobre navegar, y lo hace con la selección
        // que estaba activa al empezar el frame. Los flancos de
        // navegación ya se consumieron arriba, así que se descartan
        // en lugar de aplicarse ahora o de guardarse para después.
        if confirm_fired {
            return self.confirm();
        }

        // Navegar solo tiene sentido mientras se elige acción.
        if self.phase != EncounterPhase::PlayerChoice {
            return EncounterUpdate::Idle;
        }

        // Subir y bajar en el mismo frame se anulan. Aplicar ambas
        // dejaría el índice donde estaba por pura cancelación
        // modular, pero se reportaría un cambio que no ocurrió.
        if next_fired && previous_fired {
            return EncounterUpdate::Idle;
        }

        if next_fired {
            self.select_next();

            return EncounterUpdate::SelectionChanged;
        }

        if previous_fired {
            self.select_previous();

            return EncounterUpdate::SelectionChanged;
        }

        EncounterUpdate::Idle
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
                // Apartar la mirada para huir es letal de inmediato.
                if self.last_action == Some(PlayerAction::Flee) {
                    self.begin_enemy_turn(self.definition.flee_response);

                    return EncounterUpdate::PhaseAdvanced(self.phase);
                }

                // La cantidad de turnos normales la define el propio
                // catálogo de respuestas de la entidad.
                match self.definition.enemy_turns.get(self.turn_count - 1) {
                    Some(turn) => self.begin_enemy_turn(*turn),

                    // Agotados los turnos normales, el cuerpo cede.
                    None => self.begin_forced_sequence(),
                }

                EncounterUpdate::PhaseAdvanced(self.phase)
            }

            EncounterPhase::ForcedSequence => {
                self.forced_step += 1;

                match self.definition.forced_steps.get(self.forced_step) {
                    Some(text) => {
                        self.current_text = text.to_string();
                    }

                    // Tras el último paso llega el ataque letal.
                    None => self.begin_enemy_turn(self.definition.forced_response),
                }

                EncounterUpdate::PhaseAdvanced(self.phase)
            }

            EncounterPhase::EnemyResolution => {
                if self.enemy_action == Some(EnemyAction::LethalAttack) {
                    // El impacto se muestra antes de cerrar: la
                    // muerte se reporta en la confirmación siguiente.
                    self.phase = EncounterPhase::DeathBeat;

                    self.current_text = self.definition.death_text.to_string();

                    self.selected_index = 0;

                    return EncounterUpdate::PhaseAdvanced(self.phase);
                }

                self.phase = EncounterPhase::PlayerChoice;

                self.enemy_action = None;

                self.selected_index = 0;

                // El texto de la entidad sigue visible hasta que el
                // jugador elija su siguiente acción.
                EncounterUpdate::PhaseAdvanced(self.phase)
            }

            // El golpe ya está en pantalla: confirmarlo cierra el
            // encuentro. La fase no cambia, así que "CRACK." sigue
            // visible mientras el bucle principal pasa a la derrota.
            EncounterPhase::DeathBeat => EncounterUpdate::PlayerDeath,
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

        // Solo hay tres turnos normales: el cuarto entra en la
        // secuencia forzada, que ya cubre `lethal_tests`.
        let expected = [0, 1, 0];

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

        // La franja anuncia lo que hace la entidad.
        assert_eq!(session.choices()[0].label, "OBSERVAR");

        assert_eq!(session.actions_title(), "ACCIÓN ENEMIGA");
    }

    #[test]
    fn the_enemy_text_matches_the_turn() {
        let mut session = session();

        let expected = [
            "La figura permanece completamente inmóvil.",
            "Sientes cómo la extraña figura te observa atentamente.",
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

        // Huir es letal: para volver al turno del jugador hace falta
        // una acción normal.
        select(&mut session, 3);

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

#[cfg(test)]
mod lethal_tests {
    use super::{
        EncounterInput, EncounterPhase, EncounterSession, EncounterUpdate, EnemyAction,
        PlayerAction, SCP_173_ENCOUNTER,
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

    fn session() -> EncounterSession {
        EncounterSession::new(SCP_173_ENCOUNTER)
    }

    fn confirm_once(session: &mut EncounterSession) -> EncounterUpdate {
        let update = session.update(CONFIRM);

        session.update(RELEASED);

        update
    }

    fn select(session: &mut EncounterSession, index: usize) {
        while session.selected_index() != index {
            session.update(NEXT);

            session.update(RELEASED);
        }
    }

    /// Indices de las acciones en el nodo de eleccion.
    const ATTACK: usize = 0;
    const ITEM: usize = 1;
    const FLEE: usize = 2;
    const GAZE: usize = 3;

    /// Juega un turno completo con una accion no letal.
    fn play_safe_turn(session: &mut EncounterSession, index: usize) {
        select(session, index);

        confirm_once(session);

        confirm_once(session);

        confirm_once(session);
    }

    #[test]
    fn the_first_three_turns_answer_with_observe() {
        let mut session = session();

        let expected = [
            "La figura permanece completamente inmóvil.",
            "Sientes cómo la extraña figura te observa atentamente.",
            "No se mueve.",
        ];

        for (turn, text) in expected.iter().enumerate() {
            select(&mut session, GAZE);

            confirm_once(&mut session);

            confirm_once(&mut session);

            assert_eq!(session.phase(), EncounterPhase::EnemyResolution);

            assert_eq!(
                session.enemy_action(),
                Some(EnemyAction::Observe),
                "el turno {} no respondio con Observe",
                turn + 1,
            );

            assert!(session.current_text().starts_with(text));

            assert_eq!(session.actions_title(), "ACCIÓN ENEMIGA");

            assert_eq!(session.choices().len(), 1);

            assert_eq!(session.choices()[0].label, "OBSERVAR");

            confirm_once(&mut session);
        }
    }

    #[test]
    fn observe_returns_to_player_choice_without_killing() {
        let mut session = session();

        for turn in 1..=3 {
            select(&mut session, ITEM);

            confirm_once(&mut session);

            confirm_once(&mut session);

            let update = confirm_once(&mut session);

            assert_eq!(
                update,
                EncounterUpdate::PhaseAdvanced(EncounterPhase::PlayerChoice),
                "Observe no devolvio el control en el turno {turn}",
            );

            assert_eq!(session.phase(), EncounterPhase::PlayerChoice);

            assert_eq!(session.choices().len(), 4);
        }
    }

    #[test]
    fn fleeing_triggers_the_lethal_attack() {
        let mut session = session();

        select(&mut session, FLEE);

        // La resolucion del jugador conserva su texto.
        let update = confirm_once(&mut session);

        assert_eq!(update, EncounterUpdate::ActionTaken(PlayerAction::Flee));

        assert!(session.current_text().starts_with("Tus piernas se tensan."));

        // Al continuar llega el ataque letal.
        confirm_once(&mut session);

        assert_eq!(session.phase(), EncounterPhase::EnemyResolution);

        assert_eq!(session.enemy_action(), Some(EnemyAction::LethalAttack));

        assert!(
            session
                .current_text()
                .starts_with("Apartas la mirada al intentar huir.")
        );

        assert!(
            session
                .current_text()
                .contains("Algo se mueve detrás de ti.")
        );

        assert_eq!(session.choices()[0].label, "ATACAR");
    }

    #[test]
    fn confirming_the_lethal_attack_reports_the_death() {
        let mut session = session();

        select(&mut session, FLEE);

        confirm_once(&mut session);

        confirm_once(&mut session);

        // Confirmar el ataque deja el impacto en pantalla; todavia no
        // reporta la muerte.
        let update = confirm_once(&mut session);

        assert_eq!(
            update,
            EncounterUpdate::PhaseAdvanced(EncounterPhase::DeathBeat),
        );

        assert_eq!(session.current_text(), "CRACK.");

        // Es la confirmacion siguiente la que cierra el encuentro.
        assert_eq!(confirm_once(&mut session), EncounterUpdate::PlayerDeath);

        assert_eq!(session.current_text(), "CRACK.");
    }

    #[test]
    fn the_fourth_turn_enters_the_forced_sequence() {
        let mut session = session();

        for _ in 0..3 {
            play_safe_turn(&mut session, ATTACK);
        }

        assert_eq!(session.turn_count(), 3);

        // Cuarta accion.
        select(&mut session, ATTACK);

        confirm_once(&mut session);

        assert_eq!(session.turn_count(), 4);

        assert_eq!(session.phase(), EncounterPhase::PlayerResolution);

        // Al continuar ya no hay Observe.
        confirm_once(&mut session);

        assert_eq!(session.phase(), EncounterPhase::ForcedSequence);

        assert_eq!(session.enemy_action(), None);

        assert_eq!(session.forced_step(), 0);
    }

    #[test]
    fn the_three_forced_texts_appear_in_order() {
        let mut session = session();

        for _ in 0..3 {
            play_safe_turn(&mut session, ITEM);
        }

        select(&mut session, ITEM);

        confirm_once(&mut session);

        confirm_once(&mut session);

        let expected = [
            "El cansancio se ha apoderado de tu cuerpo.",
            "Tus párpados comienzan a cerrarse.",
            "Sin pensarlo, parpadeas.",
        ];

        for (step, text) in expected.iter().enumerate() {
            assert_eq!(session.phase(), EncounterPhase::ForcedSequence);

            assert_eq!(session.forced_step(), step);

            assert_eq!(session.current_text(), *text);

            // Cada paso solo ofrece continuar.
            assert_eq!(session.choices().len(), 1);

            assert_eq!(session.choices()[0].label, "CONTINUAR");

            confirm_once(&mut session);
        }
    }

    #[test]
    fn a_held_confirm_does_not_skip_forced_steps() {
        let mut session = session();

        for _ in 0..3 {
            play_safe_turn(&mut session, GAZE);
        }

        select(&mut session, GAZE);

        confirm_once(&mut session);

        confirm_once(&mut session);

        assert_eq!(session.forced_step(), 0);

        // Sostener Enter no avanza mas alla del primer paso.
        session.update(CONFIRM);

        assert_eq!(session.forced_step(), 1);

        for _ in 0..60 {
            assert_eq!(session.update(CONFIRM), EncounterUpdate::Idle);

            assert_eq!(session.forced_step(), 1);
        }

        session.update(RELEASED);

        session.update(CONFIRM);

        assert_eq!(session.forced_step(), 2);
    }

    #[test]
    fn the_blink_is_followed_by_the_lethal_attack() {
        let mut session = session();

        for _ in 0..3 {
            play_safe_turn(&mut session, ATTACK);
        }

        select(&mut session, ATTACK);

        confirm_once(&mut session);

        confirm_once(&mut session);

        // Tres pasos forzados.
        confirm_once(&mut session);
        confirm_once(&mut session);
        confirm_once(&mut session);

        assert_eq!(session.phase(), EncounterPhase::EnemyResolution);

        assert_eq!(session.enemy_action(), Some(EnemyAction::LethalAttack));

        assert_eq!(session.actions_title(), "ACCIÓN ENEMIGA");

        assert_eq!(session.choices()[0].label, "ATACAR");

        assert_eq!(
            session.current_text(),
            "Durante un instante, SCP-173 desaparece de tu vista.",
        );

        // Confirmarlo muestra el impacto...
        assert_eq!(
            confirm_once(&mut session),
            EncounterUpdate::PhaseAdvanced(EncounterPhase::DeathBeat),
        );

        assert_eq!(session.current_text(), "CRACK.");

        // ...y la confirmacion siguiente cierra el encuentro.
        assert_eq!(confirm_once(&mut session), EncounterUpdate::PlayerDeath);
    }

    #[test]
    fn the_encounter_can_be_closed_during_normal_phases() {
        let mut session = session();

        // Eleccion.
        assert!(!session.is_lethal_locked());

        // Resolucion del jugador.
        select(&mut session, ATTACK);
        confirm_once(&mut session);
        assert_eq!(session.phase(), EncounterPhase::PlayerResolution);
        assert!(!session.is_lethal_locked());

        // Turno enemigo con Observe.
        confirm_once(&mut session);
        assert_eq!(session.enemy_action(), Some(EnemyAction::Observe));
        assert!(!session.is_lethal_locked());
    }

    #[test]
    fn the_forced_sequence_cannot_be_cancelled() {
        let mut session = session();

        for _ in 0..3 {
            play_safe_turn(&mut session, ATTACK);
        }

        select(&mut session, ATTACK);
        confirm_once(&mut session);
        confirm_once(&mut session);

        // Los tres pasos estan bloqueados.
        for _ in 0..3 {
            assert_eq!(session.phase(), EncounterPhase::ForcedSequence);

            assert!(
                session.is_lethal_locked(),
                "la secuencia forzada deberia estar bloqueada",
            );

            confirm_once(&mut session);
        }

        // Y el ataque letal tambien.
        assert!(session.is_lethal_locked());
    }

    #[test]
    fn the_lethal_attack_from_fleeing_cannot_be_cancelled() {
        let mut session = session();

        select(&mut session, FLEE);

        confirm_once(&mut session);

        // El desenlace ya esta comprometido: la resolucion de huir
        // solo puede continuar hacia el ataque letal.
        assert!(session.is_lethal_locked());

        confirm_once(&mut session);

        assert!(session.is_lethal_locked());
    }

    #[test]
    fn there_is_no_victory_outcome() {
        // El unico desenlace del encuentro es la muerte: ninguna
        // accion devuelve algo distinto de avance o muerte.
        let mut session = session();

        let mut updates = Vec::new();

        for _ in 0..40 {
            let update = session.update(CONFIRM);

            session.update(RELEASED);

            updates.push(update);

            if update == EncounterUpdate::PlayerDeath {
                break;
            }
        }

        assert!(
            updates.contains(&EncounterUpdate::PlayerDeath),
            "el encuentro nunca termino en muerte",
        );

        // Y nada indica una victoria: tras morir, el texto es CRACK.
        assert_eq!(session.current_text(), "CRACK.");
    }

    #[test]
    fn a_fresh_session_resets_the_whole_encounter() {
        let mut session = session();

        for _ in 0..3 {
            play_safe_turn(&mut session, ATTACK);
        }

        assert_eq!(session.turn_count(), 3);

        assert_eq!(session.attack_count(), 3);

        // Reintentar crea una sesion limpia.
        let fresh = EncounterSession::new(SCP_173_ENCOUNTER);

        assert_eq!(fresh.phase(), EncounterPhase::PlayerChoice);
        assert_eq!(fresh.turn_count(), 0);
        assert_eq!(fresh.attack_count(), 0);
        assert_eq!(fresh.forced_step(), 0);
        assert_eq!(fresh.enemy_action(), None);
        assert_eq!(fresh.selected_index(), 0);
        assert!(!fresh.is_lethal_locked());
        assert_eq!(fresh.choices().len(), 4);
    }
}

#[cfg(test)]
mod input_precedence_tests {
    use super::{
        EncounterInput, EncounterPhase, EncounterSession, EncounterUpdate, PlayerAction,
        SCP_173_ENCOUNTER,
    };

    const RELEASED: EncounterInput = EncounterInput {
        next_down: false,
        previous_down: false,
        confirm_down: false,
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

    /// Arriba y abajo pulsadas a la vez.
    const BOTH_DIRECTIONS: EncounterInput = EncounterInput {
        next_down: true,
        previous_down: true,
        confirm_down: false,
    };

    /// Confirmar y bajar a la vez.
    const CONFIRM_AND_NEXT: EncounterInput = EncounterInput {
        next_down: true,
        previous_down: false,
        confirm_down: true,
    };

    /// Confirmar y subir a la vez.
    const CONFIRM_AND_PREVIOUS: EncounterInput = EncounterInput {
        next_down: false,
        previous_down: true,
        confirm_down: true,
    };

    /// Las tres a la vez.
    const EVERYTHING: EncounterInput = EncounterInput {
        next_down: true,
        previous_down: true,
        confirm_down: true,
    };

    fn session() -> EncounterSession {
        EncounterSession::new(SCP_173_ENCOUNTER)
    }

    fn select(session: &mut EncounterSession, index: usize) {
        while session.selected_index() != index {
            session.update(NEXT);

            session.update(RELEASED);
        }
    }

    // ----- Confirmar tiene prioridad -----

    #[test]
    fn confirming_wins_over_moving_down_in_the_same_frame() {
        let mut session = session();

        // ATACAR resaltado al empezar el frame.
        assert_eq!(session.selected_index(), 0);

        let update = session.update(CONFIRM_AND_NEXT);

        // Se confirma la acción que estaba activa, no la siguiente.
        assert_eq!(update, EncounterUpdate::ActionTaken(PlayerAction::Attack));

        assert_eq!(session.phase(), EncounterPhase::PlayerResolution);
    }

    #[test]
    fn confirming_wins_over_moving_up_in_the_same_frame() {
        let mut session = session();

        // OBJETO resaltado al empezar el frame.
        select(&mut session, 1);

        let update = session.update(CONFIRM_AND_PREVIOUS);

        // No se confirma ATACAR, que sería la opción anterior.
        assert_eq!(update, EncounterUpdate::ActionTaken(PlayerAction::Item));
    }

    #[test]
    fn confirming_uses_the_selection_from_the_start_of_the_frame() {
        // Se comprueba en las cuatro posiciones, para que no dependa
        // de dónde esté el cursor.
        let expected = [
            PlayerAction::Attack,
            PlayerAction::Item,
            PlayerAction::Flee,
            PlayerAction::MaintainGaze,
        ];

        for (index, action) in expected.iter().enumerate() {
            for input in [CONFIRM_AND_NEXT, CONFIRM_AND_PREVIOUS, EVERYTHING] {
                let mut session = session();

                select(&mut session, index);

                assert_eq!(
                    session.update(input),
                    EncounterUpdate::ActionTaken(*action),
                    "la selección {index} cambió antes de confirmarse",
                );
            }
        }
    }

    #[test]
    fn the_navigation_edge_is_consumed_when_confirming() {
        let mut session = session();

        // Confirmar mientras se baja: el flanco de bajar se gasta.
        session.update(CONFIRM_AND_NEXT);

        // Con la tecla aún pulsada no queda un movimiento pendiente
        // que se aplique al volver a elegir.
        session.update(NEXT);
        session.update(NEXT);

        assert_eq!(session.selected_index(), 0);
    }

    // ----- Direcciones opuestas se anulan -----

    #[test]
    fn opposite_directions_cancel_each_other() {
        let mut session = session();

        let update = session.update(BOTH_DIRECTIONS);

        assert_eq!(
            update,
            EncounterUpdate::Idle,
            "pulsar ambas direcciones no debe reportar un cambio",
        );

        assert_eq!(session.selected_index(), 0);
    }

    #[test]
    fn opposite_directions_cancel_from_any_position() {
        for index in 0..4 {
            let mut session = session();

            select(&mut session, index);

            assert_eq!(session.update(BOTH_DIRECTIONS), EncounterUpdate::Idle);

            assert_eq!(
                session.selected_index(),
                index,
                "la selección se movió desde la posición {index}",
            );
        }
    }

    #[test]
    fn cancelled_directions_still_consume_their_edges() {
        let mut session = session();

        assert_eq!(session.update(BOTH_DIRECTIONS), EncounterUpdate::Idle);

        // Soltar solo una: la otra sigue pulsada y ya gastó su flanco,
        // así que no debe moverse nada.
        assert_eq!(session.update(NEXT), EncounterUpdate::Idle);

        assert_eq!(session.selected_index(), 0);

        // Tras soltar del todo, cada dirección vuelve a responder.
        session.update(RELEASED);

        assert_eq!(session.update(NEXT), EncounterUpdate::SelectionChanged,);

        assert_eq!(session.selected_index(), 1);
    }

    #[test]
    fn releasing_one_direction_lets_the_other_act_again() {
        let mut session = session();

        session.update(BOTH_DIRECTIONS);

        // Se suelta bajar y se mantiene subir: subir ya gastó su
        // flanco, así que hace falta volver a pulsarla.
        session.update(PREVIOUS);

        assert_eq!(session.selected_index(), 0);

        session.update(RELEASED);

        assert_eq!(session.update(PREVIOUS), EncounterUpdate::SelectionChanged,);

        // Subir desde la primera opción envuelve a la última.
        assert_eq!(session.selected_index(), 3);
    }

    #[test]
    fn opposite_directions_are_still_inert_outside_player_choice() {
        let mut session = session();

        session.update(CONFIRM_AND_NEXT);
        session.update(RELEASED);

        assert_eq!(session.phase(), EncounterPhase::PlayerResolution);

        assert_eq!(session.update(BOTH_DIRECTIONS), EncounterUpdate::Idle);

        assert_eq!(session.selected_index(), 0);
    }
}

#[cfg(test)]
mod death_beat_tests {
    use super::{
        ActionOutcome, EncounterChoice, EncounterDefinition, EncounterInput, EncounterNode,
        EncounterPhase, EncounterSession, EncounterUpdate, EnemyAction, EnemyTurn, PlayerAction,
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

    const ATTACK: usize = 0;
    const ITEM: usize = 1;
    const FLEE: usize = 2;
    const GAZE: usize = 3;

    fn session() -> EncounterSession {
        EncounterSession::new(SCP_173_ENCOUNTER)
    }

    fn confirm_once(session: &mut EncounterSession) -> EncounterUpdate {
        let update = session.update(CONFIRM);

        session.update(RELEASED);

        update
    }

    fn select(session: &mut EncounterSession, index: usize) {
        while session.selected_index() != index {
            session.update(NEXT);

            session.update(RELEASED);
        }
    }

    fn play_safe_turn(session: &mut EncounterSession, index: usize) {
        select(session, index);

        confirm_once(session);

        confirm_once(session);

        confirm_once(session);
    }

    /// Lleva la sesion hasta el ataque letal por huida.
    fn reach_lethal_attack() -> EncounterSession {
        let mut session = session();

        select(&mut session, FLEE);

        confirm_once(&mut session);

        confirm_once(&mut session);

        assert_eq!(session.enemy_action(), Some(EnemyAction::LethalAttack));

        session
    }

    // ----- El beat del impacto -----

    #[test]
    fn confirming_the_lethal_attack_enters_the_death_beat() {
        let mut session = reach_lethal_attack();

        let update = confirm_once(&mut session);

        assert_eq!(
            update,
            EncounterUpdate::PhaseAdvanced(EncounterPhase::DeathBeat),
            "el ataque letal no debe reportar la muerte todavia",
        );

        assert_eq!(session.phase(), EncounterPhase::DeathBeat);
    }

    #[test]
    fn the_death_beat_shows_the_impact() {
        let mut session = reach_lethal_attack();

        confirm_once(&mut session);

        assert_eq!(session.current_text(), "CRACK.");

        assert_eq!(session.actions_title(), "IMPACTO");

        assert_eq!(session.selected_index(), 0);
    }

    #[test]
    fn the_death_beat_only_offers_continue() {
        let mut session = reach_lethal_attack();

        confirm_once(&mut session);

        assert_eq!(session.choices().len(), 1);

        assert_eq!(session.choices()[0].label, "CONTINUAR");

        assert!(session.choices()[0].action.is_none());
    }

    #[test]
    fn confirming_the_death_beat_reports_the_death() {
        let mut session = reach_lethal_attack();

        confirm_once(&mut session);

        let update = confirm_once(&mut session);

        assert_eq!(update, EncounterUpdate::PlayerDeath);

        // El impacto sigue visible al cerrar el encuentro.
        assert_eq!(session.current_text(), "CRACK.");
    }

    #[test]
    fn the_impact_stays_visible_until_a_new_confirmation() {
        let mut session = reach_lethal_attack();

        session.update(CONFIRM);

        assert_eq!(session.current_text(), "CRACK.");

        // Sostener la tecla no cierra el encuentro ni borra el texto.
        for _ in 0..60 {
            assert_eq!(session.update(CONFIRM), EncounterUpdate::Idle);

            assert_eq!(session.phase(), EncounterPhase::DeathBeat);

            assert_eq!(session.current_text(), "CRACK.");
        }
    }

    #[test]
    fn a_held_confirm_cannot_cross_the_attack_and_the_death_beat() {
        let mut session = reach_lethal_attack();

        // Primera pulsacion: del ataque al impacto.
        assert_eq!(
            session.update(CONFIRM),
            EncounterUpdate::PhaseAdvanced(EncounterPhase::DeathBeat),
        );

        // Sostenida no llega a la muerte.
        for _ in 0..30 {
            assert_eq!(session.update(CONFIRM), EncounterUpdate::Idle);
        }

        // Hace falta soltar para cerrar.
        session.update(RELEASED);

        assert_eq!(session.update(CONFIRM), EncounterUpdate::PlayerDeath);

        // Y seguir sosteniendola no vuelve a reportar la muerte, asi
        // que la pantalla de derrota no puede reintentar sola.
        for _ in 0..30 {
            assert_eq!(session.update(CONFIRM), EncounterUpdate::Idle);
        }
    }

    // ----- Bloqueo de F6 -----

    #[test]
    fn fleeing_locks_the_exit_from_its_own_resolution() {
        let mut session = session();

        select(&mut session, FLEE);

        // Antes de confirmar todavia se puede salir.
        assert!(!session.is_lethal_locked());

        confirm_once(&mut session);

        assert_eq!(session.phase(), EncounterPhase::PlayerResolution);

        assert!(
            session.is_lethal_locked(),
            "huir debe bloquear F6 desde su propia resolucion",
        );
    }

    #[test]
    fn the_fourth_action_locks_the_exit_from_its_resolution() {
        let mut session = session();

        for _ in 0..3 {
            play_safe_turn(&mut session, ATTACK);
        }

        select(&mut session, ATTACK);

        assert!(!session.is_lethal_locked());

        confirm_once(&mut session);

        assert_eq!(session.phase(), EncounterPhase::PlayerResolution);

        assert_eq!(session.turn_count(), 4);

        assert!(
            session.is_lethal_locked(),
            "la cuarta accion debe bloquear F6 desde su resolucion",
        );
    }

    #[test]
    fn normal_actions_in_the_first_three_turns_do_not_lock_the_exit() {
        for index in [ATTACK, ITEM, GAZE] {
            let mut session = session();

            for turn in 1..=3 {
                select(&mut session, index);

                assert!(!session.is_lethal_locked());

                // Resolucion del jugador.
                confirm_once(&mut session);

                assert_eq!(session.turn_count(), turn);

                assert!(
                    !session.is_lethal_locked(),
                    "la accion {index} bloqueo la salida en el turno {turn}",
                );

                // Turno enemigo con Observe.
                confirm_once(&mut session);

                assert_eq!(session.enemy_action(), Some(EnemyAction::Observe));

                assert!(!session.is_lethal_locked());

                confirm_once(&mut session);
            }
        }
    }

    #[test]
    fn every_lethal_phase_locks_the_exit() {
        let mut session = session();

        for _ in 0..3 {
            play_safe_turn(&mut session, GAZE);
        }

        select(&mut session, GAZE);

        confirm_once(&mut session);

        // Secuencia forzada.
        confirm_once(&mut session);

        assert_eq!(session.phase(), EncounterPhase::ForcedSequence);
        assert!(session.is_lethal_locked());

        confirm_once(&mut session);
        confirm_once(&mut session);
        confirm_once(&mut session);

        // Ataque letal.
        assert_eq!(session.phase(), EncounterPhase::EnemyResolution);
        assert_eq!(session.enemy_action(), Some(EnemyAction::LethalAttack));
        assert!(session.is_lethal_locked());

        // Impacto.
        confirm_once(&mut session);

        assert_eq!(session.phase(), EncounterPhase::DeathBeat);
        assert!(session.is_lethal_locked());
    }

    // ----- Validacion de la definicion -----

    const SOUND_FLEE: EnemyTurn = EnemyTurn {
        action: EnemyAction::LethalAttack,
        text: "huida",
    };

    const SOUND_FORCED: EnemyTurn = EnemyTurn {
        action: EnemyAction::LethalAttack,
        text: "parpadeo",
    };

    static ONE_CHOICE: [EncounterChoice; 1] = [EncounterChoice {
        label: "ATACAR",
        action: Some(PlayerAction::Attack),
    }];

    static ONE_NODE: [EncounterNode; 1] = [EncounterNode {
        text: "texto",
        choices: &ONE_CHOICE,
    }];

    static ATTACK_OUTCOME: [ActionOutcome; 1] = [ActionOutcome {
        action: PlayerAction::Attack,
        text: "golpeas",
    }];

    static NO_OUTCOMES: [ActionOutcome; 0] = [];

    static ONE_STEP: [&str; 1] = ["cansancio"];

    static NO_STEPS: [&str; 0] = [];

    static NO_NODES: [EncounterNode; 0] = [];

    fn valid_definition() -> EncounterDefinition {
        EncounterDefinition {
            entity_name: "PRUEBA",
            nodes: &ONE_NODE,
            start_node: 0,
            outcomes: &ATTACK_OUTCOME,
            enemy_turns: &[],
            flee_response: SOUND_FLEE,
            forced_steps: &ONE_STEP,
            forced_response: SOUND_FORCED,
            death_text: "CRACK.",
        }
    }

    fn error_of(definition: EncounterDefinition) -> String {
        definition
            .validate()
            .expect_err("la definicion deberia ser invalida")
    }

    #[test]
    fn a_valid_definition_passes() {
        assert!(valid_definition().validate().is_ok());

        assert!(SCP_173_ENCOUNTER.validate().is_ok());
    }

    #[test]
    fn a_definition_without_nodes_fails_clearly() {
        let mut definition = valid_definition();

        definition.nodes = &NO_NODES;

        let error = error_of(definition);

        assert!(error.contains("PRUEBA"), "sin la entidad: {error}");

        assert!(error.contains("nodo"), "sin la causa: {error}");
    }

    #[test]
    fn an_out_of_range_start_node_fails_clearly() {
        let mut definition = valid_definition();

        definition.start_node = 7;

        let error = error_of(definition);

        assert!(error.contains("7"), "sin el indice: {error}");

        assert!(error.contains("nodo"), "sin la causa: {error}");
    }

    #[test]
    fn a_missing_outcome_fails_clearly() {
        let mut definition = valid_definition();

        definition.outcomes = &NO_OUTCOMES;

        let error = error_of(definition);

        assert!(error.contains("ATACAR"), "sin la opcion: {error}");

        assert!(error.contains("Attack"), "sin la accion: {error}");
    }

    #[test]
    fn an_empty_forced_sequence_fails_clearly() {
        let mut definition = valid_definition();

        definition.forced_steps = &NO_STEPS;

        let error = error_of(definition);

        assert!(error.contains("secuencia forzada"), "poco claro: {error}");
    }

    #[test]
    fn a_non_lethal_flee_response_fails_clearly() {
        let mut definition = valid_definition();

        definition.flee_response = EnemyTurn {
            action: EnemyAction::Observe,
            text: "huida",
        };

        let error = error_of(definition);

        assert!(error.contains("huir"), "poco claro: {error}");

        assert!(error.contains("letal"), "poco claro: {error}");
    }

    #[test]
    fn a_non_lethal_forced_response_fails_clearly() {
        let mut definition = valid_definition();

        definition.forced_response = EnemyTurn {
            action: EnemyAction::Observe,
            text: "parpadeo",
        };

        let error = error_of(definition);

        assert!(error.contains("parpadeo"), "poco claro: {error}");

        assert!(error.contains("letal"), "poco claro: {error}");
    }

    #[test]
    fn an_empty_death_text_fails_clearly() {
        for death_text in ["", "   "] {
            let mut definition = valid_definition();

            definition.death_text = death_text;

            let error = error_of(definition);

            assert!(error.contains("texto de muerte"), "poco claro: {error}");
        }
    }

    #[test]
    #[should_panic(expected = "Definición de encuentro inválida")]
    fn building_a_session_from_an_invalid_definition_panics() {
        let mut definition = valid_definition();

        definition.nodes = &NO_NODES;

        let _ = EncounterSession::new(definition);
    }
}
