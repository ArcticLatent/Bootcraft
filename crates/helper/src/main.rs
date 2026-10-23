//! `bootcraft-helper`: the privileged side of Bootcraft, also usable directly
//! from a terminal. This phase only detects and plans; nothing is changed.

use std::path::PathBuf;
use std::process::ExitCode;

use bootcraft_core::catalog::catalog;
use bootcraft_core::detect::{Env, detect};
use bootcraft_core::plan::recommended;
use bootcraft_core::probe::{Probe, Status};
use bootcraft_core::system::{Desktop, NvidiaGen, SecureBoot, System};
use bootcraft_core::{Preset, plan};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(version, about = "Bootcraft post-install helper")]
struct Cli {
    /// Detect a different root filesystem (for testing against fixtures).
    #[arg(long, global = true, hide = true, default_value = "/")]
    root: PathBuf,
    /// Override the detected desktop (gnome, kde, cinnamon or any other name).
    #[arg(long, global = true)]
    desktop: Option<String>,
    /// Override the NVIDIA driver choice.
    #[arg(long, global = true)]
    nvidia: Option<NvidiaArg>,
    /// Print JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show what Bootcraft detects about this machine.
    Detect,
    /// List the tasks available on this machine and whether they're already done.
    Tasks,
    /// Print what would run, without changing anything.
    Plan {
        /// Task IDs to run (default: the recommended set).
        #[arg(long, value_delimiter = ',')]
        select: Vec<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum NvidiaArg {
    /// Current driver with open kernel modules (Turing and newer).
    Modern,
    /// 580 legacy driver (Maxwell, Pascal, Volta).
    Legacy,
    /// Skip NVIDIA driver tasks.
    None,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut sys = detect(&cli.root, &Env::from_process());
    if let Some(desktop) = &cli.desktop {
        sys.desktop = Desktop::parse(desktop);
    }
    match cli.nvidia {
        Some(NvidiaArg::Modern) => sys.nvidia = Some(NvidiaGen::Modern),
        Some(NvidiaArg::Legacy) => sys.nvidia = Some(NvidiaGen::Legacy580),
        Some(NvidiaArg::None) => sys.nvidia = None,
        None => {}
    }

    match cli.command {
        Command::Detect => {
            if cli.json {
                print_json(&sys);
            } else {
                print_system(&sys);
            }
        }
        Command::Tasks => print_tasks(&cli, &sys),
        Command::Plan { select } => {
            let tasks = catalog(&sys);
            let selection: Vec<String> = if select.is_empty() {
                recommended(&tasks).into_iter().map(str::to_owned).collect()
            } else {
                select
            };
            match plan(&sys, &tasks, &selection) {
                Ok(plan) if cli.json => print_json(&plan),
                Ok(plan) => print!("{}", plan.render()),
                Err(err) => {
                    eprintln!("error: {err}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    ExitCode::SUCCESS
}

fn print_json<T: serde::Serialize>(value: &T) {
    println!("{}", serde_json::to_string_pretty(value).expect("plain data serializes"));
}

fn print_system(sys: &System) {
    let support = match (sys.distro(), sys.os.version_supported()) {
        (None, _) => "unsupported",
        (Some(_), true) => "supported",
        (Some(_), false) => "untested release",
    };
    println!("System       {} [{support}]", sys.os.pretty_name);
    println!("Kernel       {}", sys.kernel_release);
    if !sys.arch_kernels.is_empty() {
        println!("Kernels      {}", sys.arch_kernels.join(", "));
    }
    println!("Desktop      {} ({:?})", sys.desktop.label(), sys.session);
    for gpu in &sys.gpus {
        let name = gpu.name.as_deref().unwrap_or("unknown device");
        let driver = gpu.nvidia_gen().map(|g| format!(" -> {}", g.label())).unwrap_or_default();
        let boot = if gpu.boot_vga { ", primary" } else { "" };
        println!(
            "GPU          {name} [{:04x}:{:04x} at {}{boot}]{driver}",
            gpu.vendor_id, gpu.device_id, gpu.slot
        );
    }
    if sys.gpus.is_empty() {
        println!("GPU          none found");
    }
    let nvidia = sys.nvidia.map_or("none", NvidiaGen::label);
    println!("NVIDIA plan  {nvidia}");
    let secure_boot = match sys.secure_boot {
        SecureBoot::Enabled => "enabled",
        SecureBoot::Disabled => "disabled",
        SecureBoot::NotEfi => "not an EFI system",
        SecureBoot::Unknown => "unknown",
    };
    println!("Secure Boot  {secure_boot}");
    for warning in sys.warnings() {
        println!("Warning      {warning}");
    }
}

fn print_tasks(cli: &Cli, sys: &System) {
    let tasks = catalog(sys);
    let probe = sys.distro().map(|d| Probe::new(&cli.root, d));
    let status = |task| probe.as_ref().map_or(Status::Unknown, |p| p.status(task));
    if cli.json {
        let rows: Vec<_> =
            tasks.iter().map(|t| serde_json::json!({ "task": t, "status": status(t) })).collect();
        print_json(&rows);
        return;
    }
    if tasks.is_empty() {
        println!("No tasks for {} yet.", sys.os.pretty_name);
        return;
    }
    // Group by section for display; execution keeps catalog order.
    let mut by_category: Vec<_> = tasks.iter().collect();
    by_category.sort_by_key(|t| t.category);
    let mut category = None;
    for task in by_category {
        if category != Some(task.category) {
            category = Some(task.category);
            println!("\n{}", task.category);
        }
        let tick = if task.preset == Preset::Recommended { "[x]" } else { "[ ]" };
        let state = match status(task) {
            Status::Done => "done",
            Status::Pending => "",
            Status::Unknown => "?",
        };
        println!("  {tick} {:<24} {:<42} {state}", task.id, task.title);
        for note in &task.notes {
            println!("        note: {note}");
        }
    }
}
