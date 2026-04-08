mod app;
mod config;
mod daemon;
mod data;
mod monitor;
mod styles;

use app::{RunMode, namespace, new, subscription, update, view};
use iced_layershell::build_pattern::daemon;
use iced_layershell::reexport::{KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use styles::window_style;

// ── arg parsing ───────────────────────────────────────────────────────────────

struct Args {
    help: bool,
    daemon_mode: bool,
    show: bool,
    config: Option<String>,
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut args = Args { help: false, daemon_mode: false, show: false, config: None };
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--help" | "-h" => args.help = true,
            "--daemon" | "-d" => args.daemon_mode = true,
            "--show" | "-s" => args.show = true,
            "--config" | "-c" => {
                i += 1;
                match argv.get(i) {
                    Some(p) => args.config = Some(p.clone()),
                    None => {
                        eprintln!("Error: --config requires a path argument");
                        std::process::exit(1);
                    }
                }
            }
            other => {
                eprintln!("Unknown argument: {other}");
                eprintln!("Run with --help for usage.");
                std::process::exit(1);
            }
        }
        i += 1;
    }
    args
}

fn print_help() {
    println!(
        "Usage: rust-app-menu [OPTIONS]

Options:
  -h, --help           Show this help message and exit
  -c, --config PATH    Use a custom config file
                       (default: $XDG_CONFIG_HOME/rust-app-menu/config.toml)
  -d, --daemon         Run as a background daemon, listening for --show signals
  -s, --show           Send a show signal to a running daemon

When launched with no arguments the menu opens as a one-shot window using the
default config location. Press Escape or launch an app to close it."
    );
}

// ── entry point ───────────────────────────────────────────────────────────────

fn main() -> Result<(), iced_layershell::Error> {
    let args = parse_args();

    if args.help {
        print_help();
        return Ok(());
    }

    if let Some(path) = args.config {
        crate::config::set_path(path);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // --show: signal the running daemon and exit
    if args.show {
        if !rt.block_on(crate::daemon::try_show_existing()) {
            eprintln!("Error: no daemon is running. Start one with --daemon.");
            std::process::exit(1);
        }
        return Ok(());
    }

    // --daemon: start the background daemon (error if one is already running)
    if args.daemon_mode {
        if rt.block_on(crate::daemon::is_running()) {
            eprintln!("Error: a daemon is already running.");
            std::process::exit(1);
        }
        app::RUN_MODE.set(RunMode::Daemon).ok();
    } else {
        // no flags: one-shot normal window
        app::RUN_MODE.set(RunMode::Normal).ok();
    }

    let cfg = crate::config::get();

    let (initial_size, initial_keyboard) = match app::run_mode() {
        RunMode::Normal => ((cfg.window.width, 80), KeyboardInteractivity::Exclusive),
        RunMode::Daemon => ((1, 1), KeyboardInteractivity::None),
    };

    daemon(new, namespace, update, view)
        .style(window_style)
        .subscription(subscription)
        .settings(Settings {
            layer_settings: LayerShellSettings {
                size: Some(initial_size),
                anchor: cfg.window.anchor.to_anchor(),
                margin: cfg.window.anchor.to_margin(cfg.window.margin),
                exclusive_zone: 0,
                layer: Layer::Overlay,
                keyboard_interactivity: initial_keyboard,
                start_mode: StartMode::AllScreens,
                ..Default::default()
            },
            ..Default::default()
        })
        .run()
}
