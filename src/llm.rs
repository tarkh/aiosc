use reqwest::blocking::Client;
use serde_json;
use colored::Colorize;
use crate::{config::Config, message::Message};

pub fn query_llm(config: &Config, conversation: &[Message]) -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    let url = format!("{}/chat/completions", config.api_addr);

    if config.debug {
        let pretty_in = serde_json::to_string_pretty(conversation)?;
        println!("{}", format!("[API request]\n{}", pretty_in).truecolor(128, 128, 128));
    }

    let mut request = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "model": &config.model, "messages": conversation }));

    if !config.api_key.is_empty() {
        request = request.header("Authorization", format!("Bearer {}", config.api_key));
    }

    let res = match request.send() {
        Ok(res) => {
            if res.status().is_success() { res } else {
                return Err(match res.status().as_u16() {
                    401 => Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Invalid API key or authentication failed")),
                    404 => Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "API endpoint not found")),
                    code => Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("API request failed with status code: {}", code))),
                });
            }
        }
        Err(e) => return Err(if e.is_connect() {
            Box::new(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Failed to connect to the LLM server"))
        } else { Box::new(e) }),
    };

    let json: serde_json::Value = res.json().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
    Ok(content)
}