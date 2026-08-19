use crate::game::GameState;

/// Decide si el cursor debe estar capturado en el frame actual.
///
/// Es una función pura y sin dependencias de plataforma, así que
/// puede probarse sin abrir una ventana. La captura solo tiene
/// sentido mientras se explora y la ventana tiene el foco: si el
/// jugador hace Alt+Tab, dejar el cursor confinado atraparía el
/// puntero fuera del juego.
pub fn should_capture_cursor(state: GameState, window_is_active: bool) -> bool {
    matches!(state, GameState::Playing) && window_is_active
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use std::ptr;

    use winapi::shared::windef::{HWND, POINT, RECT};
    use winapi::um::winuser::{ClientToScreen, ClipCursor, GetClientRect, SetCursorPos};

    /// Confina el cursor al área cliente y lo devuelve a su centro
    /// después de cada lectura, de modo que el giro no tiene tope.
    pub struct MouseCapture {
        active: bool,

        /// Último rectángulo entregado a `ClipCursor`, en coordenadas
        /// de pantalla. Permite no repetir la llamada cuando la
        /// ventana no se movió.
        clipped_rect: Option<(i32, i32, i32, i32)>,

        /// Centro del área cliente en coordenadas de pantalla, que es
        /// lo que espera `SetCursorPos`.
        center_on_screen: (i32, i32),

        /// Evita repetir el mismo aviso en cada frame.
        failure_reported: bool,
    }

    impl MouseCapture {
        pub fn new() -> Self {
            Self {
                active: false,
                clipped_rect: None,
                center_on_screen: (0, 0),
                failure_reported: false,
            }
        }

        pub fn is_active(&self) -> bool {
            self.active
        }

        /// Activa o mantiene la captura. Devuelve `true` si el cursor
        /// quedó confinado y se puede medir contra el centro.
        pub fn engage(&mut self, window_handle: *mut c_void) -> bool {
            if window_handle.is_null() {
                self.report_failure("el handle de la ventana es nulo");

                return false;
            }

            let window = window_handle as HWND;

            let Some(rect) = client_rect_on_screen(window) else {
                self.report_failure("no se pudo leer el área cliente");

                return false;
            };

            let (left, top, right, bottom) = rect;

            self.center_on_screen = ((left + right) / 2, (top + bottom) / 2);

            // Volver a confinar solo si el rectángulo cambió, por
            // ejemplo porque la ventana se movió de sitio.
            if self.clipped_rect != Some(rect) {
                let clip_rect = RECT {
                    left,
                    top,
                    right,
                    bottom,
                };

                // SAFETY: `clip_rect` vive durante toda la llamada y
                // `ClipCursor` únicamente lo lee. Un puntero no nulo
                // confina el cursor al rectángulo indicado.
                let confined = unsafe { ClipCursor(&clip_rect) } != 0;

                if !confined {
                    self.report_failure("ClipCursor falló");

                    return false;
                }

                self.clipped_rect = Some(rect);
            }

            self.active = true;

            // Si más adelante vuelve a fallar, queremos enterarnos.
            self.failure_reported = false;

            true
        }

        /// Devuelve el cursor al centro del área cliente. Debe
        /// llamarse *después* de leer el movimiento del frame, o el
        /// recentrado se confundiría con movimiento del jugador.
        pub fn recenter(&self) {
            if !self.active {
                return;
            }

            let (center_x, center_y) = self.center_on_screen;

            // SAFETY: `SetCursorPos` recibe coordenadas de pantalla
            // por valor y no conserva punteros. El centro proviene
            // del área cliente ya convertida con `ClientToScreen`.
            unsafe {
                SetCursorPos(center_x, center_y);
            }
        }

        /// Libera el confinamiento. Es idempotente y seguro de llamar
        /// aunque nunca se haya capturado.
        pub fn release(&mut self) {
            if !self.active && self.clipped_rect.is_none() {
                return;
            }

            // SAFETY: un puntero nulo indica a `ClipCursor` que
            // elimine cualquier restricción vigente. Es válido
            // aunque no exista ninguna.
            unsafe {
                ClipCursor(ptr::null());
            }

            self.active = false;

            self.clipped_rect = None;
        }

        fn report_failure(&mut self, reason: &str) {
            self.release();

            if !self.failure_reported {
                eprintln!(
                    "Mouse: no se pudo capturar el cursor ({reason}). \
                     Se conserva la rotación sin confinamiento."
                );

                self.failure_reported = true;
            }
        }
    }

    impl Drop for MouseCapture {
        /// Última barrera: pase lo que pase —cierre normal, salida
        /// anticipada o desenrollado por panic— el cursor nunca queda
        /// atrapado en el escritorio.
        fn drop(&mut self) {
            self.release();
        }
    }

    /// Área cliente de la ventana en coordenadas de pantalla.
    ///
    /// `GetClientRect` la entrega con origen (0, 0) y sin barra de
    /// título ni bordes, así que hay que convertirla: la posición de
    /// la ventana no equivale al origen del área cliente.
    fn client_rect_on_screen(window: HWND) -> Option<(i32, i32, i32, i32)> {
        let mut client_rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };

        // SAFETY: el llamador ya comprobó que el handle no es nulo, y
        // `GetClientRect` solo escribe en `client_rect`.
        if unsafe { GetClientRect(window, &mut client_rect) } == 0 {
            return None;
        }

        let mut top_left = POINT {
            x: client_rect.left,
            y: client_rect.top,
        };

        let mut bottom_right = POINT {
            x: client_rect.right,
            y: client_rect.bottom,
        };

        // SAFETY: ambos puntos son locales y `ClientToScreen` solo
        // los reescribe en sitio.
        let converted = unsafe {
            ClientToScreen(window, &mut top_left) != 0
                && ClientToScreen(window, &mut bottom_right) != 0
        };

        if !converted {
            return None;
        }

        // Una ventana minimizada puede dar un área vacía; confinar el
        // cursor a un rectángulo degenerado no tendría sentido.
        if bottom_right.x <= top_left.x || bottom_right.y <= top_left.y {
            return None;
        }

        Some((top_left.x, top_left.y, bottom_right.x, bottom_right.y))
    }
}

#[cfg(not(windows))]
mod platform {
    use std::ffi::c_void;

    /// Fuera de Windows no se confina ni se recentra el cursor.
    ///
    /// El confinamiento y el recentrado están implementados para
    /// Windows, que es el entorno donde el juego se ejecuta y se
    /// prueba. En las demás plataformas el juego conserva el
    /// comportamiento anterior basado en `MouseMode`: la rotación se
    /// detiene cuando el cursor sale de la ventana.
    pub struct MouseCapture;

    impl MouseCapture {
        pub fn new() -> Self {
            Self
        }

        pub fn is_active(&self) -> bool {
            false
        }

        pub fn engage(&mut self, _window_handle: *mut c_void) -> bool {
            false
        }

        pub fn recenter(&self) {}

        pub fn release(&mut self) {}
    }
}

pub use platform::MouseCapture;

#[cfg(test)]
mod tests {
    use super::should_capture_cursor;
    use crate::game::GameState;

    #[test]
    fn playing_with_focus_requests_capture() {
        assert!(should_capture_cursor(GameState::Playing, true));
    }

    #[test]
    fn playing_without_focus_does_not_request_capture() {
        assert!(!should_capture_cursor(GameState::Playing, false));
    }

    #[test]
    fn paused_does_not_request_capture() {
        assert!(!should_capture_cursor(GameState::Paused, true));
    }

    #[test]
    fn welcome_does_not_request_capture() {
        assert!(!should_capture_cursor(GameState::Welcome, true));
    }

    #[test]
    fn level_transition_does_not_request_capture() {
        assert!(!should_capture_cursor(GameState::LevelTransition, true));
    }

    #[test]
    fn victory_does_not_request_capture() {
        assert!(!should_capture_cursor(GameState::Victory, true));
    }
}
