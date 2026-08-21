use image::ImageReader;

/// Parte fraccionaria en `[0, 1)`, equivalente a `rem_euclid(1.0)`.
///
/// `rem_euclid` sobre `f32` son dos `fmod`, y en el muestreo de los
/// planos se ejecuta por píxel: medido, era el coste dominante del
/// suelo y el techo. `x - x.floor()` es una sola instrucción.
///
/// Para cualquier `x` finito, `x - floor(x)` y `x.rem_euclid(1.0)`
/// hacen la misma resta contra el mismo entero, así que coinciden
/// bit a bit. Incluido el caso límite en que un negativo diminuto
/// redondea el resultado a `1.0`: ambos lo hacen, y el recorte del
/// índice en `sample_planes` ya lo contemplaba.
#[inline(always)]
fn fract(value: f32) -> f32 {
    value - value.floor()
}

pub struct Texture {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
}

pub struct SpriteTexture {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
}

pub struct TextureSet {
    walls_by_level: Vec<Texture>,
    column: Option<Texture>,
    goal: Option<Texture>,

    /// Planos del escenario. Son obligatorios: sin ellos no hay
    /// suelo ni techo que dibujar, así que el inicio debe fallar.
    floor: Texture,
    ceiling: Texture,
}

impl Texture {
    pub fn from_file(path: &str) -> Result<Self, String> {
        let image = ImageReader::open(path)
            .map_err(|error| {
                format!(
                    "No se pudo abrir la \
                         textura '{path}': \
                         {error}"
                )
            })?
            .decode()
            .map_err(|error| {
                format!(
                    "No se pudo decodificar \
                         la textura '{path}': \
                         {error}"
                )
            })?
            .to_rgb8();

        let width = image.width() as usize;

        let height = image.height() as usize;

        if width == 0 || height == 0 {
            return Err(format!(
                "La textura '{path}' no \
                 puede estar vacía"
            ));
        }

        let pixels = image
            .pixels()
            .map(|pixel| {
                let [red, green, blue] = pixel.0;

                ((red as u32) << 16) | ((green as u32) << 8) | blue as u32
            })
            .collect();

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn column_index(&self, texture_u: f32) -> usize {
        let normalized_u = texture_u.rem_euclid(1.0);

        (normalized_u * self.width as f32) as usize
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn sample_column(&self, texture_x: usize, texture_y: usize) -> u32 {
        self.pixels[texture_y * self.width + texture_x]
    }

    pub fn width(&self) -> usize {
        self.width
    }

    /// Texel por índice lineal ya validado.
    ///
    /// Es privado al módulo a propósito: permite que `TextureSet`
    /// reutilice un índice entre dos texturas sin exponer el vector
    /// de píxeles fuera de aquí.
    fn texel(&self, index: usize) -> u32 {
        self.pixels[index]
    }
}

impl SpriteTexture {
    pub fn from_file(path: &str) -> Result<Self, String> {
        let image = ImageReader::open(path)
            .map_err(|error| format!("No se pudo abrir el sprite '{path}': {error}"))?
            .decode()
            .map_err(|error| format!("No se pudo decodificar el sprite '{path}': {error}"))?
            .to_rgba8();

        let width = image.width() as usize;

        let height = image.height() as usize;

        if width == 0 || height == 0 {
            return Err(format!("El sprite '{path}' no puede estar vacio"));
        }

        let pixels = image
            .pixels()
            .map(|pixel| {
                let [red, green, blue, alpha] = pixel.0;

                ((alpha as u32) << 24) | ((red as u32) << 16) | ((green as u32) << 8) | blue as u32
            })
            .collect();

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn sample(&self, texture_x: usize, texture_y: usize) -> u32 {
        self.pixels[texture_y * self.width + texture_x]
    }
}

impl TextureSet {
    pub fn from_files(
        wall_paths: &[&str],
        column_path: &str,
        goal_path: &str,
        floor_path: &str,
        ceiling_path: &str,
    ) -> Result<Self, String> {
        if wall_paths.is_empty() {
            return Err(String::from("Se necesita al menos una textura de pared"));
        }

        let walls_by_level = wall_paths
            .iter()
            .map(|path| Texture::from_file(path))
            .collect::<Result<Vec<_>, _>>()?;

        let floor = Texture::from_file(floor_path)?;

        let ceiling = Texture::from_file(ceiling_path)?;

        // `sample_planes` reutiliza un solo índice para los dos
        // planos, así que la invariante se valida al cargar en lugar
        // de asumirse en silencio.
        if floor.width() != ceiling.width() || floor.height() != ceiling.height() {
            return Err(format!(
                "El suelo '{floor_path}' ({}x{}) y el techo '{ceiling_path}' ({}x{}) \
                 deben tener la misma resolución",
                floor.width(),
                floor.height(),
                ceiling.width(),
                ceiling.height(),
            ));
        }

        Ok(Self {
            walls_by_level,

            column: Some(Texture::from_file(column_path)?),

            goal: Some(Texture::from_file(goal_path)?),

            floor,

            ceiling,
        })
    }

    /// Muestrea suelo y techo con las mismas coordenadas del mundo.
    ///
    /// Suelo y techo se dibujan en filas simétricas respecto al
    /// horizonte, que comparten posición mundial. Normalizar aquí una
    /// sola vez evita repetir dos `rem_euclid` y dos conversiones por
    /// cada par de píxeles.
    ///
    /// Ambos planos tienen la misma resolución —validado al cargar—,
    /// así que un único índice sirve para los dos.
    pub fn sample_planes(&self, texture_u: f32, texture_v: f32) -> (u32, u32) {
        let normalized_u = fract(texture_u);

        let normalized_v = fract(texture_v);

        let width = self.floor.width();

        let height = self.floor.height();

        let texture_x = ((normalized_u * width as f32) as usize).min(width - 1);

        let texture_y = ((normalized_v * height as f32) as usize).min(height - 1);

        let index = texture_y * width + texture_x;

        (self.floor.texel(index), self.ceiling.texel(index))
    }

    pub fn for_cell(&self, cell: char, level_number: usize) -> Option<&Texture> {
        match cell {
            '+' => self.column.as_ref(),

            'g' | 'G' => self.goal.as_ref(),

            _ => self
                .walls_by_level
                .get(level_number.saturating_sub(1))
                .or_else(|| self.walls_by_level.first()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SpriteTexture, TextureSet};

    #[test]
    fn loads_scp_173_transparency() {
        let sprite = SpriteTexture::from_file("./assets/sprites/scp_173.png")
            .expect("El sprite de SCP-173 debe cargar correctamente");

        assert_eq!(sprite.width(), 320);
        assert_eq!(sprite.height(), 320);

        // La esquina del lienzo es relleno transparente.
        assert_eq!(sprite.sample(0, 0) >> 24, 0);

        // El torso de la figura es opaco. La figura ocupa
        // la franja x 114..204 del lienzo de 320 px.
        assert_eq!(sprite.sample(159, 280) >> 24, 255);
    }

    #[test]
    fn selects_a_stable_wall_texture_for_each_level() {
        let textures = TextureSet::from_files(
            &[
                "./assets/textures/wall_industrial.png",
                "./assets/textures/wall_industrial_connected.png",
            ],
            "./assets/textures/column_reinforced.png",
            "./assets/textures/goal_elevator.png",
            "./assets/textures/floor_industrial.png",
            "./assets/textures/ceiling_industrial.png",
        )
        .expect("Las texturas de prueba deben cargar correctamente");

        let level_one_wall = textures
            .for_cell('|', 1)
            .expect("El nivel 1 debe tener pared");
        let level_two_wall = textures
            .for_cell('|', 2)
            .expect("El nivel 2 debe tener pared");

        assert!(!std::ptr::eq(level_one_wall, level_two_wall));
        assert!(std::ptr::eq(
            textures.for_cell('+', 1).expect("Debe existir columna"),
            textures.for_cell('+', 2).expect("Debe existir columna"),
        ));
        assert!(std::ptr::eq(
            textures.for_cell('g', 1).expect("Debe existir meta"),
            textures.for_cell('g', 2).expect("Debe existir meta"),
        ));
    }
}

#[cfg(test)]
mod plane_texture_tests {
    use super::{Texture, TextureSet};

    const WALL: &str = "./assets/textures/wall_industrial.png";
    const COLUMN: &str = "./assets/textures/column_reinforced.png";
    const GOAL: &str = "./assets/textures/goal_elevator.png";
    const FLOOR: &str = "./assets/textures/floor_industrial.png";
    const CEILING: &str = "./assets/textures/ceiling_industrial.png";

    fn planes(floor: &str, ceiling: &str) -> Result<TextureSet, String> {
        TextureSet::from_files(&[WALL], COLUMN, GOAL, floor, ceiling)
    }

    fn loaded() -> TextureSet {
        planes(FLOOR, CEILING).expect("las texturas deben cargar")
    }

    #[test]
    fn wrapping_repeats_coordinates_above_one() {
        let textures = loaded();

        for (u, v) in [(0.0, 0.0), (0.25, 0.75), (0.5, 0.1), (0.99, 0.42)] {
            let base = textures.sample_planes(u, v);

            // Sumar vueltas completas no cambia los píxeles.
            assert_eq!(textures.sample_planes(u + 1.0, v + 1.0), base);
            assert_eq!(textures.sample_planes(u + 7.0, v + 3.0), base);
            assert_eq!(textures.sample_planes(u + 128.0, v + 64.0), base);
        }
    }

    #[test]
    fn wrapping_repeats_negative_coordinates() {
        let textures = loaded();

        for (u, v) in [(0.0, 0.0), (0.3, 0.6), (0.87, 0.12)] {
            let base = textures.sample_planes(u, v);

            assert_eq!(textures.sample_planes(u - 1.0, v - 1.0), base);
            assert_eq!(textures.sample_planes(u - 5.0, v - 9.0), base);
        }

        // Justo por debajo de cero cae al último texel, no al primero.
        assert_eq!(
            textures.sample_planes(-0.001, -0.001),
            textures.sample_planes(0.999, 0.999),
        );
    }

    #[test]
    fn wrapping_never_leaves_the_texture() {
        let textures = loaded();

        // Valores extremos y no finitos no deben provocar pánico.
        for coordinate in [
            -1_000.0,
            -0.5,
            0.0,
            1.0,
            1_000.5,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
        ] {
            let _ = textures.sample_planes(coordinate, coordinate);
        }
    }

    /// Colores esperados en coordenadas conocidas.
    ///
    /// Fija la apariencia: si el muestreo compartido cambiara de
    /// texel respecto al original, estos valores se moverían.
    #[test]
    fn known_coordinates_keep_their_colours() {
        let textures = loaded();

        let floor = Texture::from_file(FLOOR).unwrap();

        let ceiling = Texture::from_file(CEILING).unwrap();

        for (u, v) in [
            (0.0f32, 0.0f32),
            (0.125, 0.375),
            (0.5, 0.5),
            (0.75, 0.25),
            (0.99, 0.01),
            (2.25, -3.75),
            (-0.6, 1.4),
        ] {
            // Referencia calculada de forma independiente, igual que
            // hacía el muestreo por textura separada.
            let width = floor.width();
            let height = floor.height();

            let x = ((u.rem_euclid(1.0) * width as f32) as usize).min(width - 1);
            let y = ((v.rem_euclid(1.0) * height as f32) as usize).min(height - 1);

            let expected = (floor.sample_column(x, y), ceiling.sample_column(x, y));

            assert_eq!(
                textures.sample_planes(u, v),
                expected,
                "el muestreo compartido cambió el color en ({u}, {v})",
            );
        }
    }

    #[test]
    fn the_texture_set_loads_floor_and_ceiling() {
        let textures = loaded();

        // Ambos planos existen y no son la misma imagen.
        let mut differences = 0;

        for step in 0..64 {
            let coordinate = step as f32 / 64.0;

            let (floor_color, ceiling_color) = textures.sample_planes(coordinate, coordinate);

            if floor_color != ceiling_color {
                differences += 1;
            }
        }

        assert!(differences > 0, "suelo y techo resultaron idénticos");
    }

    #[test]
    fn the_ceiling_is_darker_than_the_wall() {
        let textures = loaded();

        let wall = Texture::from_file(WALL).unwrap();

        let mut ceiling_total = 0.0;
        let mut wall_total = 0.0;

        for y in 0..64 {
            for x in 0..64 {
                let (_, ceiling_color) = textures.sample_planes(x as f32 / 64.0, y as f32 / 64.0);

                let wall_color = wall.sample_column(x, y);

                for color in [(ceiling_color, true), (wall_color, false)] {
                    let luma = (((color.0 >> 16) & 0xFF)
                        + ((color.0 >> 8) & 0xFF)
                        + (color.0 & 0xFF)) as f32
                        / 3.0;

                    if color.1 {
                        ceiling_total += luma;
                    } else {
                        wall_total += luma;
                    }
                }
            }
        }

        assert!(
            ceiling_total < wall_total,
            "el techo debe ser más oscuro que la pared",
        );
    }

    #[test]
    fn a_missing_plane_texture_fails_with_a_clear_error() {
        let error = planes("./assets/textures/no_existe_suelo.png", CEILING)
            .err()
            .expect("una textura de suelo ausente debe fallar");

        assert!(
            error.contains("no_existe_suelo.png"),
            "error poco claro: {error}"
        );
        assert!(
            error.contains("No se pudo abrir"),
            "error poco claro: {error}"
        );

        let ceiling_error = planes(FLOOR, "./assets/textures/no_existe_techo.png")
            .err()
            .expect("una textura de techo ausente debe fallar");

        assert!(ceiling_error.contains("no_existe_techo.png"));
    }

    #[test]
    fn mismatched_plane_resolutions_fail_with_a_clear_error() {
        // El sprite de SCP-173 es 320x320: sirve como plano de otra
        // resolución para comprobar la validación.
        let error = planes(FLOOR, "./assets/sprites/scp_173.png")
            .err()
            .expect("resoluciones distintas deben fallar");

        assert!(
            error.contains("misma resolución"),
            "el error no explica la invariante: {error}",
        );
        assert!(
            error.contains("64x64"),
            "el error no indica los tamaños: {error}"
        );
        assert!(
            error.contains("320x320"),
            "el error no indica los tamaños: {error}"
        );
    }
}

#[cfg(test)]
mod fract_tests {
    use super::fract;

    #[test]
    fn fract_matches_rem_euclid_over_a_wide_range() {
        let mut value = -2_000.0f32;

        while value < 2_000.0 {
            assert_eq!(fract(value), value.rem_euclid(1.0), "difieren en {value}",);

            value += 0.37;
        }
    }

    #[test]
    fn fract_matches_rem_euclid_on_the_edges() {
        for value in [
            0.0f32,
            -0.0,
            1.0,
            -1.0,
            0.999_999_9,
            -0.000_000_1,
            -1e-30,
            -1e-7,
            1e-7,
            123.456,
            -123.456,
        ] {
            assert_eq!(fract(value), value.rem_euclid(1.0), "difieren en {value}");
        }
    }

    /// El caso límite: un negativo diminuto hace que la parte
    /// fraccionaria redondee a `1.0`. `rem_euclid` se comporta igual,
    /// y el recorte del índice lo absorbe sin salirse de la textura.
    #[test]
    fn the_rounding_edge_behaves_like_rem_euclid_and_stays_in_range() {
        for value in [
            -1e-30f32,
            -1e-20,
            -1e-10,
            f32::MIN_POSITIVE,
            -f32::MIN_POSITIVE,
            1e30,
            -1e30,
        ] {
            let fraction = fract(value);

            assert_eq!(fraction, value.rem_euclid(1.0), "difieren en {value}");

            assert!(
                (0.0..=1.0).contains(&fraction),
                "fract({value}) salió del rango: {fraction}",
            );

            // Lo que de verdad importa: el índice nunca sale de la
            // textura, gracias al recorte de `sample_planes`.
            let width = 64usize;

            let index = ((fraction * width as f32) as usize).min(width - 1);

            assert!(index < width);
        }
    }
}
