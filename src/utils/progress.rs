use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::time::Duration;

/// Style for spinning tasks (while running)
fn spinner_style() -> ProgressStyle {
    ProgressStyle::default_spinner()
        .template("  {spinner:.cyan} {wide_msg}")
        .expect("valid template")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
}

/// Style for finished tasks (no spinner)
fn finished_style() -> ProgressStyle {
    ProgressStyle::default_spinner()
        .template("{msg}")
        .expect("valid template")
}

/// A task being tracked with a spinner
pub struct TaskSpinner {
    bar: ProgressBar,
    name: String,
}

impl TaskSpinner {
    /// Mark task as successful with elapsed time
    pub fn succeed(&self, elapsed_secs: f32) {
        self.bar.set_style(finished_style());
        self.bar.finish_with_message(format!(
            "  {} {:<14} {}",
            "✓".green(),
            self.name,
            format!("{:.1}s", elapsed_secs).dimmed()
        ));
    }

    /// Mark task as successful with a custom message
    pub fn succeed_with_message(&self, message: String) {
        self.bar.set_style(finished_style());
        self.bar.finish_with_message(message);
    }

    /// Mark task as failed with elapsed time
    pub fn fail(&self, elapsed_secs: f32) {
        self.bar.set_style(finished_style());
        self.bar.finish_with_message(format!(
            "  {} {:<14} {}",
            "✗".red(),
            self.name,
            format!("{:.1}s", elapsed_secs).dimmed()
        ));
    }

    /// Mark task as skipped with reason
    pub fn skip(&self, reason: &str) {
        self.bar.set_style(finished_style());
        self.bar.finish_with_message(format!(
            "  {} {:<14} {}",
            "⊘".yellow(),
            self.name,
            reason.dimmed()
        ));
    }

    /// Update the spinner message while running (e.g., show current status)
    pub fn set_message(&self, msg: &str) {
        // Truncate message if too long, show last part
        let max_len = 50;
        let display_msg = if msg.len() > max_len {
            format!("…{}", &msg[msg.len() - max_len + 1..])
        } else {
            msg.to_string()
        };
        self.bar
            .set_message(format!("{:<14} {}", self.name, display_msg.dimmed()));
    }

    /// Clear the spinner without showing anything (for skipped tasks)
    pub fn clear(&self) {
        self.bar.set_style(finished_style());
        self.bar.finish_and_clear();
    }
}

/// Manages multiple concurrent task spinners
pub struct TaskProgress {
    mp: MultiProgress,
}

impl TaskProgress {
    pub fn new() -> Self {
        Self {
            mp: MultiProgress::new(),
        }
    }

    /// Add a new task spinner (starts spinning immediately)
    pub fn add_task(&self, name: &str) -> TaskSpinner {
        let bar = self.mp.add(ProgressBar::new_spinner());
        bar.set_style(spinner_style());
        bar.set_message(format!("{:<14}", name));
        bar.enable_steady_tick(Duration::from_millis(80));

        TaskSpinner {
            bar,
            name: name.to_string(),
        }
    }
}

impl Default for TaskProgress {
    fn default() -> Self {
        Self::new()
    }
}

use std::io::{BufRead, BufReader};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;

/// Run a command and update the spinner with the last line of output
pub fn run_command_with_spinner(
    command: &[String],
    envs: &[(&str, &str)],
    spinner: &TaskSpinner,
) -> Result<Output, std::io::Error> {
    let mut cmd = Command::new(&command[0]);
    for arg in &command[1..] {
        cmd.arg(arg);
    }
    for (key, value) in envs {
        cmd.env(key, value);
    }
    // Force color output
    cmd.env("FORCE_COLOR", "1");
    cmd.env("CARGO_TERM_COLOR", "always");

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");

    // Channels to collect output
    let (stdout_tx, stdout_rx) = mpsc::channel::<String>();
    let (stderr_tx, stderr_rx) = mpsc::channel::<String>();

    // Spawn threads to read stdout and stderr
    let stdout_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = stdout_tx.send(line);
        }
    });

    let stderr_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = stderr_tx.send(line);
        }
    });

    let mut stdout_buffer = Vec::new();
    let mut stderr_buffer = Vec::new();

    loop {
        match child.try_wait()? {
            Some(status) => {
                // Process finished, collect remaining output
                while let Ok(line) = stdout_rx.try_recv() {
                    stdout_buffer.push(line);
                }
                while let Ok(line) = stderr_rx.try_recv() {
                    stderr_buffer.push(line);
                }

                let _ = stdout_thread.join();
                let _ = stderr_thread.join();

                let stdout_bytes = stdout_buffer.join("\n").into_bytes();
                let stderr_bytes = stderr_buffer.join("\n").into_bytes();

                return Ok(Output {
                    status,
                    stdout: stdout_bytes,
                    stderr: stderr_bytes,
                });
            }
            None => {
                // Process still running, update spinner with latest output
                let mut last_line = None;

                while let Ok(line) = stdout_rx.try_recv() {
                    last_line = Some(line.clone());
                    stdout_buffer.push(line);
                }
                while let Ok(line) = stderr_rx.try_recv() {
                    // Prefer stderr for status (cargo writes there)
                    last_line = Some(line.clone());
                    stderr_buffer.push(line);
                }

                if let Some(line) = last_line {
                    // Strip ANSI codes for display
                    let clean = strip_ansi_escapes::strip_str(&line);
                    spinner.set_message(&clean);
                }

                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}
