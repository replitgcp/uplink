use crate::language::US_ENGLISH;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    // Selected Language
    #[serde(default = "default_lang")]
    pub language: String,
    #[serde(default = "default_font_scale")]
    font_scale: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: default_lang(),
            font_scale: 1.0,
        }
    }
}

fn default_font_scale() -> f32 {
    1.0_f32
}

fn default_lang() -> String {
    US_ENGLISH.1.to_string()
}

impl Settings {
    pub fn font_scale(&self) -> f32 {
        self.font_scale
    }
    pub fn set_font_scale(&mut self, scale: f32) {
        self.font_scale = scale;
    }
}
