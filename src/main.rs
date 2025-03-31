use cli::{run_cli, run_non_interactive};
use config::load_config;
use colored::Colorize;
mod config;
mod message;
mod cli;
mod llm;
mod executor;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config();
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        let mut no_confirm = false;
        let mut silent = false;
        let mut prompt_parts = Vec::new();
        let mut i = 1;

        // Parse flags and collect prompt
        while i < args.len() {
            match args[i].as_str() {
                "--no-confirm" => no_confirm = true,
                "--silent" => silent = true,
                _ => prompt_parts.push(args[i].clone()),
            }
            i += 1;
        }

        if prompt_parts.is_empty() {
            println!("{}", "Error: No prompt provided after flags.".red());
            return Ok(());
        }

        let prompt = prompt_parts.join(" ");
        if no_confirm {
            config.require_confirmation = false;
        }
        run_non_interactive(config, &prompt, silent)?;
    } else {
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
    }

    Ok(())
}