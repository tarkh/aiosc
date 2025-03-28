use cli::run_cli;
use config::load_config;
use colored::Colorize;
mod config;
mod message;
mod cli;
mod llm;
mod executor;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config();
    println!(
        "{}",
        format!(
            "{}\n{}\n{}\n{}\nv{}",
            "   _   ___ __   ___  ___",
            "  /_\\ |_ _/ _ \\/ __|/ __|",
            " / _ \\ | | (_) \\__ \\ (__ ",
            "/_/ \\_\\___\\___/|___/\\___|",
            env!("CARGO_PKG_VERSION")
        )
        .blue()
    );
    run_cli(config)?;
    println!("{}", "Goodbye!".blue());
    Ok(())
}