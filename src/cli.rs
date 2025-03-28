use std::io::{self, Write};
use rustyline::{DefaultEditor, error::ReadlineError};
use colored::Colorize;
use crate::{config::Config, message::Message, llm::query_llm, executor::execute_command};

pub fn run_cli(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut rl = DefaultEditor::new()?;
    let info = os_info::get();
    let cwd = std::env::current_dir()?;
    let os_info = format!(
        "- OS Type: {}\n- Platform: {}\n- Release: {}\n- Hostname: {}\n- Shell: {}\n- Working Directory: {}",
        std::env::consts::FAMILY, std::env::consts::OS,
        format!("{} {} [{}]", info.os_type(), info.version(), info.bitness()),
        hostname::get()?.to_string_lossy(), config.shell_type, cwd.display()
    );

    let mut conversation = vec![Message {
        role: "system".to_string(),
        content: format!(
          "You are a CLI assistant running on the following operating system and shell:\n{}\n\n\
          You have two CLI tools to execute shell commands. Use them strictly as follows:\n\
          - `<cmd>...</cmd>`: Runs a command and returns only success or error status. Use this when you only need to confirm the command executed successfully (e.g., file creation, deletion, or running scripts without output analysis).\n\
          - `<cmdctx>...</cmdctx>`: Runs a command and returns the full output. Use this *only* when you must analyze the output to proceed (e.g., reading file contents, checking system status, or listing data for further decisions).\n\n\
          **Strict Guidelines**:\n\
          - Always prefer `<cmd>` to minimize context size. Use `<cmdctx>` only when the task explicitly requires output analysis.\n\
          - For `<cmdctx>`, minimize output using shell tools (e.g., `grep`, `head`, `tail`, `awk`) or redirect to a file and process parts of it if the output might be large.\n\
          - Use absolute paths in all commands. Do not use `cd`. You are anchored to: {}\n\
          - Execute exactly one command per response in the specified format.\n\
          - Analyze `<cmdctx>` output in subsequent steps to decide next actions.\n\
          - For multi-turn tasks (e.g., needing a commit message), ask for clarification and wait for user input.\n\
          - Stop when the task is complete (respond without command tags).\n\
          - If a command fails multiple times (e.g., 2+ attempts), stop and report the issue.\n\
          - Warn and ask for confirmation if a command risks harm (e.g., overwriting data).\n\
          - Keep responses concise, focusing only on the current task. Context is limited to recent messages.\n\
          - Use commands compatible with the OS and shell specified above.\n\n\
          **Examples**:\n\
          - 'check if process my_app is running' → 'Checking process...\\n<cmdctx>ps aux | grep my_app</cmdctx>'\n\
          - 'analyze ping' → 'Pinging...\\n<cmdctx>ping -c 4 google.com</cmdctx>'\n\
          - 'describe directory' → 'Describing directory...\\n<cmdctx>ls -la</cmdctx>'\n\
          - 'list directory' → '<cmd>ls -la</cmd>'\n\
          - 'create a directory named test' → 'Creating directory...\\n<cmd>mkdir {}/test</cmd>'\n\
          - 'show first 5 lines of log.txt' → '<cmd>head -n 5 {}/log.txt</cmd>'\n\
          - 'build a project and save output' → 'Building...\\n<cmd>make > {}/build.log 2>&1</cmd>'\n\
          - 'check build errors' → 'Checking errors...\\n<cmdctx>grep \"error\" {}/build.log</cmdctx>'\n",
          os_info,
          cwd.display(),
          cwd.display(),
          cwd.display(),
          cwd.display(),
          cwd.display()
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
                      if let Err(e) = std::env::set_current_dir(path) {
                          println!(
                              "{}",
                              format!("Failed to change directory to '{}': {}", path, e).red()
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
    let cmd_match = response.match_indices("<cmd>").next().map(|(i, _)| (i, "</cmd>"));
    let cmdctx_match = response.match_indices("<cmdctx>").next().map(|(i, _)| (i, "</cmdctx>"));
    let (start, end_tag, needs_full_context) = if let Some((start, end_tag)) = cmd_match {
        (start, end_tag, false)
    } else if let Some((start, end_tag)) = cmdctx_match {
        (start, end_tag, true)
    } else {
        println!("{}", response.yellow());
        conversation.push(Message { role: "assistant".to_string(), content: response });
        return Ok(());
    };

    let end = response[start..].find(end_tag).unwrap_or(response.len()) + end_tag.len();
    let command = response[start + end_tag.len() - 1..start + end - end_tag.len()].trim();
    let text = response[..start].trim();
    if !text.is_empty() { println!("{}", text.yellow()); }
    println!("{}", format!("[Executing{}] {}", if needs_full_context { " (fo)" } else { "" }, command).truecolor(128, 128, 128));

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
            content: format!("<{}>{}</{}>", if needs_full_context { "cmdctx" } else { "cmd" }, command, if needs_full_context { "cmdctx" } else { "cmd" }),
        });
        conversation.push(Message { role: "tool".to_string(), content: output.clone() });
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