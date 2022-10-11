use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub write_dir: String,
    pub lua_path: String,
    pub dll_path: String,
    pub debug: bool,
    pub enable_object_log: bool,
    pub enable_framerate_log: bool,
    pub enable_gui: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            write_dir: "".to_string(),
            lua_path: "".to_string(),
            dll_path: "".to_string(),
            debug: false,
            enable_object_log: false,
            enable_framerate_log: true,
            enable_gui: true,
        }
    }
}
