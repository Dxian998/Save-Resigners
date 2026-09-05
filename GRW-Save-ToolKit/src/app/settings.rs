use std::fs;
use std::path::Path;

pub struct AppSettings {
    pub default_tab: i32,
    pub ubisoft_dir: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_tab: 0,
            ubisoft_dir: r"C:\Program Files (x86)\Ubisoft\Ubisoft Game Launcher\savegames".to_string(),
        }
    }
}

impl AppSettings {
    pub fn load() -> Self {
        let path = Path::new("settings.json");
        let mut s = Self::default();
        if let Ok(data) = fs::read_to_string(path) {
            if let Some(pos) = data.find("\"default_tab\":") {
                let rest = data[pos + 14..].trim();
                let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(tab) = num_str.parse::<i32>() {
                    s.default_tab = tab;
                }
            }
            if let Some(pos) = data.find("\"ubisoft_dir\":") {
                let rest = data[pos + 14..].trim();
                if let Some(start) = rest.find('"') {
                    if let Some(end) = rest[start + 1..].find('"') {
                        s.ubisoft_dir = rest[start + 1..start + 1 + end].replace(r"\\", r"\");
                    }
                }
            }
        }
        s
    }

    pub fn save(&self) {
        let escaped = self.ubisoft_dir.replace('\\', "\\\\");
        let json = format!("{{\n  \"default_tab\": {},\n  \"ubisoft_dir\": \"{}\"\n}}\n", self.default_tab, escaped);
        fs::write("settings.json", json).ok();
    }
}