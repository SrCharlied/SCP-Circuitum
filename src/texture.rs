use image::ImageReader;

pub struct Texture {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
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

    pub fn sample(&self, texture_u: f32, texture_v: f32) -> u32 {
        let normalized_u = texture_u.rem_euclid(1.0);

        let normalized_v = texture_v.clamp(0.0, 0.999_999);

        let texture_x = (normalized_u * self.width as f32) as usize;

        let texture_y = (normalized_v * self.height as f32) as usize;

        self.pixels[texture_y * self.width + texture_x]
    }
}
