use image::ImageReader;

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

    /// Muestrea con coordenadas normalizadas repetibles.
    ///
    /// Envuelve ambos ejes con `rem_euclid`, así que acepta valores
    /// mayores que 1 y negativos: es lo que permite teselar un plano
    /// del mundo sin acotar las coordenadas en el llamador. El
    /// filtrado es nearest-neighbour, para conservar el pixel art.
    pub fn sample_wrapped(&self, texture_u: f32, texture_v: f32) -> u32 {
        let normalized_u = texture_u.rem_euclid(1.0);

        let normalized_v = texture_v.rem_euclid(1.0);

        // `rem_euclid` sobre un valor no finito da NaN, y convertirlo
        // a entero satura en 0. El `min` cubre además el caso límite
        // en que el redondeo alcance el ancho exacto.
        let texture_x = ((normalized_u * self.width as f32) as usize).min(self.width - 1);

        let texture_y = ((normalized_v * self.height as f32) as usize).min(self.height - 1);

        self.pixels[texture_y * self.width + texture_x]
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

        Ok(Self {
            walls_by_level,

            column: Some(Texture::from_file(column_path)?),

            goal: Some(Texture::from_file(goal_path)?),

            floor: Texture::from_file(floor_path)?,

            ceiling: Texture::from_file(ceiling_path)?,
        })
    }

    pub fn floor(&self) -> &Texture {
        &self.floor
    }

    pub fn ceiling(&self) -> &Texture {
        &self.ceiling
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

    const FLOOR: &str = "./assets/textures/floor_industrial.png";
    const CEILING: &str = "./assets/textures/ceiling_industrial.png";

    fn floor_texture() -> Texture {
        Texture::from_file(FLOOR).expect("la textura de suelo debe cargar")
    }

    #[test]
    fn wrapping_repeats_coordinates_above_one() {
        let texture = floor_texture();

        for (u, v) in [(0.0, 0.0), (0.25, 0.75), (0.5, 0.1), (0.99, 0.42)] {
            let base = texture.sample_wrapped(u, v);

            // Sumar vueltas completas no cambia el píxel.
            assert_eq!(texture.sample_wrapped(u + 1.0, v + 1.0), base);
            assert_eq!(texture.sample_wrapped(u + 7.0, v + 3.0), base);
            assert_eq!(texture.sample_wrapped(u + 128.0, v + 64.0), base);
        }
    }

    #[test]
    fn wrapping_repeats_negative_coordinates() {
        let texture = floor_texture();

        for (u, v) in [(0.0, 0.0), (0.3, 0.6), (0.87, 0.12)] {
            let base = texture.sample_wrapped(u, v);

            assert_eq!(texture.sample_wrapped(u - 1.0, v - 1.0), base);
            assert_eq!(texture.sample_wrapped(u - 5.0, v - 9.0), base);
        }

        // Justo por debajo de cero cae al último texel, no al primero.
        assert_eq!(
            texture.sample_wrapped(-0.001, -0.001),
            texture.sample_wrapped(0.999, 0.999),
        );
    }

    #[test]
    fn wrapping_never_leaves_the_texture() {
        let texture = floor_texture();

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
            let _ = texture.sample_wrapped(coordinate, coordinate);
        }
    }

    #[test]
    fn the_texture_set_loads_floor_and_ceiling() {
        let textures = TextureSet::from_files(
            &["./assets/textures/wall_industrial.png"],
            "./assets/textures/column_reinforced.png",
            "./assets/textures/goal_elevator.png",
            FLOOR,
            CEILING,
        )
        .expect("las texturas deben cargar");

        // Son planos distintos entre sí y distintos de la pared.
        assert!(!std::ptr::eq(textures.floor(), textures.ceiling()));

        assert!(!std::ptr::eq(
            textures.floor(),
            textures.for_cell('|', 1).expect("debe existir pared"),
        ));

        // Y ambos se pueden muestrear.
        let _ = textures.floor().sample_wrapped(0.5, 0.5);
        let _ = textures.ceiling().sample_wrapped(0.5, 0.5);
    }

    #[test]
    fn the_ceiling_is_darker_than_the_wall() {
        let wall = Texture::from_file("./assets/textures/wall_industrial.png").unwrap();

        let ceiling = Texture::from_file(CEILING).unwrap();

        fn average_luma(texture: &Texture) -> f32 {
            let mut total = 0.0;
            let mut count = 0.0;

            for step in 0..64 {
                for other in 0..64 {
                    let u = step as f32 / 64.0;
                    let v = other as f32 / 64.0;
                    let color = texture.sample_wrapped(u, v);
                    let red = ((color >> 16) & 0xFF) as f32;
                    let green = ((color >> 8) & 0xFF) as f32;
                    let blue = (color & 0xFF) as f32;
                    total += (red + green + blue) / 3.0;
                    count += 1.0;
                }
            }

            total / count
        }

        assert!(
            average_luma(&ceiling) < average_luma(&wall),
            "el techo debe ser más oscuro que la pared",
        );
    }

    #[test]
    fn a_missing_plane_texture_fails_with_a_clear_error() {
        let error = TextureSet::from_files(
            &["./assets/textures/wall_industrial.png"],
            "./assets/textures/column_reinforced.png",
            "./assets/textures/goal_elevator.png",
            "./assets/textures/no_existe_suelo.png",
            CEILING,
        )
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

        let ceiling_error = TextureSet::from_files(
            &["./assets/textures/wall_industrial.png"],
            "./assets/textures/column_reinforced.png",
            "./assets/textures/goal_elevator.png",
            FLOOR,
            "./assets/textures/no_existe_techo.png",
        )
        .err()
        .expect("una textura de techo ausente debe fallar");

        assert!(ceiling_error.contains("no_existe_techo.png"));
    }
}
