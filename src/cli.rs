use std::io::{self, Write};
//use std::path::Path;
use rustyline::{
    Config as RustyConfig, Editor, error::ReadlineError,
    completion::{Completer, Pair, FilenameCompleter},
    hint::{Hinter, HistoryHinter},
    highlight::{Highlighter, MatchingBracketHighlighter, CmdKind},
    validate::Validator,
    Helper, history::FileHistory
};
use colored::Colorize;
use crate::{config::Config, message::Message, llm::query_llm, executor::execute_command};

struct AioscCompleter {
    filename_completer: FilenameCompleter,
    hinter: HistoryHinter,
    bracket_highlighter: MatchingBracketHighlighter,
}

impl Helper for AioscCompleter {}

impl Completer for AioscCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Built-in special commands (always available at start)
        let special_commands = vec![
            "cd", "cmd", "exit", "help", "reset", "context"
        ];

        // Get the word under the cursor
        let (start, word) = if pos == 0 || line[..pos].ends_with(' ') {
            (pos, "")
        } else {
            let before_cursor = &line[..pos];
            if let Some(last_space) = before_cursor.rfind(' ') {
                (last_space + 1, &before_cursor[last_space + 1..])
            } else {
                (0, before_cursor)
            }
        };

        // Command completion logic
        if start == 0 || line[..start].trim().is_empty() {
            // Case 1: At start of line - only show special commands
            let candidates: Vec<Pair> = special_commands
                .iter()
                .filter(|cmd| cmd.starts_with(word))
                .map(|cmd| Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                })
                .collect();
            
            if !candidates.is_empty() {
                return Ok((start, candidates));
            }
        } else if line.starts_with("cmd ") {
            // Case 2: After "cmd " - show system commands
            let mut system_commands = Vec::new();

            // Load system commands from PATH
            if let Ok(path) = std::env::var("PATH") {
                for dir in std::env::split_paths(&path) {
                    if let Ok(entries) = std::fs::read_dir(dir) {
                        for entry in entries.filter_map(Result::ok) {
                            if let Ok(file_type) = entry.file_type() {
                                if file_type.is_file() || file_type.is_symlink() {
                                    if let Some(name) = entry.file_name().into_string().ok() {
                                        system_commands.push(name);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            system_commands.sort_unstable();
            system_commands.dedup();

            let candidates: Vec<Pair> = system_commands
                .into_iter()
                .filter(|cmd| cmd.starts_with(word))
                .map(|cmd| Pair {
                    display: cmd.clone(),
                    replacement: cmd,
                })
                .collect();

            if !candidates.is_empty() {
                return Ok((start, candidates));
            }
        }

        // Fallback to filename completion
        self.filename_completer.complete(line, pos, ctx)
    }
}

impl Hinter for AioscCompleter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &rustyline::Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for AioscCompleter {
    fn highlight_hint<'h>(&self, hint: &'h str) -> std::borrow::Cow<'h, str> {
        std::borrow::Cow::Owned(hint.truecolor(128, 128, 128).to_string())
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> std::borrow::Cow<'l, str> {
        self.bracket_highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, forced: CmdKind) -> bool {
        self.bracket_highlighter.highlight_char(line, pos, forced)
    }
}

impl Validator for AioscCompleter {}

pub fn run_cli(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let rusty_config = RustyConfig::builder()
    .completion_type(rustyline::CompletionType::List)
    .build();

    let mut rl: Editor<AioscCompleter, FileHistory> = Editor::with_config(rusty_config)?;
    rl.set_helper(Some(AioscCompleter {
        filename_completer: FilenameCompleter::new(),
        hinter: HistoryHinter {},
        bracket_highlighter: MatchingBracketHighlighter::new(),
    }));

    let info = os_info::get();
    let cwd = std::env::current_dir()?;
    let os_info = format!(
        "- OS Type: {}\n- Platform: {}\n- Release: {}\n- Hostname: {}\n- Shell: {}\n- Working Directory: {}",
        std::env::consts::FAMILY, std::env::consts::OS,
        format!("{} {} [{}]", info.os_type(), info.version(), info.bitness()),
        hostname::get()?.to_string_lossy(), config.shell_type, cwd.display()
    );

    // Build references section from config
    let references_section = if config.references.is_empty() {
        "".to_string()
    } else {
        let mut section = String::from("\n**Strict Custom Command References**:\n");
        for ref_item in &config.references {
            section.push_str(&format!("- `{}`: {}\n", ref_item.command, ref_item.description));
        }
        section
    };

    let mut conversation = vec![Message {
        role: "system".to_string(),
        content: format!(
            "You are a CLI assistant running on the following operating system and shell:\n{}\n\n\
            You have two CLI tools to execute shell commands. Use them strictly as follows:\n\
            - <cmd>...</cmd>: Runs a command and returns only success or error status. Use this when you only need to confirm the command executed successfully (e.g., file creation, deletion).\n\
            - <cmdctx>...</cmdctx>: Runs a command and returns the full output. Use this *only* when you must analyze the output to proceed (e.g., reading file contents, checking system status).\n\n\
            - IMPORTANT: Only one tag per response is allowed.
            **Strict Guidelines**:\n\
            - Always prefer <cmd> to minimize context size. Use <cmdctx> only when output analysis is required.\n\
            - For <cmdctx>, minimize output with shell tools (e.g., `grep`, `head`) or redirect to a file.\n\
            - Use absolute paths in all commands. Do not use `cd`. You are anchored to: {}\n\
            - Execute one command per response in the specified format.\n\
            - Analyze <cmdctx> output in subsequent steps.\n\
            - For multi-turn tasks, ask for clarification and wait for input.\n\
            - Stop when the task is complete (no command tags).\n\
            - If a command fails multiple times (2+), stop and report it.\n\
            - Warn and ask for confirmation if a command risks harm (e.g., overwriting data).\n\
            - Keep responses concise. Context is limited to recent messages.\n\
            - Use commands compatible with the OS and shell above.\n\n\
            **Examples**:\n\
            - create a directory named test: Creating directory...\\n<cmd>mkdir {}/test</cmd>\n\
            - show first 5 lines of log.txt: <cmd>head -n 5 {}/log.txt</cmd>\n\
            - check process status: Checking process...\\n<cmdctx>ps aux | grep my_app</cmdctx>\n\
            {}\n",
            os_info,
            cwd.display(),
            cwd.display(),
            cwd.display(),
            references_section
      ),
    }];

    loop {
        match rl.readline(&"aiosc> ".green()) {
            Ok(line) => {
                let input = line.trim();
                rl.add_history_entry(input)?;
                match input {
                    "exit" => break,
                    "help" => println!(
                        "{}",
                        "Workflow:\n\
                        - Enter your request into the prompt and wait for the AI to perform the action\n\
                        \n\
                        Available commands:\n\
                        - cd <path>: Change the working directory\n\
                        - cmd <command>: Execute a shell command directly\n\
                        - exit: Exit the program\n\
                        - reset: Clear chat history\n\
                        - context: Show current conversation context\n\
                        - help: Show this help message\n"
                            .blue()
                    ),
                    "reset" => {
                        conversation.truncate(1);
                        println!("{}", "Chat history cleared.".yellow());
                    },
                    "context" => {
                        if conversation.len() == 1 {
                            println!(
                                "{}",
                                "No conversation history yet (only system prompt)."
                                    .truecolor(128, 128, 128)
                            );
                        } else {
                            println!("{}", "--- Current Chat Context ---".yellow());
                            for (i, msg) in conversation.iter().skip(1).enumerate() {
                                println!(
                                    "{}[{}] {}:",
                                    "--- ".yellow(),
                                    i + 1,
                                    msg.role.to_uppercase().cyan()
                                );
                                println!("{}", msg.content.white());
                            }
                            println!("{}", "--- End of Context ---".yellow());
                        }
                    },
                    input if input.starts_with("cd ") => {
                      let path = input[3..].trim();
                        let expanded_path = if path == "~" {
                            dirs::home_dir().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Home directory not found"))?
                        } else if path.starts_with("~/") {
                            let mut home = dirs::home_dir().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Home directory not found"))?;
                            home.push(&path[2..]);
                            home
                        } else {
                            std::path::PathBuf::from(path)
                        };

                        if let Err(e) = std::env::set_current_dir(&expanded_path) {
                          println!(
                              "{}",
                                format!("Failed to change directory to '{}': {}", expanded_path.display(), e).red()
                          );
                      } else {
                          println!(
                              "{}",
                              format!("Changed directory to: {}", std::env::current_dir()?.display())
                                  .truecolor(128, 128, 128)
                          );
                      }
                    },
                    input if input.starts_with("cmd ") => {
                        let command = input[4..].trim();
                        println!("{}", format!("[Direct] {}", command).truecolor(128, 128, 128));
                        let output = execute_command(&config, command, false, true)?;
                        println!("{}", output.white());
                    },
                    _ => {
                        trim_conversation(&config, &mut conversation);
                        conversation.push(Message { role: "user".to_string(), content: input.to_string() });
                        match query_llm(&config, &conversation) {
                            Ok(response) => process_response(&config, &mut conversation, response)?,
                            Err(e) => println!("{}", format!("LLM error: {}", e).red()),
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(e) => return Err(Box::new(e)),
        }
    }
    Ok(())
}

pub fn trim_conversation(config: &Config, conversation: &mut Vec<Message>) {
    let system_message = conversation[0].clone();
    let non_system_messages = &conversation[1..];
    let max_messages = config.context_window_size;
    if non_system_messages.len() > max_messages {
        let messages_to_remove = non_system_messages.len() - max_messages;
        *conversation = std::iter::once(system_message)
            .chain(non_system_messages.iter().skip(messages_to_remove).cloned())
            .collect();
    }
}

pub fn process_response(config: &Config, conversation: &mut Vec<Message>, response: String) -> Result<(), Box<dyn std::error::Error>> {
    // Find command tags with their lengths
    let cmd_match = response.match_indices("<cmd>").next().map(|(i, _)| (i, "</cmd>", 5));
    let cmdctx_match = response.match_indices("<cmdctx>").next().map(|(i, _)| (i, "</cmdctx>", 8));
    
    let (start, end_tag, open_tag_len, needs_full_context) = match (cmd_match, cmdctx_match) {
        (Some((start, end_tag, open_len)), _) => (start, end_tag, open_len, false),
        (_, Some((start, end_tag, open_len))) => (start, end_tag, open_len, true),
        _ => {
            println!("{}", response.yellow());
            conversation.push(Message { role: "assistant".to_string(), content: response });
            return Ok(());
        }
    };

    // Find closing tag
    let Some(closing_pos) = response[start..].find(end_tag) else {
        println!("{}", response.yellow());
        conversation.push(Message { role: "assistant".to_string(), content: response });
        return Ok(());
    };

    // Calculate command boundaries
    let command_start = start + open_tag_len;
    let command_end = start + closing_pos;
    let command = response[command_start..command_end].trim();

    // Print non-command text if present
    let text = response[..start].trim();
    if !text.is_empty() { 
        println!("{}", text.yellow()); 
    }
    
    println!("{}", format!("[Executing{}] {}", 
        if needs_full_context { " (fo)" } else { "" }, 
        command
    ).truecolor(128, 128, 128));

    // Rest of the function remains unchanged...
    let should_execute = if config.require_confirmation {
        print!("{}", format!("Execute '{}'? Press Enter to confirm, any key + Enter to abort: ", command).cyan());
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().is_empty()
    } else { true };

    trim_conversation(config, conversation);
    if should_execute {
        let output = execute_command(config, command, needs_full_context, false)?;
        conversation.push(Message {
            role: "assistant".to_string(),
            content: format!("<{}>{}</{}>", 
                if needs_full_context { "cmdctx" } else { "cmd" }, 
                command, 
                if needs_full_context { "cmdctx" } else { "cmd" }
            ),
        });
        conversation.push(Message { role: "tool".to_string(), content: output.clone() });

        // Cooldown logic
        if ! config.require_confirmation && config.cooldown > 0 {
            println!("{}", format!("Waiting for {} seconds due to cooldown...", config.cooldown).truecolor(128, 128, 128));
            std::thread::sleep(std::time::Duration::from_secs(config.cooldown));
        }

        match query_llm(config, conversation) {
            Ok(next_response) => process_response(config, conversation, next_response)?,
            Err(e) => println!("{}", format!("LLM error: {}", e).red()),
        }
    } else {
        conversation.push(Message { role: "assistant".to_string(), content: "Command aborted by user.".to_string() });
    }
    trim_conversation(config, conversation);
    Ok(())
}