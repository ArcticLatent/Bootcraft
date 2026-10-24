//! `bootcraft-helper`: the privileged side of Bootcraft, also usable directly
//! from a terminal. `detect`, `tasks` and `plan` change nothing; `apply` and
//! `serve` run as root.

mod run;

use std::path::PathBuf;
use std::process::ExitCode;

use bootcraft_core::apps::{Preference, Source};
use bootcraft_core::catalog::{Catalog, PACKS, catalog};
use bootcraft_core::detect::{Env, detect};
use bootcraft_core::probe::{Probe, Status};
use bootcraft_core::protocol::NvidiaChoice;
use bootcraft_core::system::{Desktop, NvidiaGen, SecureBoot, System};
use bootcraft_core::{AppPick, PlanError, Preset, Selection, Task, plan};
use clap::{Args, Parser, Subcommand, ValueEnum};

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
    /// How to install apps: auto picks per app and distribution.
    #[arg(long, global = true, default_value = "auto")]
    prefer: PreferArg,
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
    /// List the system tasks available on this machine and whether they're already done.
    Tasks,
    /// List the apps, where each would come from, and the workflows that tick them.
    Apps {
        /// Only apps in these workflows (creative, development, gaming).
        #[arg(long, value_delimiter = ',')]
        packs: Vec<String>,
    },
    /// Print what would run, without changing anything.
    Plan(Choose),
    /// Run the plan. Needs root: `sudo bootcraft-helper apply`.
    Apply {
        #[command(flatten)]
        choose: Choose,
        /// Don't ask for confirmation.
        #[arg(long, short)]
        yes: bool,
    },
    /// Speak the JSON-lines protocol on stdin/stdout (used by the app via pkexec).
    #[command(hide = true)]
    Serve,
}

/// What to run. Without options: the Essentials.
#[derive(Args, Clone)]
struct Choose {
    /// Workflows to add on top of the Essentials: creative, development, gaming.
    #[arg(long, value_delimiter = ',')]
    packs: Vec<String>,
    /// Extra apps, optionally with a source: `gimp`, `gimp:flatpak`, `steam:native`.
    #[arg(long, value_delimiter = ',')]
    apps: Vec<String>,
    /// System task IDs to run instead of the Essentials.
    #[arg(long, value_delimiter = ',')]
    select: Vec<String>,
}

#[derive(Clone, Copy, ValueEnum)]
enum PreferArg {
    Auto,
    Flatpak,
    Native,
}

impl From<PreferArg> for Preference {
    fn from(arg: PreferArg) -> Self {
        match arg {
            PreferArg::Auto => Self::Auto,
            PreferArg::Flatpak => Self::Flatpak,
            PreferArg::Native => Self::Native,
        }
    }
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
    if matches!(cli.command, Command::Serve) {
        // serve detects for itself, with the desktop the app sends.
        return run::serve(&cli.root);
    }
    let mut sys = detect(&cli.root, &Env::from_process());
    if let Some(desktop) = &cli.desktop {
        sys.desktop = Desktop::parse(desktop);
    }
    if let Some(choice) = cli.nvidia {
        match choice {
            NvidiaArg::Modern => NvidiaChoice::Modern,
            NvidiaArg::Legacy => NvidiaChoice::Legacy,
            NvidiaArg::None => NvidiaChoice::None,
        }
        .apply(&mut sys);
    }

    match &cli.command {
        Command::Detect => {
            if cli.json {
                print_json(&sys);
            } else {
                print_system(&sys);
            }
        }
        Command::Tasks => print_tasks(&cli, &sys),
        Command::Apps { packs } => print_apps(&cli, &sys, packs),
        Command::Plan(choose) => {
            let catalog = catalog(&sys);
            let result = selection(&catalog, choose.clone(), cli.prefer.into())
                .and_then(|selection| plan(&sys, &catalog, &selection));
            match result {
                Ok(plan) if cli.json => print_json(&plan),
                Ok(plan) => print!("{}", plan.render()),
                Err(err) => {
                    eprintln!("error: {err}");
                    return ExitCode::FAILURE;
                }
            }
        }
        Command::Apply { choose, yes } => {
            let catalog = catalog(&sys);
            return match selection(&catalog, choose.clone(), cli.prefer.into()) {
                Ok(selection) => run::apply(&cli.root, &sys, &selection, *yes),
                Err(err) => {
                    eprintln!("error: {err}");
                    ExitCode::FAILURE
                }
            };
        }
        Command::Serve => unreachable!("handled above"),
    }
    ExitCode::SUCCESS
}

/// Essentials plus the packs' ticked apps, unless `--select` replaces the
/// system tasks; then the extra `--apps`.
fn selection(catalog: &Catalog, choose: Choose, preference: Preference) -> Result<Selection, PlanError> {
    let mut selection = Selection::from_packs(catalog, &choose.packs)?;
    if !choose.select.is_empty() {
        selection.tasks = choose.select;
    }
    for arg in choose.apps {
        let (id, source) = match arg.split_once(':') {
            Some((id, "flatpak")) => (id.to_owned(), Some(Source::Flatpak)),
            Some((id, "native")) => (id.to_owned(), Some(Source::Native)),
            Some(_) => return Err(PlanError::UnknownApp(arg)),
            None => (arg, None),
        };
        selection.apps.retain(|p| p.id != id);
        selection.apps.push(AppPick { id, source });
    }
    selection.preference = preference;
    Ok(selection)
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

fn print_apps(cli: &Cli, sys: &System, packs: &[String]) {
    let catalog = catalog(sys);
    let preference = cli.prefer.into();
    let in_packs = |id: &str| -> Vec<String> {
        PACKS
            .iter()
            .filter_map(|p| {
                let &(_, ticked) = p.apps.iter().find(|(app, _)| *app == id)?;
                Some(if ticked { format!("{}*", p.id) } else { p.id.to_owned() })
            })
            .collect()
    };
    let mut apps: Vec<_> = catalog
        .apps
        .iter()
        .filter(|a| {
            packs.is_empty()
                || packs.iter().any(|p| in_packs(a.id).iter().any(|m| m.trim_end_matches('*') == p))
        })
        .collect();
    apps.sort_by_key(|a| a.section);
    if cli.json {
        let rows: Vec<_> = apps
            .iter()
            .map(|a| serde_json::json!({ "app": a, "choice": a.choose(sys, preference, None), "packs": in_packs(a.id) }))
            .collect();
        print_json(&rows);
        return;
    }
    let mut section = None;
    for app in apps {
        if section != Some(app.section) {
            section = Some(app.section);
            println!("\n{}", app.section.title());
        }
        let source = match app.choose(sys, preference, None) {
            Some(choice) => {
                let label = match choice.source {
                    Source::Flatpak if app.flatpak.as_ref().is_some_and(|f| f.verified) => {
                        "Flatpak (verified)"
                    }
                    Source::Flatpak => "Flatpak",
                    Source::Native => "system",
                };
                format!("{label} - {}", choice.reason)
            }
            None => "not available here".to_owned(),
        };
        println!("  {:<18} {:<20} {:<62} {}", app.id, app.name, source, in_packs(app.id).join(" "));
    }
    println!("\n* = ticked by default in that workflow");
}

fn print_tasks(cli: &Cli, sys: &System) {
    let tasks = catalog(sys).tasks;
    let probe = sys.distro().map(|d| Probe::new(&cli.root, d));
    let status = |task: &Task| probe.as_ref().map_or(Status::Unknown, |p| p.status(&task.steps));
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
