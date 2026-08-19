# SCP-Circuitum — Reglas de trabajo

Raycaster 3D en Rust con minifb. Estética industrial oscura inspirada en SCP.
El jugador es un sujeto de pruebas que explora una instalación y debe
sobrevivir a entidades anómalas.

Este archivo define reglas **permanentes**. No documenta el estado de una
tarea ni planes que caducan.

---

## Stack

- Rust edition 2024. Toolchain instalado: 1.97 (compatible desde 1.89).
- `minifb` 0.28 — ventana y buffer de píxeles.
- `nalgebra-glm` 0.21 — solo `Vec2`.
- `font8x8` 0.3 — texto en pantalla.
- `image` `=0.25.8`, `default-features = false`, feature `png` únicamente.
- `[profile.dev]` usa `opt-level = 3`: el perfil dev ya está optimizado.

Constantes de referencia: ventana 1300×900, `BLOCK_SIZE = 100`, `FOV = PI / 3`.

## Arquitectura

Un módulo por responsabilidad. El bucle principal separa
**input → lógica → render**; mantener esa separación.

| Módulo | Responsabilidad |
|---|---|
| `main.rs` | Bucle principal, máquina de estados, orquestación, control de FPS |
| `framebuffer.rs` | Buffer plano `Vec<u32>`; `buffer` es público para las rutas calientes |
| `maze.rs` | `Maze = Vec<Vec<char>>`; carga el `.txt` y extrae la posición inicial |
| `caster.rs` | DDA en espacio de celdas; devuelve `RayHit` |
| `player.rs` | Input, stamina, colisión por 4 esquinas con deslizamiento por eje |
| `texture.rs` | `Texture` (RGB, paredes), `SpriteTexture` (RGBA), `TextureSet` |
| `renderer.rs` | Render 3D, sprites, minimapa, HUD, menús |
| `scp173.rs` | Entidad SCP-173: observación, BFS, waypoints, colisión |
| `game.rs` | `GameState`, `GameSettings`, `GameSession` |

Celdas transitables: `' '`, `'g'`, `'G'`. El resto son sólidas.
Los niveles viven en `levels/`; los assets en `assets/`.

### Calidad

- Todo movimiento y todo temporizador usa `delta_time`. Nunca por frame.
- Los PNG se cargan una sola vez al inicio, jamás dentro del bucle.
- No silenciar advertencias con `#[allow(dead_code)]` ni renombrando a
  `_variable`. Resolver la causa.
- No eliminar tests existentes para que la suite pase.
- Agregar tests para lógica determinista nueva.
- Conservar la arquitectura y las convenciones actuales salvo razón explícita.

## Verificación obligatoria

Ejecutar al cerrar cada bloque y reportar el resultado **real**:

```bash
cargo fmt
cargo check --all-targets
cargo test
```

Si el cambio afecta gameplay o renderizado, entregar además una lista concreta
de pruebas manuales para `cargo run --release`.

Las pruebas manuales las ejecuta la persona, no el asistente: `cargo run` abre
una ventana interactiva y su resultado no es observable desde la terminal.
Nunca inventar resultados de ejecución. Si algo no se pudo ejecutar, decirlo.

## SCP-173 — restricciones de diseño

- Se congela si **cualquiera** de sus muestras de observación cae dentro del
  FOV y tiene línea de visión. Una muestra visible basta.
- Solo se mueve cuando no está siendo observado.
- Navega por celdas transitables mediante BFS; nunca atraviesa paredes.
- Las colisiones físicas son la última barrera aunque el BFS ya haya
  determinado la ruta.
- Existe únicamente en el nivel 1.
- No cambiar la atribución del sprite. No eliminar `ATTRIBUTIONS.md`.
- No agregar SCP-096 ni otras entidades sin aprobación previa.
- No inventar reglas de gameplay que no hayan sido aprobadas.

## Forma de trabajo

### Antes de modificar código

1. `git status` y revisar los últimos commits.
2. Leer los archivos relacionados con la tarea.
3. Verificar que la funcionalidad no esté ya implementada.
4. No asumir el comportamiento de un módulo por su nombre.

### Preservación del trabajo humano

- Nunca sobrescribir ni descartar cambios locales.
- Prohibido `reset`, `checkout` destructivo, `rebase` y limpieza automática.
- Si aparecen cambios locales de origen desconocido: **detenerse y preguntar**
  antes de tocarlos.
- No modificar ni eliminar trabajo que no pertenezca a la tarea en curso.

### Bloques coherentes

- Trabajar por bloques funcionales completos, no línea por línea.
- Con la tarea definida: analizar y luego implementar el bloque entero sin
  pedir permiso por cada microcambio.
- No mezclar sistemas no relacionados en un mismo bloque.
- Si falta una decisión real de producto o diseño: detenerse y preguntar.

### Alcance

- Sin refactors oportunistas.
- Sin dependencias nuevas de peso sin consultar.
- Sin sistemas genéricos "para el futuro" que la tarea actual no necesite.
- Preferir siempre lo pequeño, claro y verificable.
- **Estabilidad antes que amplitud.** Ante la duda entre añadir una función y
  consolidar lo existente, consolidar.

### Git

- **Nunca** `commit`, `push`, `pull`, `merge`, `rebase` ni cambio de rama sin
  petición explícita.
- La persona hace las pruebas manuales, el commit y el push.
- Los mensajes de commit van en español.

## Comunicación

- Responder en español natural.
- Explicar las decisiones importantes; omitir lo trivial.
- Distinguir con claridad entre **hecho comprobado**, **decisión de diseño** y
  **recomendación**.
- Un problema fuera del alcance se reporta como follow-up; no se arregla en
  silencio.

### Reporte obligatorio al cerrar cada bloque

1. Resumen de lo implementado.
2. Archivos creados o modificados.
3. Decisiones técnicas relevantes.
4. Comandos ejecutados y su resultado real.
5. Pruebas manuales pendientes para la persona.
6. Limitaciones conocidas.
7. `git status` final y diff resumido por archivo.

## Revisión externa

Tras el push, un reviewer independiente lee el diff desde GitHub. Cuando
lleguen sus hallazgos: verificar cada uno contra el código real antes de
actuar. No aceptarlos ni rechazarlos de forma automática. Mantener las
correcciones dentro del alcance del bloque.
