//! Optional Discord Rich Presence support for Aurelia.
//!
//! Presence is deliberately narrow: it talks to the local Discord client over
//! IPC, never opens a network socket, and never includes a project path. The
//! default activity is generic; a project name is only included when the user
//! explicitly enables that separate option.

use std::io::Write;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

#[cfg(windows)]
mod ipc;

pub const LARGE_IMAGE: &str = "aurelia_logo";
pub const LARGE_TEXT: &str = "Aurelia";
pub const APPLICATION_NAME: &str = "Aurelia Studio";

fn trace(message: &str) {
    let Ok(local) = std::env::var("LOCALAPPDATA") else {
        return;
    };
    let path = Path::new(&local).join("Aurelia").join("presence.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{message}");
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PresenceOptions {
    /// Include the project's display name. Off by default.
    pub show_project_name: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresenceUpdate {
    pub enabled: bool,
    pub show_project_name: bool,
    pub project_name: Option<String>,
    pub painting: bool,
}

impl PresenceUpdate {
    fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.enabled,
            self.show_project_name,
            self.painting,
            self.project_name.as_deref().unwrap_or("")
        )
    }
}

/// Extract a safe display name from a project path. Never returns a path.
pub fn project_name(path: Option<&Path>) -> Option<String> {
    let stem = path?.file_stem()?.to_str()?;
    let cleaned: String = stem
        .chars()
        .filter(|ch| !ch.is_control())
        .take(80)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Build the Rich Presence activity payload.
///
/// The `project` argument must already be a display name (see
/// [`project_name`]); a path passed here would be a bug, which the privacy test
/// in this crate is designed to catch.
pub fn build_activity(
    project: Option<&str>,
    options: &PresenceOptions,
    painting: bool,
    started_at: i64,
    application_id: &str,
) -> Value {
    let state = if options.show_project_name {
        project
            .filter(|name| !name.is_empty())
            .map(|name| format!("Project: {name}"))
            .unwrap_or_else(|| "In Aurelia".to_string())
    } else {
        "In Aurelia".to_string()
    };

    json!({
        // 0 = Playing. Without an explicit type Discord may file the activity
        // under "recent activities" without showing it on the profile.
        "type": 0,
        // Custom RPC implementations inside Discord send these too; they make
        // the activity self-describing even before the server enriches it.
        "name": APPLICATION_NAME,
        "application_id": application_id,
        "flags": 1,
        "details": if painting { "Painting" } else { "Idle" },
        "state": state,
        "timestamps": { "start": started_at },
        "assets": {
            "large_image": LARGE_IMAGE,
            "large_text": LARGE_TEXT
        }
    })
}

/// Owns the background worker. Dropping the manager clears the activity.
pub struct PresenceManager {
    tx: Sender<PresenceUpdate>,
    last: Mutex<Option<PresenceUpdate>>,
}

impl PresenceManager {
    pub fn new(client_id: impl Into<String>) -> Self {
        let client_id = client_id.into();
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let (tx, rx) = channel();
        let _ = thread::Builder::new()
            .name("aurelia-discord-presence".to_string())
            .spawn(move || worker(client_id, started_at, rx));

        Self {
            tx,
            last: Mutex::new(None),
        }
    }

    /// Send an update only when the visible state changed.
    pub fn update(&self, update: PresenceUpdate) {
        let mut last = match self.last.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if last.as_ref() == Some(&update) {
            return;
        }
        *last = Some(update.clone());
        let _ = self.tx.send(update);
    }

    pub fn clear(&self) {
        self.update(PresenceUpdate {
            enabled: false,
            show_project_name: false,
            project_name: None,
            painting: false,
        });
    }
}

impl Drop for PresenceManager {
    fn drop(&mut self) {
        let _ = self.tx.send(PresenceUpdate {
            enabled: false,
            show_project_name: false,
            project_name: None,
            painting: false,
        });
    }
}

#[cfg(windows)]
fn worker(client_id: String, started_at: i64, rx: Receiver<PresenceUpdate>) {
    let mut connection: Option<ipc::DiscordIpc> = None;
    let mut pending: Option<PresenceUpdate> = None;
    let mut last_key: Option<String> = None;
    let mut last_sent = Instant::now() - Duration::from_secs(60);
    let mut next_retry = Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(update) => pending = Some(update),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // Discord pings the socket; answer it even when the presence has not
        // changed, otherwise the connection can be dropped and the activity
        // disappears from the profile.
        if let Some(conn) = connection.as_mut() {
            if conn.poll().is_err() {
                connection = None;
                last_key = None;
            }
        }

        let Some(update) = pending.clone() else {
            continue;
        };
        trace(&format!(
            "update enabled={} key={} conn={}",
            update.enabled,
            update.key(),
            connection.is_some()
        ));

        if !update.enabled {
            if let Some(mut conn) = connection.take() {
                let _ = conn.clear();
            }
            last_key = None;
            pending = None;
            continue;
        }

        if client_id.is_empty() {
            pending = None;
            continue;
        }

        if last_key.as_deref() == Some(update.key().as_str()) {
            pending = None;
            continue;
        }

        // Discord rate-limits activity updates; coalesce inside that window.
        if last_sent.elapsed() < Duration::from_secs(15) {
            continue;
        }

        if connection.is_none() {
            if Instant::now() < next_retry {
                continue;
            }
            connection = ipc::DiscordIpc::connect(&client_id).ok();
            if connection.is_none() {
                trace("connect failed");
                next_retry = Instant::now() + Duration::from_secs(15);
                continue;
            }
            trace("connected");
            // A fresh IPC socket must receive the current activity even if the
            // desired state did not change while the connection was down.
            last_key = None;
        }
        let Some(conn) = connection.as_mut() else {
            continue;
        };

        let activity = build_activity(
            update.project_name.as_deref(),
            &PresenceOptions {
                show_project_name: update.show_project_name,
            },
            update.painting,
            started_at,
            &client_id,
        );

        match conn.set_activity(&activity) {
            Ok(()) => {
                trace("set_activity ok");
                last_key = Some(update.key());
                last_sent = Instant::now();
                pending = None;
            }
            Err(error) => {
                trace(&format!("set_activity failed: {error}"));
                connection = None;
                last_key = None;
                next_retry = Instant::now() + Duration::from_secs(15);
            }
        }
    }
}

#[cfg(not(windows))]
fn worker(_client_id: String, _started_at: i64, rx: Receiver<PresenceUpdate>) {
    while rx.recv().is_ok() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn default_payload_contains_no_project_name_or_path() {
        let path = Path::new("projects").join("secret-client.aur");
        let name = project_name(Some(&path));
        assert_eq!(name.as_deref(), Some("secret-client"));

        let activity = build_activity(
            name.as_deref(),
            &PresenceOptions::default(),
            true,
            1_700_000_000,
            "test-app",
        );
        let serialized = activity.to_string();

        assert!(serialized.contains("Painting"));
        assert!(!serialized.contains("secret-client"));
        assert!(!serialized.contains("projects"));
        assert!(!serialized.contains(".aur"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_never_leak_into_the_default_payload() {
        let path = Path::new(r"C:\Users\alice\secret-client.aur");
        let name = project_name(Some(path));
        let activity = build_activity(
            name.as_deref(),
            &PresenceOptions::default(),
            false,
            1,
            "test-app",
        );
        let serialized = activity.to_string();
        assert!(!serialized.contains("alice"));
        assert!(!serialized.contains(r"C:\"));
        assert!(!serialized.contains("secret-client"));
    }

    #[test]
    fn project_name_is_only_included_when_opted_in() {
        let activity = build_activity(
            Some("client-work"),
            &PresenceOptions {
                show_project_name: true,
            },
            false,
            1,
            "test-app",
        );
        assert_eq!(activity["state"], "Project: client-work");
        assert_eq!(activity["details"], "Idle");
        assert!(!activity.to_string().contains(".aur"));
    }

    #[test]
    fn project_name_strips_paths_and_control_characters() {
        assert_eq!(
            project_name(Some(&Path::new("art").join("two.aur"))).as_deref(),
            Some("two")
        );
        assert_eq!(project_name(Some(Path::new("bad\u{0007}name.aur"))).as_deref(), Some("badname"));
        assert_eq!(project_name(None), None);
    }

    #[test]
    fn long_names_are_truncated() {
        let long = "x".repeat(200);
        let name = project_name(Some(Path::new(&format!("{long}.aur")))).unwrap();
        assert_eq!(name.len(), 80);
    }
}
