use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

fn color_enabled(stderr: bool) -> bool {
    if std::env::var_os("NO_COLOR").is_some()
        || std::env::var("TERM").is_ok_and(|term| term == "dumb")
    {
        return false;
    }

    if stderr {
        std::io::stderr().is_terminal()
    } else {
        std::io::stdout().is_terminal()
    }
}

fn paint(text: &str, code: &str, stderr: bool) -> String {
    if color_enabled(stderr) {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

pub fn banner(command: &str, subtitle: &str) {
    println!();
    println!(
        "  {} {} {}",
        paint("◆", "38;5;45", false),
        paint("ANVIL", "1;38;5;45", false),
        paint(&format!("/ {command}"), "1;97", false)
    );

    if !subtitle.is_empty() {
        println!("    {}", paint(subtitle, "38;5;250", false));
    }

    println!(
        "  {}",
        paint(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            "38;5;39",
            false
        )
    );
}

pub fn section(title: &str) {
    println!(
        "\n  {} {}",
        paint("▸", "1;38;5;45", false),
        paint(title, "1;97", false)
    );
}

pub fn key_value(key: &str, value: &str) {
    println!(
        "    {} {}",
        paint(&format!("{key:<11}"), "38;5;110", false),
        value
    );
}

pub fn success(message: &str) {
    println!("  {} {}", paint("✔", "1;32", false), message);
}

pub fn warning(message: &str) {
    println!("  {} {}", paint("⚠", "1;33", false), message);
}

pub fn info(message: &str) {
    println!("  {} {}", paint("ℹ", "1;36", false), message);
}

pub fn status(passed: bool, label: &str, detail: &str) {
    let (symbol, code) = if passed {
        ("✔", "1;32")
    } else {
        ("✖", "1;31")
    };

    println!("  {} {:<15} {}", paint(symbol, code, false), label, detail);
}

pub fn fatal(error: &anyhow::Error) {
    eprintln!();
    eprintln!(
        "  {} {}",
        paint("✖", "1;31", true),
        paint("Anvil command failed", "1;31", true)
    );

    for line in format!("{error:#}").lines() {
        eprintln!("    {}", paint(line, "38;5;252", true));
    }

    eprintln!();
}

fn print_tail(label: &str, output: &[u8]) {
    let text = String::from_utf8_lossy(output);
    let lines: Vec<&str> = text.lines().collect();

    if lines.is_empty() {
        return;
    }

    eprintln!("\n{label}:");
    let start = lines.len().saturating_sub(12);

    for line in &lines[start..] {
        eprintln!("  {line}");
    }
}

fn elapsed(duration: Duration) -> String {
    format!("{:.2}s", duration.as_secs_f64())
}

pub struct BuildProgress {
    bar: ProgressBar,
    total: u64,
    started: Instant,
    stages: Vec<(String, Duration)>,
    verbose: bool,
}

impl BuildProgress {
    pub fn new(total: u64, verbose: bool) -> Result<Self> {
        let bar = ProgressBar::new(total);

        bar.set_style(
            ProgressStyle::with_template(
                "  {spinner:.cyan} {msg}\n  {bar:40.cyan/blue} {pos}/{len} stages",
            )?
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .progress_chars("━╸─"),
        );

        if verbose {
            bar.set_draw_target(ProgressDrawTarget::hidden());
        } else {
            bar.enable_steady_tick(Duration::from_millis(85));
        }

        Ok(Self {
            bar,
            total,
            started: Instant::now(),
            stages: Vec::with_capacity(total as usize),
            verbose,
        })
    }

    pub fn run_step(&mut self, command: &mut Command, label: &str) -> Result<()> {
        let stage = self.bar.position() + 1;
        let message = format!("[{stage}/{}] {label}", self.total);
        let started = Instant::now();

        if self.verbose {
            println!("  {}", paint(&message, "1;38;5;45", false));
        } else {
            self.bar.set_message(message);
        }

        let result = if self.verbose {
            command
                .status()
                .with_context(|| format!("Unable to start {label}"))
                .and_then(|status| {
                    if status.success() {
                        Ok(())
                    } else {
                        bail!("{label} failed ({status})")
                    }
                })
        } else {
            match command.output() {
                Ok(output) if output.status.success() => Ok(()),
                Ok(output) => {
                    print_tail("Error output", &output.stderr);
                    print_tail("Command output", &output.stdout);

                    Err(anyhow::anyhow!(
                        "{label} failed ({}). Run again with `--verbose` for full output.",
                        output.status
                    ))
                }
                Err(error) => Err(error).with_context(|| format!("Unable to start {label}")),
            }
        };

        let duration = started.elapsed();

        if let Err(error) = result {
            self.bar.finish_and_clear();
            eprintln!(
                "  {} {label} ({})",
                paint("✖", "1;31", true),
                elapsed(duration)
            );
            return Err(error);
        }

        self.stages.push((label.to_owned(), duration));
        self.bar.inc(1);

        if self.verbose {
            println!(
                "    {} Completed in {}",
                paint("✔", "1;32", false),
                elapsed(duration)
            );
        }

        Ok(())
    }

    pub fn finish(&self, apk: &Path) -> Result<()> {
        self.bar.finish_and_clear();

        let total_duration = self.started.elapsed();
        let size = std::fs::metadata(apk)
            .with_context(|| format!("Unable to read APK metadata: {}", apk.display()))?
            .len();

        let size_mb = size as f64 / (1024.0 * 1024.0);

        println!();
        println!(
            "  {} {} {}/{}",
            paint("◆", "1;38;5;45", false),
            paint("BUILD COMPLETE", "1;32", false),
            self.stages.len(),
            self.total
        );

        section("STAGE SUMMARY");

        for (label, duration) in &self.stages {
            println!(
                "  {} {:<36} {}",
                paint("✔", "1;32", false),
                label,
                paint(&elapsed(*duration), "38;5;110", false)
            );
        }

        section("BUILD ARTIFACT");
        key_value("Status", &paint("SUCCESS", "1;32", false));
        key_value("Duration", &elapsed(total_duration));
        key_value("APK", &apk.display().to_string());
        key_value("Size", &format!("{size_mb:.2} MB"));
        println!();

        Ok(())
    }
}

pub fn run_step_capture(
    command: &mut Command,
    label: &str,
    verbose: bool,
) -> Result<std::process::Output> {
    let started = Instant::now();
    let spinner = ProgressBar::new_spinner();

    spinner.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")?.tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );

    spinner.set_message(label.to_owned());

    if !verbose {
        spinner.enable_steady_tick(Duration::from_millis(85));
    }

    let output = match command.output() {
        Ok(output) => output,
        Err(error) => {
            spinner.finish_and_clear();
            return Err(error).with_context(|| format!("Unable to start {label}"));
        }
    };

    spinner.finish_and_clear();
    let duration = elapsed(started.elapsed());

    if verbose {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stdout.is_empty() {
            print!("{stdout}");
        }

        if !stderr.is_empty() {
            eprint!("{stderr}");
        }
    }

    if output.status.success() {
        success(&format!("{label} ({duration})"));
    } else {
        eprintln!("  {} {label} ({duration})", paint("✖", "1;31", true));

        if !verbose {
            print_tail("Error output", &output.stderr);
            print_tail("Command output", &output.stdout);
        }
    }

    Ok(output)
}

pub fn run_step(command: &mut Command, label: &str, verbose: bool) -> Result<()> {
    let started = Instant::now();

    if verbose {
        println!("  {}", paint(label, "1;38;5;45", false));

        let status = command
            .status()
            .with_context(|| format!("Unable to start {label}"))?;

        if !status.success() {
            bail!("{label} failed ({status})");
        }

        success(&format!(
            "{label} completed in {}",
            elapsed(started.elapsed())
        ));

        return Ok(());
    }

    let spinner = ProgressBar::new_spinner();

    spinner.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")?.tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );

    spinner.set_message(label.to_owned());
    spinner.enable_steady_tick(Duration::from_millis(85));

    let output = match command.output() {
        Ok(output) => output,
        Err(error) => {
            spinner.finish_and_clear();
            eprintln!("  {} {label}", paint("✖", "1;31", true));
            return Err(error).with_context(|| format!("Unable to start {label}"));
        }
    };

    let duration = elapsed(started.elapsed());

    if output.status.success() {
        spinner.finish_and_clear();
        success(&format!("{label} ({duration})"));
        return Ok(());
    }

    spinner.finish_and_clear();
    eprintln!("  {} {label} ({duration})", paint("✖", "1;31", true));

    print_tail("Error output", &output.stderr);
    print_tail("Command output", &output.stdout);

    bail!(
        "{label} failed ({}). Run again with `--verbose` for full output.",
        output.status
    )
}
