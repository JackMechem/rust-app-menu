use std::path::PathBuf;

#[derive(Clone)]
pub struct App {
    pub name: String,
    pub exec: String,
}

pub fn load_apps() -> Vec<App> {
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

pub fn launch(exec: &str) {
    let _ = std::process::Command::new("sh").arg("-c").arg(exec).spawn();
}
