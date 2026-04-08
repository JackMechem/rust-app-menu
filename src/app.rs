use std::cell::RefCell;
use std::sync::{OnceLock, mpsc};

use iced::futures::SinkExt;
use iced::keyboard::key::Named;
use iced::widget::{Id, button, column, container, scrollable, text, text_input};
use iced::window::Id as WindowId;
use iced::{Alignment, Element, Event, Length, Task as Command, event, keyboard};
use iced_layershell::reexport::KeyboardInteractivity;
use iced_layershell::to_layer_message;

use crate::daemon::{DAEMON_RX, DaemonCommand, listen_for_commands};
use crate::data::{App, launch, load_apps};
use crate::styles::{button_style, button_style_selected, container_style, input_style};

pub const INPUT_ID: &str = "search";

// ── run mode ─────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub enum RunMode {
    /// One-shot window: show immediately, exit on close.
    Normal,
    /// Background daemon: wait for --show signals over a socket.
    Daemon,
}

pub static RUN_MODE: OnceLock<RunMode> = OnceLock::new();

pub fn run_mode() -> &'static RunMode {
    RUN_MODE.get().unwrap_or(&RunMode::Normal)
}

// ── state ─────────────────────────────────────────────────────────────────────

pub struct Launcher {
    pub query: String,
    pub all_apps: Vec<App>,
    pub visible: bool,
    pub selected: usize,
    pub known_ids: RefCell<Vec<WindowId>>,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    QueryChanged(String),
    IcedEvent(Event),
    Launch(String),
    Close,
    Show,
    Reload,
}

pub fn namespace() -> String {
    String::from("Launcher")
}

pub fn new() -> (Launcher, Command<Message>) {
    let cmd = match run_mode() {
        RunMode::Daemon => {
            let (tx, rx) = mpsc::channel();
            DAEMON_RX.set(std::sync::Mutex::new(rx)).ok();
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(listen_for_commands(tx));
            });
            Command::none()
        }
        RunMode::Normal => Command::done(Message::Show),
    };

    let state = Launcher {
        query: String::new(),
        all_apps: load_apps(),
        visible: false,
        selected: 0,
        known_ids: RefCell::new(Vec::new()),
    };
    (state, cmd)
}

// ── layer shell helpers ───────────────────────────────────────────────────────

fn all_ids_hide(launcher: &Launcher) -> Command<Message> {
    let anchor = crate::config::get().window.anchor.to_anchor();
    let cmds: Vec<Command<Message>> = launcher
        .known_ids
        .borrow()
        .iter()
        .flat_map(|&id| {
            [
                Command::done(Message::AnchorSizeChange {
                    id,
                    anchor,
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

fn all_ids_show(launcher: &Launcher, height: u32) -> Command<Message> {
    let cfg = crate::config::get();
    let anchor = cfg.window.anchor.to_anchor();
    let margin = cfg.window.anchor.to_margin(cfg.window.margin);
    let width = cfg.window.width;
    let cmds: Vec<Command<Message>> = launcher
        .known_ids
        .borrow()
        .iter()
        .flat_map(|&id| {
            [
                Command::done(Message::AnchorSizeChange {
                    id,
                    anchor,
                    size: (width, height),
                }),
                Command::done(Message::MarginChange { id, margin }),
                Command::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::Exclusive,
                }),
            ]
        })
        .collect();
    Command::batch(cmds)
}

fn all_ids_resize(launcher: &Launcher, height: u32) -> Command<Message> {
    let cfg = crate::config::get();
    let anchor = cfg.window.anchor.to_anchor();
    let width = cfg.window.width;
    let cmds: Vec<Command<Message>> = launcher
        .known_ids
        .borrow()
        .iter()
        .map(|&id| {
            Command::done(Message::AnchorSizeChange {
                id,
                anchor,
                size: (width, height),
            })
        })
        .collect();
    Command::batch(cmds)
}

fn compute_height(launcher: &Launcher) -> u32 {
    if launcher.query.is_empty() {
        return 80;
    }
    let count = launcher.filtered().len() as u32;
    if count < 1 {
        return 80;
    }
    // 80px for search bar + padding; 55px per item
    (80 + count * 55).min(400)
}

/// Close the launcher. In Normal mode this exits the process; in Daemon mode
/// it hides the window back to a 1×1 stub.
fn do_close(launcher: &mut Launcher) -> Command<Message> {
    launcher.query.clear();
    launcher.selected = 0;
    launcher.visible = false;
    match run_mode() {
        RunMode::Normal => std::process::exit(0),
        RunMode::Daemon => all_ids_hide(launcher),
    }
}

// ── update ────────────────────────────────────────────────────────────────────

pub fn update(launcher: &mut Launcher, message: Message) -> Command<Message> {
    match message {
        Message::Show => {
            launcher.query.clear();
            launcher.selected = 0;
            launcher.visible = true;
            let height = compute_height(launcher);
            all_ids_show(launcher, height)
        }

        Message::IcedEvent(Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Escape),
            ..
        })) => do_close(launcher),

        Message::IcedEvent(Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Tab),
            ..
        })) => {
            let count = launcher.filtered().len();
            if count > 0 {
                launcher.selected = (launcher.selected + 1) % count;
            }
            Command::none()
        }

        Message::IcedEvent(Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Enter),
            ..
        })) => {
            let results = launcher.filtered();
            let idx = launcher.selected.min(results.len().saturating_sub(1));
            if let Some(app) = results.get(idx) {
                launch(&app.exec.clone());
                return do_close(launcher);
            }
            Command::none()
        }

        Message::IcedEvent(Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Character(c),
            ..
        })) => {
            launcher.query.push_str(c.as_str());
            launcher.selected = 0;
            let height = compute_height(launcher);
            all_ids_resize(launcher, height)
        }

        Message::IcedEvent(Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Backspace),
            ..
        })) => {
            launcher.query.pop();
            launcher.selected = 0;
            let height = compute_height(launcher);
            all_ids_resize(launcher, height)
        }

        Message::IcedEvent(_) => Command::none(),

        Message::QueryChanged(q) => {
            launcher.query = q;
            launcher.selected = 0;
            let height = compute_height(launcher);
            all_ids_resize(launcher, height)
        }

        Message::Launch(exec) => {
            launch(&exec);
            do_close(launcher)
        }

        Message::Close => do_close(launcher),

        Message::Reload => {
            crate::config::reload();
            launcher.all_apps = load_apps();
            Command::none()
        }

        _ => unreachable!(),
    }
}

// ── view ──────────────────────────────────────────────────────────────────────

pub fn view(launcher: &Launcher, id: WindowId) -> Element<'_, Message> {
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

    let search = text_input("Search...", &launcher.query)
        .id(Id::new(INPUT_ID))
        .on_input(Message::QueryChanged)
        .style(input_style)
        .padding([10, 20]);

    let mut col = column![search]
        .align_x(Alignment::Center)
        .padding(20)
        .spacing(10)
        .width(Length::Fill);

    if !launcher.query.is_empty() {
        let results = launcher.filtered();
        if !results.is_empty() {
            let selected = launcher.selected.min(results.len() - 1);
            let list = scrollable(
                results
                    .iter()
                    .enumerate()
                    .fold(column![].spacing(10), |col, (i, app)| {
                        col.push(
                            button(text(&app.name))
                                .on_press(Message::Launch(app.exec.clone()))
                                .style(if i == selected {
                                    button_style_selected
                                } else {
                                    button_style
                                })
                                .width(Length::Fill)
                                .padding([10, 20]),
                        )
                    }),
            )
            .height(Length::Fill);
            col = col.height(Length::Fill).push(list);
        }
    }

    container(col)
        .style(container_style)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ── subscription ──────────────────────────────────────────────────────────────

pub fn subscription(_launcher: &Launcher) -> iced::Subscription<Message> {
    let event_sub = event::listen().map(Message::IcedEvent);

    if matches!(run_mode(), RunMode::Normal) {
        return event_sub;
    }

    let show_sub = iced::Subscription::run(|| {
        iced::stream::channel(
            1,
            |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                loop {
                    let (tx, rx) = iced::futures::channel::oneshot::channel();
                    std::thread::spawn(move || {
                        let cmd = DAEMON_RX
                            .get()
                            .and_then(|rx| rx.lock().unwrap().recv().ok());
                        let _ = tx.send(cmd);
                    });
                    if let Ok(Some(cmd)) = rx.await {
                        let msg = match cmd {
                            DaemonCommand::Show => Message::Show,
                            DaemonCommand::Reload => Message::Reload,
                        };
                        let _ = output.send(msg).await;
                    }
                }
            },
        )
    });

    iced::Subscription::batch([event_sub, show_sub])
}

// ── helpers ───────────────────────────────────────────────────────────────────

impl Launcher {
    pub fn filtered(&self) -> Vec<&App> {
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
