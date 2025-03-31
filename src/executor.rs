use std::io::{self, Read, Write};
use std::process::Command;
use ptyprocess::PtyProcess;
use nix::pty::Winsize;
use nix::sys::termios;
use nix::poll::{poll, PollFd, PollFlags};
use nix::fcntl::{fcntl, FcntlArg, F_GETFL, OFlag};
use std::os::unix::io::BorrowedFd;
use std::os::fd::AsRawFd;
use colored::Colorize;
use crate::config::Config;

pub fn execute_command(
    config: &Config,
    command: &str,
    needs_full_context: bool,
    user_command: bool,
    silent: bool, // Added silent param
) -> Result<String, Box<dyn std::error::Error>> {
    let trimmed_command = command.trim();
    if trimmed_command.starts_with("cd ") {
        return Ok("Error: 'cd' is not allowed. Use absolute or relative paths instead.".to_string());
    }

    let (shell, shell_arg) = match config.shell_type.to_lowercase().as_str() {
        "bash" | "zsh" => (config.shell_type.as_str(), "-c"),
        "cmd" => ("cmd.exe", "/c"),
        "powershell" => ("powershell.exe", "-Command"),
        _ => {
            if !silent {
                println!("{}", format!("Unsupported SHELL_TYPE: {}. Defaulting to 'bash'.", config.shell_type).red());
            }
            ("bash", "-c")
        }
    };

    let mut cmd = Command::new(shell);
    cmd.arg(shell_arg).arg(trimmed_command).envs(std::env::vars());
    let mut process = PtyProcess::spawn(cmd)?;

    let mut winsize = Winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
    unsafe { nix::libc::ioctl(0, nix::libc::TIOCGWINSZ, &mut winsize as *mut _); }
    process.set_window_size(winsize.ws_col as u16, winsize.ws_row as u16)?;

    let mut pty = process.get_pty_stream()?;
    let pty_fd = pty.as_raw_fd();
    let pty_borrowed_fd = unsafe { BorrowedFd::borrow_raw(pty_fd) };
    let mut pty_termios = termios::tcgetattr(pty_borrowed_fd)?;
    pty_termios.local_flags |= termios::LocalFlags::ECHO;
    termios::tcsetattr(pty_borrowed_fd, termios::SetArg::TCSANOW, &pty_termios)?;

    let flags = fcntl(pty_fd, F_GETFL)?;
    fcntl(pty_fd, FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK))?;

    let mut stdin = io::stdin();
    let original_termios = termios::tcgetattr(&stdin)?;
    let mut raw = original_termios.clone();
    termios::cfmakeraw(&mut raw);
    termios::tcsetattr(&stdin, termios::SetArg::TCSANOW, &raw)?;

    let mut output = String::new();
    let mut buffer = [0; 4096];
    let mut input_buffer = [0; 1024];
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
                if poll_fds[0].revents().unwrap_or(PollFlags::empty()).contains(PollFlags::POLLIN) {
                    match stdin.read(&mut input_buffer) {
                        Ok(n) if n > 0 => { pty.write_all(&input_buffer[..n])?; pty.flush()?; },
                        Ok(_) => break,
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {},
                        Err(e) => { termios::tcsetattr(&stdin, termios::SetArg::TCSANOW, &original_termios)?; return Err(Box::new(e)); },
                    }
                }
                if poll_fds[1].revents().unwrap_or(PollFlags::empty()).contains(PollFlags::POLLIN) {
                    match pty.read(&mut buffer) {
                        Ok(n) if n > 0 => {
                            let chunk = String::from_utf8_lossy(&buffer[..n]);
                            // Only print if not silent and (show_ai_commands_output or user_command)
                            if !silent && (config.show_ai_commands_output || user_command) { 
                                print!("{}", chunk); 
                                io::stdout().flush()?; 
                            }
                            output.push_str(&chunk);
                        }
                        Ok(_) => running = false,
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {},
                        Err(e) => { termios::tcsetattr(&stdin, termios::SetArg::TCSANOW, &original_termios)?; return Err(Box::new(e)); },
                    }
                }
                if poll_fds[1].revents().unwrap_or(PollFlags::empty()).contains(PollFlags::POLLHUP) { running = false; }
            }
            0 => {},
            _ => running = false,
        }
    }

    termios::tcsetattr(&stdin, termios::SetArg::TCSANOW, &original_termios)?;
    let status = process.wait()?;
    match status {
        ptyprocess::WaitStatus::Exited(_, code) => Ok(if code == 0 && !needs_full_context {
            if user_command {
                "".to_string()
            } else {
                "Success".to_string()
            }
        } else if code == 0 && needs_full_context {
            output
        } else {
            format!("Error: Exit code {}\n{}", code, output)
        }),
        _ => Ok(format!("Error: Process terminated abnormally\n{}", output)),
    }
}