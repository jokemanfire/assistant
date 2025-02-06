pub struct SettingsUI {
    volume: u8,
    language: String,
}

impl SettingsUI {
    pub fn new() -> Self {
        SettingsUI {
            volume: 50,
            language: "English".to_string(),
        }
    }

    pub fn display(&self) {
        println!("Settings:");
        println!("Volume: {}", self.volume);
        println!("Language: {}", self.language);
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume;
    }

    pub fn set_language(&mut self, language: String) {
        self.language = language;
    }
}
