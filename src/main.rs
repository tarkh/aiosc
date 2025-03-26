use std::io::{self, Read, Write};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use colored::Colorize;
use json_comments::StripComments;
use serde::{Serialize, Deserialize};
use reqwest::blocking::Client;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use ptyprocess::PtyProcess;
use nix::pty::Winsize;
use nix::sys::termios;
use nix::poll::{poll, PollFd, PollFlags};
use nix::fcntl::{fcntl, FcntlArg, F_GETFL, OFlag};
use std::os::unix::io::BorrowedFd;
use std::os::fd::AsRawFd;

#[derive(Serialize, Deserialize, Default)]
struct Config {
    debug: bool,
    api_addr: String,
    api_key: String,
    model: String,
    show_ai_commands_output: bool,
    context_window_size: usize,
    shell_type: String,
    require_confirmation: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Message {
    role: String,
    content: String,
}

fn get_config_path() -> PathBuf {
    if let Ok(path) = std::env::var("AIOSC_CONFIG_PATH") {
        return PathBuf::from(path);
    }

    let mut config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir.push("aiosc");
    config_dir.push("aiosc.config.json");

    if config_dir.exists() {
        return config_dir;
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let mut local_path = exe_dir.to_path_buf();
            local_path.push("aiosc.config.json");
            return local_path;
        }
    }

    config_dir
}

fn load_config() -> Config {
    let mut config = Config {
        debug: false,
        api_addr: "https://openrouter.ai/api/v1".to_string(),
        api_key: "".to_string(),
        model: "qwen/qwen-2.5-coder-32b-instruct:free".to_string(),
        show_ai_commands_output: true,
        context_window_size: 32,
        shell_type: "bash".to_string(),
        require_confirmation: true,
    };

    let config_path = get_config_path();
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

    if let Ok(debug) = std::env::var("AIOSC_DEBUG") {
        config.debug = debug.to_lowercase() == "true";
    }
    if let Ok(api_addr) = std::env::var("AIOSC_API_ADDR") {
        config.api_addr = api_addr;
    }
    if let Ok(api_key) = std::env::var("AIOSC_API_KEY") {
        config.api_key = api_key;
    }
    if let Ok(model) = std::env::var("AIOSC_MODEL") {
        config.model = model;
    }
    if let Ok(show_ai_commands_output) = std::env::var("AIOSC_SHOW_AI_COMMANDS_OUTPUT") {
        config.show_ai_commands_output = show_ai_commands_output.to_lowercase() == "true";
    }
    if let Ok(context_window_size) = std::env::var("AIOSC_CONTEXT_WINDOW_SIZE") {
        if let Ok(size) = context_window_size.parse::<usize>() {
            config.context_window_size = size;
        }
    }
    if let Ok(shell_type) = std::env::var("AIOSC_SHELL_TYPE") {
        config.shell_type = shell_type;
    }
    if let Ok(require_confirmation) = std::env::var("AIOSC_REQUIRE_CONFIRMATION") {
        config.require_confirmation = require_confirmation.to_lowercase() == "true";
    }

    config
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = load_config();
    let mut rl = DefaultEditor::new()?;
    let info = os_info::get();
    let cwd = std::env::current_dir()?;
    let os_info = format!(
        "- OS Type: {}\n- Platform: {}\n- Release: {}\n- Hostname: {}\n- Shell: {}\n- Working Directory: {}",
        std::env::consts::FAMILY,
        std::env::consts::OS,
        format!("{} {} [{}]", info.os_type(), info.version(), info.bitness()),
        hostname::get()?.to_string_lossy(),
        config.shell_type,
        cwd.display()
    );

    let mut conversation = vec![
    Message {
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
    }
];

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
                    }
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
                    }
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
                    }
                    input if input.starts_with("cmd ") => {
                        let command = input[4..].trim();
                        println!(
                            "{}",
                            format!("[Direct] {}", command).truecolor(128, 128, 128)
                        );
                        let output = execute_command(&config, command, false, true)?;
                        println!("{}", output.white());
                    }
                    _ => {
                        trim_conversation(&config, &mut conversation);
                        conversation.push(Message {
                            role: "user".to_string(),
                            content: input.to_string(),
                        });
                        let response = query_llm(&config, &conversation)?;
                        process_response(&config, &mut conversation, response)?;
                    }
                }
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(e) => return Err(Box::new(e)),
        }
    }

    println!("{}", "Goodbye!".blue());
    Ok(())
}

fn trim_conversation(config: &Config, conversation: &mut Vec<Message>) {
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

fn query_llm(config: &Config, conversation: &[Message]) -> Result<String, Box<dyn Error>> {
    let client = Client::new();
    let url = format!("{}/chat/completions", config.api_addr);

    if config.debug {
        let pretty_in = serde_json::to_string_pretty(conversation)?;
        println!(
            "{}",
            format!("[API request]\n{}", pretty_in).truecolor(128, 128, 128)
        );
    }

    let mut request = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "model": &config.model, "messages": conversation }));

    if !config.api_key.is_empty() {
        request = request.header("Authorization", format!("Bearer {}", config.api_key));
    }

    let res = request.send().map_err(|e| Box::new(e) as Box<dyn Error>)?;

    let json: serde_json::Value = res
        .json()
        .map_err(|e| Box::new(e) as Box<dyn Error>)?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(content)
}

fn execute_command(
    config: &Config,
    command: &str,
    needs_full_context: bool,
    user_command: bool,
) -> Result<String, Box<dyn Error>> {
    let trimmed_command = command.trim();
    if trimmed_command.starts_with("cd ") {
        return Ok("Error: 'cd' is not allowed. Use absolute or relative paths instead.".to_string());
    }

    let (shell, shell_arg) = match config.shell_type.to_lowercase().as_str() {
        "bash" | "zsh" => (config.shell_type.as_str(), "-c"),
        "cmd" => ("cmd.exe", "/c"),
        "powershell" => ("powershell.exe", "-Command"),
        _ => {
            println!(
                "{}",
                format!(
                    "Unsupported SHELL_TYPE: {}. Defaulting to 'bash'.",
                    config.shell_type
                )
                .red()
            );
            ("bash", "-c")
        }
    };

    let mut cmd = Command::new(shell);
    cmd.arg(shell_arg).arg(trimmed_command);
    let mut process = PtyProcess::spawn(cmd)?;

    // Get terminal size
    let mut winsize = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe {
        nix::libc::ioctl(0, nix::libc::TIOCGWINSZ, &mut winsize as *mut _);
    }
    process.set_window_size(winsize.ws_col as u16, winsize.ws_row as u16)?;

    // Configure PTY with echo
    let mut pty = process.get_pty_stream()?;
    let pty_fd = pty.as_raw_fd();
    let pty_borrowed_fd = unsafe { BorrowedFd::borrow_raw(pty_fd) };
    let mut pty_termios = termios::tcgetattr(pty_borrowed_fd)?;
    pty_termios.local_flags |= termios::LocalFlags::ECHO; // Enable echo
    termios::tcsetattr(pty_borrowed_fd, termios::SetArg::TCSANOW, &pty_termios)?;

    // Set PTY to non-blocking
    let flags = fcntl(pty_fd, F_GETFL)?;
    fcntl(
        pty_fd,
        FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK),
    )?;

    // Enter raw mode for stdin
    let mut stdin = io::stdin();
    let original_termios = termios::tcgetattr(&stdin)?;
    let mut raw = original_termios.clone();
    termios::cfmakeraw(&mut raw);
    termios::tcsetattr(&stdin, termios::SetArg::TCSANOW, &raw)?;

    let mut output = String::new();
    let mut buffer = [0; 4096];
    let mut input_buffer = [0; 1024];

    // Poll stdin and PTY
    let stdin_fd = stdin.as_raw_fd();
    let stdin_borrowed_fd = unsafe { BorrowedFd::borrow_raw(stdin_fd) };
    let mut poll_fds = [
        PollFd::new(stdin_borrowed_fd, PollFlags::POLLIN),
        PollFd::new(pty_borrowed_fd, PollFlags::POLLIN | PollFlags::POLLHUP),
    ];

    let mut running = true;
    while running {
        match poll(&mut poll_fds, 100u16)? {
            n if n > 0 => {
                // Handle stdin (user input)
                if poll_fds[0]
                    .revents()
                    .unwrap_or(PollFlags::empty())
                    .contains(PollFlags::POLLIN)
                {
                    match stdin.read(&mut input_buffer) {
                        Ok(n) if n > 0 => {
                            let input = &input_buffer[..n];
                            pty.write_all(input)?; // Send input to PTY
                            pty.flush()?;
                        }
                        Ok(_) => break, // EOF (Ctrl+D)
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                        Err(e) => {
                            termios::tcsetattr(&stdin, termios::SetArg::TCSANOW, &original_termios)?;
                            return Err(Box::new(e));
                        }
                    }
                }

                // Handle PTY output
                if poll_fds[1]
                    .revents()
                    .unwrap_or(PollFlags::empty())
                    .contains(PollFlags::POLLIN)
                {
                    match pty.read(&mut buffer) {
                        Ok(n) if n > 0 => {
                            let chunk = String::from_utf8_lossy(&buffer[..n]);
                            if config.show_ai_commands_output || user_command {
                                print!("{}", chunk);
                                io::stdout().flush()?;
                            }
                            output.push_str(&chunk);
                        }
                        Ok(_) => running = false, // EOF from PTY
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                        Err(e) => {
                            termios::tcsetattr(&stdin, termios::SetArg::TCSANOW, &original_termios)?;
                            return Err(Box::new(e));
                        }
                    }
                }

                // Check if process exited
                if poll_fds[1]
                    .revents()
                    .unwrap_or(PollFlags::empty())
                    .contains(PollFlags::POLLHUP)
                {
                    running = false;
                }
            }
            0 => {} // Timeout
            _ => running = false, // Error or interrupt
        }
    }

    // Restore terminal and get status
    termios::tcsetattr(&stdin, termios::SetArg::TCSANOW, &original_termios)?;
    let status = process.wait()?;
    match status {
        ptyprocess::WaitStatus::Exited(_, code) => Ok(if code == 0 && !needs_full_context {
            "Success".to_string()
        } else if code == 0 && needs_full_context {
            output
        } else {
            format!("Error: Exit code {}\n{}", code, output)
        }),
        _ => Ok(format!("Error: Process terminated abnormally\n{}", output)),
    }
}

fn process_response(
    config: &Config,
    conversation: &mut Vec<Message>,
    response: String,
) -> Result<(), Box<dyn Error>> {
    let cmd_match = response
        .match_indices("<cmd>")
        .next()
        .map(|(i, _)| (i, "</cmd>"));
    let cmdctx_match = response
        .match_indices("<cmdctx>")
        .next()
        .map(|(i, _)| (i, "</cmdctx>"));

    let (start, end_tag, needs_full_context) = if let Some((start, end_tag)) = cmd_match {
        (start, end_tag, false)
    } else if let Some((start, end_tag)) = cmdctx_match {
        (start, end_tag, true)
    } else {
        println!("{}", response.yellow());
        conversation.push(Message {
            role: "assistant".to_string(),
            content: response,
        });
        return Ok(());
    };

    let end = response[start..]
        .find(end_tag)
        .unwrap_or(response.len()) + end_tag.len();
    let command = response[start + end_tag.len() - 1..start + end - end_tag.len() + 0].trim();
    let text = response[..start].trim();
    if !text.is_empty() {
        println!("{}", text.yellow());
    }
    println!(
        "{}",
        format!(
            "[Executing{}] {}",
            if needs_full_context { " (fo)" } else { "" },
            command
        )
        .truecolor(128, 128, 128)
    );

    let should_execute = if config.require_confirmation {
        print!(
            "{}",
            format!(
                "Execute '{}'? Press Enter to confirm, any key + Enter to abort: ",
                command
            )
            .cyan()
        );
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().is_empty()
    } else {
        true
    };

    trim_conversation(config, conversation);
    if should_execute {
        let output = execute_command(config, command, needs_full_context, false)?;
        conversation.push(Message {
            role: "assistant".to_string(),
            content: format!(
                "<{}>{}</{}>",
                if needs_full_context { "cmdctx" } else { "cmd" },
                command,
                if needs_full_context { "cmdctx" } else { "cmd" }
            ),
        });
        conversation.push(Message {
            role: "tool".to_string(),
            content: output.clone(),
        });
        let next_response = query_llm(config, conversation)?;
        process_response(config, conversation, next_response)?;
    } else {
        conversation.push(Message {
            role: "assistant".to_string(),
            content: "Command aborted by user.".to_string(),
        });
    }
    trim_conversation(config, conversation);

    Ok(())
}