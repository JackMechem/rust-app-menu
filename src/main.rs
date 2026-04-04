use iced::futures::SinkExt;
use iced::keyboard::key::Named;
use iced::widget::{Id, button, column, container, scrollable, text, text_input};
use iced::window::Id as WindowId;
use iced::{Alignment, Color, Element, Event, Length, Task as Command, event, keyboard};
use iced_layershell::build_pattern::daemon;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use iced_layershell::to_layer_message;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, mpsc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const INPUT_ID: &str = "search";
const SOCKET_NAME: &str = "rust-app-menu.sock";

static SHOW_RX: OnceLock<Mutex<mpsc::Receiver<()>>> = OnceLock::new();

fn socket_path() -> String {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    format!("{}/{}", runtime_dir, SOCKET_NAME)
}

async fn try_show_existing() -> bool {
    match tokio::net::UnixStream::connect(socket_path()).await {
        Ok(mut conn) => {
            eprintln!("[client] Found existing instance, sending show signal");
            let _ = conn.write_all(b"show").await;
            true
        }
        Err(e) => {
            eprintln!("[client] No existing instance ({}), starting daemon", e);
            false
        }
    }
}

async fn listen_for_show(sender: mpsc::Sender<()>) {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);

    eprintln!("[daemon] Binding socket at {}", path);
    let listener = match tokio::net::UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[daemon] Failed to bind socket: {}", e);
            return;
        }
    };

    eprintln!("[daemon] Listening for show signals...");
    loop {
        if let Ok((mut conn, _)) = listener.accept().await {
            eprintln!("[daemon] Got connection");
            let mut buf = [0u8; 8];
            if conn.read(&mut buf).await.is_ok() {
                eprintln!("[daemon] Received show signal, notifying iced");
                let _ = sender.send(());
            }
        }
    }
}

fn parse_hyprctl_focused_monitor(json: &str) -> Option<String> {
    let focused_block = json
        .split('{')
        .find(|block| block.contains("\"focused\": true") || block.contains("\"focused\":true"))?;
    let name_start = focused_block.find("\"name\":")?;
    let after = focused_block[name_start + 7..].trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

fn parse_sway_focused_output(json: &str) -> Option<String> {
    let focused_block = json
        .split('{')
        .find(|block| block.contains("\"focused\": true") || block.contains("\"focused\":true"))?;
    let name_start = focused_block.find("\"name\":")?;
    let after = focused_block[name_start + 7..].trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

// --- Styles ---

fn container_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Color::from_rgb(0.15, 0.15, 0.18))),
        border: iced::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.1),
            width: 1.0,
            radius: iced::border::Radius {
                top_left: 0.0,
                top_right: 0.0,
                bottom_left: 18.0,
                bottom_right: 18.0,
            },
        },
        ..Default::default()
    }
}

fn input_style(_theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: iced::Background::Color(Color::from_rgb(0.2, 0.2, 0.25)),
        border: iced::Border {
            color: match status {
                text_input::Status::Focused { .. } => Color::from_rgb(0.4, 0.6, 1.0),
                text_input::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 0.3),
                _ => Color::from_rgba(1.0, 1.0, 1.0, 0.1),
            },
            width: 1.0,
            radius: 20.0.into(),
        },
        value: Color::WHITE,
        placeholder: Color::from_rgba(1.0, 1.0, 1.0, 0.4),
        selection: Color::from_rgb(0.4, 0.6, 1.0),
        icon: Color::WHITE,
    }
}

fn button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(iced::Background::Color(match status {
            button::Status::Hovered => Color::from_rgb(0.25, 0.25, 0.3),
            button::Status::Pressed => Color::from_rgb(0.3, 0.3, 0.38),
            _ => Color::from_rgb(0.2, 0.2, 0.25),
        })),
        border: iced::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 50.0.into(),
        },
        text_color: Color::from_rgb(1.0, 1.0, 1.0),
        ..Default::default()
    }
}

fn window_style(_: &Launcher, _theme: &iced::Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    }
}

// --- Data ---

#[derive(Clone)]
struct App {
    name: String,
    exec: String,
}

fn load_apps() -> Vec<App> {
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/share:/run/current-system/sw/share".to_string());

    let mut apps = Vec::new();

    for dir in data_dirs.split(':') {
        let path = PathBuf::from(dir).join("applications");
        let entries = match std::fs::read_dir(&path) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if let Some(app) = parse_desktop_file(&path) {
                apps.push(app);
            }
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

fn parse_desktop_file(path: &PathBuf) -> Option<App> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut exec = None;
    let mut no_display = false;
    let mut in_desktop_entry = false;

    for line in content.lines() {
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }
        if line.starts_with('[') {
            in_desktop_entry = false;
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        if let Some(val) = line.strip_prefix("Name=") {
            name.get_or_insert_with(|| val.to_string());
        } else if let Some(val) = line.strip_prefix("Exec=") {
            exec.get_or_insert_with(|| clean_exec(val));
        } else if line == "NoDisplay=true" || line == "Type=Directory" {
            no_display = true;
        }
    }

    if no_display {
        return None;
    }

    Some(App {
        name: name?,
        exec: exec?,
    })
}

fn clean_exec(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|s| !s.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ")
}

fn launch(exec: &str) {
    let _ = std::process::Command::new("sh").arg("-c").arg(exec).spawn();
}

// --- App state ---

struct Launcher {
    query: String,
    all_apps: Vec<App>,
    visible: bool,
    known_ids: RefCell<Vec<WindowId>>,
}

// --- Message ---

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
enum Message {
    QueryChanged(String),
    IcedEvent(Event),
    Launch(String),
    Close,
    Show,
}

// --- Standalone functions ---

fn namespace() -> String {
    String::from("Launcher")
}

fn new() -> (Launcher, Command<Message>) {
    let (tx, rx) = mpsc::channel();
    SHOW_RX.set(Mutex::new(rx)).ok();

    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(listen_for_show(tx));
    });

    let state = Launcher {
        query: String::new(),
        all_apps: load_apps(),
        visible: false,
        known_ids: RefCell::new(Vec::new()),
    };
    (state, Command::none())
}

fn all_ids_hide(launcher: &Launcher) -> Command<Message> {
    let cmds: Vec<Command<Message>> = launcher
        .known_ids
        .borrow()
        .iter()
        .flat_map(|&id| {
            [
                Command::done(Message::AnchorSizeChange {
                    id,
                    anchor: Anchor::empty(),
                    size: (1, 1),
                }),
                Command::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::None,
                }),
            ]
        })
        .collect();
    Command::batch(cmds)
}

fn all_ids_show(launcher: &Launcher) -> Command<Message> {
    let cmds: Vec<Command<Message>> = launcher
        .known_ids
        .borrow()
        .iter()
        .flat_map(|&id| {
            [
                Command::done(Message::AnchorSizeChange {
                    id,
                    anchor: Anchor::Top,
                    size: (600, 400),
                }),
                Command::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::Exclusive,
                }),
            ]
        })
        .collect();
    Command::batch(cmds)
}

fn update(launcher: &mut Launcher, message: Message) -> Command<Message> {
    match message {
        Message::Show => {
            eprintln!("[iced] Show signal received");
            launcher.query.clear();
            launcher.visible = true;
            all_ids_show(launcher)
        }

        Message::IcedEvent(Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Escape),
            ..
        })) => {
            launcher.query.clear();
            launcher.visible = false;
            all_ids_hide(launcher)
        }

        Message::IcedEvent(Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Enter),
            ..
        })) => {
            if let Some(app) = launcher.filtered().first() {
                launch(&app.exec.clone());
                launcher.query.clear();
                launcher.visible = false;
                return all_ids_hide(launcher);
            }
            Command::none()
        }

        Message::IcedEvent(Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Character(c),
            ..
        })) => {
            launcher.query.push_str(c.as_str());
            Command::none()
        }

        Message::IcedEvent(Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Backspace),
            ..
        })) => {
            launcher.query.pop();
            Command::none()
        }

        Message::IcedEvent(_) => Command::none(),

        Message::QueryChanged(q) => {
            launcher.query = q;
            Command::none()
        }

        Message::Launch(exec) => {
            launch(&exec);
            launcher.query.clear();
            launcher.visible = false;
            all_ids_hide(launcher)
        }

        Message::Close => {
            launcher.query.clear();
            launcher.visible = false;
            all_ids_hide(launcher)
        }

        _ => unreachable!(),
    }
}

fn view(launcher: &Launcher, id: WindowId) -> Element<Message> {
    // Register this window id the first time view is called for it
    {
        let mut ids = launcher.known_ids.borrow_mut();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }

    if !launcher.visible {
        return container(column![])
            .width(Length::Fixed(1.0))
            .height(Length::Fixed(1.0))
            .into();
    }

    let results = launcher.filtered();

    let list = scrollable(results.iter().fold(column![].spacing(10), |col, app| {
        col.push(
            button(text(&app.name))
                .on_press(Message::Launch(app.exec.clone()))
                .style(button_style)
                .width(Length::Fill)
                .padding([10, 20]),
        )
    }))
    .height(Length::Fill);

    let content = column![
        text_input("Search...", &launcher.query)
            .id(Id::new(INPUT_ID))
            .on_input(Message::QueryChanged)
            .style(input_style)
            .padding([10, 20]),
        list,
    ]
    .align_x(Alignment::Center)
    .padding(20)
    .spacing(10)
    .width(Length::Fill)
    .height(Length::Fill);

    container(content)
        .style(container_style)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn subscription(_launcher: &Launcher) -> iced::Subscription<Message> {
    let event_sub = event::listen().map(Message::IcedEvent);

    let show_sub = iced::Subscription::run(|| {
        iced::stream::channel(
            1,
            |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                loop {
                    let (tx, rx) = iced::futures::channel::oneshot::channel();
                    std::thread::spawn(move || {
                        let got = SHOW_RX
                            .get()
                            .map(|rx| rx.lock().unwrap().recv().is_ok())
                            .unwrap_or(false);
                        let _ = tx.send(got);
                    });
                    if rx.await.unwrap_or(false) {
                        eprintln!("[sub] Got show signal, sending Message::Show");
                        let _ = output.send(Message::Show).await;
                    }
                }
            },
        )
    });

    iced::Subscription::batch([event_sub, show_sub])
}

impl Launcher {
    fn filtered(&self) -> Vec<&App> {
        if self.query.is_empty() {
            self.all_apps.iter().collect()
        } else {
            let q = self.query.to_lowercase();
            self.all_apps
                .iter()
                .filter(|a| a.name.to_lowercase().contains(&q))
                .collect()
        }
    }
}

fn main() -> Result<(), iced_layershell::Error> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    if rt.block_on(try_show_existing()) {
        return Ok(());
    }

    daemon(new, namespace, update, view)
        .style(window_style)
        .subscription(subscription)
        .settings(Settings {
            layer_settings: LayerShellSettings {
                size: Some((1, 1)),
                anchor: Anchor::empty(),
                exclusive_zone: 0,
                layer: Layer::Overlay,
                keyboard_interactivity: KeyboardInteractivity::None,
                start_mode: StartMode::AllScreens,
                ..Default::default()
            },
            ..Default::default()
        })
        .run()
}
