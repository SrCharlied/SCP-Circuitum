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

    /// Marca la tecla como ya pulsada sin disparar.
    ///
    /// Se usa al abrir el encuentro: si el jugador todavía sostiene
    /// la tecla que lo abrió, no debe contar como confirmación.
    pub fn suppress_until_release(&mut self) {
        self.was_down = true;
    }
}

/// Estado de las teclas relevantes en el frame actual.
#[derive(Clone, Copy, Debug, Default)]
pub struct EncounterInput {
    pub next_down: bool,
    pub previous_down: bool,
    pub confirm_down: bool,
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

    /// Impide que una tecla ya sostenida al abrir el encuentro
    /// cuente como pulsación.
    pub fn suppress_held_keys(&mut self) {
        self.next_trigger.suppress_until_release();
        self.previous_trigger.suppress_until_release();
        self.confirm_trigger.suppress_until_release();
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

        if confirm_fired {
            if let Some(choice) = self.choices().get(self.selected_index).copied() {
                return EncounterUpdate::Confirmed(choice);
            }
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
    fn keys_held_when_the_encounter_opens_do_not_act() {
        let mut session = session();

        // El jugador abre el encuentro con Enter todavía pulsado.
        session.suppress_held_keys();

        for _ in 0..10 {
            assert_eq!(
                session.update(press(true, true, true)),
                EncounterUpdate::Idle,
            );
        }

        assert_eq!(session.selected_index(), 0);

        // Solo tras soltar vuelve a responder.
        session.update(RELEASED);

        assert!(matches!(
            session.update(press(true, false, false)),
            EncounterUpdate::Confirmed(_),
        ));
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
