use std::path::PathBuf;
use colored::Colorize;
use json_comments::StripComments;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub debug: bool,
    pub api_addr: String,
    pub api_key: String,
    pub model: String,
    pub show_ai_commands_output: bool,
    pub context_window_size: usize,
    pub shell_type: String,
    pub require_confirmation: bool,
    #[serde(default)] // Optional field, defaults to empty vec
    pub references: Vec<Reference>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Reference {
    pub command: String,
    pub description: String,
}

pub fn get_config_path() -> PathBuf {
    if let Ok(path) = std::env::var("AIOSC_CONFIG_PATH") {
        return PathBuf::from(path);
    }
    let mut config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir.push("aiosc");
    config_dir.push("aiosc.config.json");
    config_dir
}

pub fn load_config() -> Config {
    let mut config = Config {
        debug: false,
        api_addr: "https://openrouter.ai/api/v1".to_string(),
        api_key: "".to_string(),
        model: "qwen/qwen-2.5-coder-32b-instruct:free".to_string(),
        show_ai_commands_output: true,
        context_window_size: 32,
        shell_type: "bash".to_string(),
        require_confirmation: true,
        references: Vec::new(), // Default empty references
    };

    let config_path = get_config_path();
    if !config_path.exists() {
        if let Some(parent) = config_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                println!(
                    "{}",
                    format!("Failed to create config directory '{}': {}", parent.display(), e).red()
                );
            } else {
                let default_json = serde_json::to_string_pretty(&config).unwrap_or_else(|_| "{}".to_string());
                if let Err(e) = std::fs::write(&config_path, default_json) {
                    println!(
                        "{}",
                        format!("Failed to write default config to '{}': {}", config_path.display(), e).red()
                    );
                } else {
                    println!(
                        "{}",
                        format!("Created default config at '{}'", config_path.display()).green()
                    );
                }
            }
        }
    }

    if let Ok(json) = std::fs::read_to_string(&config_path) {
        let stripped = StripComments::new(json.as_bytes());
        match serde_json::from_reader(stripped) {
            Ok(file_config) => config = file_config,
            Err(e) => println!(
                "{} {} {}\n{}",
                "Failed to parse".red(),
                config_path.display().to_string().red(),
                format!(": {}", e).red(),
                "Using default config"
            ),
        }
    }

    if let Ok(debug) = std::env::var("AIOSC_DEBUG") { config.debug = debug.to_lowercase() == "true"; }
    if let Ok(api_addr) = std::env::var("AIOSC_API_ADDR") { config.api_addr = api_addr; }
    if let Ok(api_key) = std::env::var("AIOSC_API_KEY") { config.api_key = api_key; }
    if let Ok(model) = std::env::var("AIOSC_MODEL") { config.model = model; }
    if let Ok(show) = std::env::var("AIOSC_SHOW_AI_COMMANDS_OUTPUT") { config.show_ai_commands_output = show.to_lowercase() == "true"; }
    if let Ok(size) = std::env::var("AIOSC_CONTEXT_WINDOW_SIZE") { if let Ok(n) = size.parse() { config.context_window_size = n; } }
    if let Ok(shell_type) = std::env::var("AIOSC_SHELL_TYPE") { config.shell_type = shell_type; }
    if let Ok(confirm) = std::env::var("AIOSC_REQUIRE_CONFIRMATION") { config.require_confirmation = confirm.to_lowercase() == "true"; }

    config
}