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
        })
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
