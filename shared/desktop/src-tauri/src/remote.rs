//! Settings → Remote for the desktop shell (#448). The engine is already in
//! `morrow_core::remote` and `morrow_platform::remote`; this module only holds
//! the host list, hands the bundled hook and plugin sources to the platform
//! tunnel, and turns its generation-guarded status into a Tauri event.
//!
//! Hosts live in the same `~/.codeisland/settings.json` the other pages use.
//! Every decision a test needs (validation, dedupe, status mapping) is a pure
//! function here; only the commands and the pump thread touch a process or a
//! clock. No new dependency and no second remote engine.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use morrow_core::remote::host::SshHost;
use morrow_core::remote::tunnel::TunnelStatus;
use morrow_platform::remote::tunnel::RemoteTunnel;
use morrow_platform::settings;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

/// The hook and plugin the platform installer writes on the host. Both come
/// from the shared resource tree at compile time, never from an app bundle or
/// `/Applications`, so the Linux and Windows paths are the same as macOS.
const HOOK_SOURCE: &str =
    include_str!("../../../../macos/Sources/Morrow/Resources/morrow-remote-hook.py");
const PLUGIN_SOURCE: &str =
    include_str!("../../../../macos/Sources/Morrow/Resources/morrow-opencode-remote.js");

pub const KEY_REMOTE_HOSTS: &str = "remoteHosts";
pub const REMOTE_STATUS_EVENT: &str = "remote-status-changed";

/// What the card draws. `Failed` carries the engine's message, including the
/// folder/socket refusal details, unmodified.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RemoteStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

impl From<&TunnelStatus> for RemoteStatus {
    fn from(status: &TunnelStatus) -> Self {
        match status {
            TunnelStatus::Disconnected => Self::Disconnected,
            TunnelStatus::Connecting => Self::Connecting,
            TunnelStatus::Connected => Self::Connected,
            TunnelStatus::Failed(message) => Self::Failed(message.clone()),
        }
    }
}

static DISCONNECTED: TunnelStatus = TunnelStatus::Disconnected;

/// Read the host list from one settings document. A missing key or a malformed
/// value is an empty list, never a crash on startup.
pub fn hosts_from_value(value: &Value) -> Vec<SshHost> {
    value
        .as_object()
        .and_then(|object| object.get(KEY_REMOTE_HOSTS))
        .and_then(serde_json::Value::as_array)
        .and_then(|rows| serde_json::from_value::<Vec<SshHost>>(Value::Array(rows.clone())).ok())
        .unwrap_or_default()
}

/// One host added or replaced by id. Refuses a blank id or a field ssh could
/// read as an option, before anything is persisted or any process is started.
pub fn save_host(hosts: &mut Vec<SshHost>, host: SshHost) -> Result<bool, String> {
    if host.id.trim().is_empty() {
        return Err("This host has no ID; remove it and add it again.".into());
    }
    if let Some(field) = host.unsafe_ssh_field() {
        return Err(format!(
            "\"{field}\" can't be given to ssh as it is: a host, user or identity file may not start with '-' or hold control characters, a host has no spaces either, and a port is 1-65535."
        ));
    }
    if let Some(existing) = hosts.iter_mut().find(|row| row.id == host.id) {
        if *existing == host {
            return Ok(false);
        }
        *existing = host;
        return Ok(true);
    }
    hosts.push(host);
    Ok(true)
}

pub fn remove_host(hosts: &mut Vec<SshHost>, id: &str) -> bool {
    let before = hosts.len();
    hosts.retain(|host| host.id != id);
    hosts.len() != before
}

/// The shell's half of the remote slice. Tauri manages this behind an
/// `Arc<Mutex<_>>`: commands mutate it, one pump thread drives the engine's
/// `poll()` and publishes what `take_status_change()` reports.
pub struct RemoteManager {
    codeisland_dir: PathBuf,
    local_socket_path: String,
    binary: String,
    hook_source: String,
    plugin_source: String,
    hosts: Vec<SshHost>,
    tunnels: BTreeMap<String, RemoteTunnel>,
}

impl RemoteManager {
    /// Production wiring: the resolved home, this island's own endpoint, the
    /// `ssh` on PATH and the sources compiled into the binary.
    pub fn new(codeisland_dir: PathBuf, local_socket_path: String) -> Self {
        Self::with_options(
            codeisland_dir,
            local_socket_path,
            "ssh".to_owned(),
            HOOK_SOURCE.to_owned(),
            PLUGIN_SOURCE.to_owned(),
        )
    }

    /// Tests point the binary at a stand-in and inject the sources so no real
    /// ssh or app resource is needed to drive the decisions.
    pub fn with_options(
        codeisland_dir: PathBuf,
        local_socket_path: String,
        binary: String,
        hook_source: String,
        plugin_source: String,
    ) -> Self {
        let hosts = settings::load(&codeisland_dir)
            .map(|stored| hosts_from_value(&stored))
            .unwrap_or_default();
        Self {
            codeisland_dir,
            local_socket_path,
            binary,
            hook_source,
            plugin_source,
            hosts,
            tunnels: BTreeMap::new(),
        }
    }

    pub fn hosts(&self) -> Vec<SshHost> {
        self.hosts.clone()
    }

    /// One status per saved host, so the card can draw a row before any tunnel
    /// has been opened.
    pub fn statuses(&self) -> BTreeMap<String, RemoteStatus> {
        self.hosts
            .iter()
            .map(|host| {
                let status = self
                    .tunnels
                    .get(&host.id)
                    .map(RemoteTunnel::status)
                    .unwrap_or(&DISCONNECTED);
                (host.id.clone(), RemoteStatus::from(status))
            })
            .collect()
    }

    fn host(&self, id: &str) -> Result<&SshHost, String> {
        self.hosts
            .iter()
            .find(|host| host.id == id)
            .ok_or_else(|| format!("no remote host with id {id}"))
    }

    fn persist(&self) -> Result<(), String> {
        let value = serde_json::to_value(&self.hosts).map_err(|error| error.to_string())?;
        settings::set(&self.codeisland_dir, KEY_REMOTE_HOSTS, &value)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn tunnel(&mut self, id: &str) -> &mut RemoteTunnel {
        self.tunnels.entry(id.to_owned()).or_insert_with(|| {
            RemoteTunnel::with_binary(self.binary.clone())
                .with_resources(self.hook_source.clone(), self.plugin_source.clone())
        })
    }

    fn validate(&self, id: &str) -> Result<(), String> {
        save_host(&mut Vec::new(), self.host(id)?.clone()).map(|_| ())
    }

    pub fn save(&mut self, host: SshHost) -> Result<bool, String> {
        let changed = save_host(&mut self.hosts, host)?;
        if changed {
            // Editing a field the connection depends on drops the held tunnel:
            // the next connect rebuilds it from the new host.
            self.tunnels.clear();
            self.persist()?;
        }
        Ok(changed)
    }

    pub fn remove(&mut self, id: &str) -> Result<bool, String> {
        self.disconnect(id);
        self.tunnels.remove(id);
        let changed = remove_host(&mut self.hosts, id);
        if changed {
            self.persist()?;
        }
        Ok(changed)
    }

    /// Called on a worker thread: the engine's `connect` runs ssh probes and
    /// waits for the socket, which must not freeze the Settings window.
    pub fn finish_connect(&mut self, id: &str) {
        let Ok(host) = self.host(id).cloned() else {
            return;
        };
        let socket = self.local_socket_path.clone();
        self.tunnel(id).connect(&host, &socket);
    }

    pub fn disconnect(&mut self, id: &str) {
        if let Some(tunnel) = self.tunnels.get_mut(id) {
            tunnel.disconnect();
        }
    }

    /// Drive the engine once. True when it published a status the card has not
    /// seen, so the pump emits only on a change rather than on a timer.
    pub fn poll_all(&mut self) -> bool {
        let mut changed = false;
        for tunnel in self.tunnels.values_mut() {
            tunnel.poll();
            changed |= tunnel.take_status_change().is_some();
        }
        changed
    }
}

fn emit_statuses(app: &AppHandle, manager: &RemoteManager) {
    let _ = app.emit(REMOTE_STATUS_EVENT, manager.statuses());
}

fn lock(
    manager: &Arc<Mutex<RemoteManager>>,
) -> Result<std::sync::MutexGuard<'_, RemoteManager>, String> {
    manager
        .lock()
        .map_err(|_| "remote state poisoned".to_owned())
}

// --- Tauri commands --------------------------------------------------------

#[tauri::command]
pub fn remote_hosts(manager: State<'_, Arc<Mutex<RemoteManager>>>) -> Result<Vec<SshHost>, String> {
    Ok(lock(&manager)?.hosts())
}

#[tauri::command]
pub fn remote_status(
    manager: State<'_, Arc<Mutex<RemoteManager>>>,
) -> Result<BTreeMap<String, RemoteStatus>, String> {
    Ok(lock(&manager)?.statuses())
}

#[tauri::command]
pub fn remote_host_save(
    app: AppHandle,
    manager: State<'_, Arc<Mutex<RemoteManager>>>,
    host: SshHost,
) -> Result<bool, String> {
    let mut state = lock(&manager)?;
    let changed = state.save(host)?;
    emit_statuses(&app, &state);
    Ok(changed)
}

#[tauri::command]
pub fn remote_host_remove(
    app: AppHandle,
    manager: State<'_, Arc<Mutex<RemoteManager>>>,
    id: String,
) -> Result<bool, String> {
    let mut state = lock(&manager)?;
    let changed = state.remove(&id)?;
    emit_statuses(&app, &state);
    Ok(changed)
}

#[tauri::command]
pub fn remote_connect(
    app: AppHandle,
    manager: State<'_, Arc<Mutex<RemoteManager>>>,
    id: String,
) -> Result<RemoteStatus, String> {
    let state = Arc::clone(&manager);
    {
        let manager = lock(&state)?;
        manager.validate(&id)?;
    }
    let worker = Arc::clone(&state);
    let worker_app = app.clone();
    let worker_id = id.clone();
    thread::spawn(move || {
        if let Ok(mut manager) = lock(&worker) {
            manager.finish_connect(&worker_id);
            emit_statuses(&worker_app, &manager);
        }
    });
    Ok(RemoteStatus::Connecting)
}

#[tauri::command]
pub fn remote_disconnect(
    app: AppHandle,
    manager: State<'_, Arc<Mutex<RemoteManager>>>,
    id: String,
) -> Result<RemoteStatus, String> {
    let mut state = lock(&manager)?;
    if state.host(&id).is_err() {
        return Err(format!("no remote host with id {id}"));
    }
    state.disconnect(&id);
    emit_statuses(&app, &state);
    Ok(RemoteStatus::Disconnected)
}

/// Spawn the one pump that notices an ssh exit. The engine owns the generation
/// rule and the status change flag; this thread only drives `poll()` and
/// publishes what it reports, so the frontend never polls status itself.
pub fn spawn_status_pump(app: AppHandle, manager: Arc<Mutex<RemoteManager>>) {
    thread::spawn(move || {
        loop {
            thread::sleep(std::time::Duration::from_millis(400));
            let Ok(mut state) = lock(&manager) else {
                continue;
            };
            if state.poll_all() {
                emit_statuses(&app, &state);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;

    fn host(id: &str, host: &str) -> SshHost {
        SshHost {
            id: id.into(),
            name: id.into(),
            host: host.into(),
            user: "me".into(),
            port: Some(22),
            ..SshHost::default()
        }
    }

    fn settings_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("morrow-remote-{name}-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn hosts_from_value_skips_a_malformed_list() {
        assert!(hosts_from_value(&serde_json::json!({})).is_empty());
        assert!(
            hosts_from_value(&serde_json::json!({ KEY_REMOTE_HOSTS: "not a list" })).is_empty()
        );
        let stored = serde_json::json!({ KEY_REMOTE_HOSTS: [host("a", "box.example")] });
        let rows = hosts_from_value(&stored);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ssh_target(), "me@box.example");
    }

    #[test]
    fn saving_refuses_a_blank_id_and_an_unsafe_ssh_field() {
        let mut hosts = Vec::new();
        assert!(save_host(&mut hosts, host("", "box.example")).is_err());
        assert!(save_host(&mut hosts, host("a", "-proxy")).is_err());
        let mut bad_port = host("a", "box.example");
        bad_port.port = Some(70_000);
        assert!(save_host(&mut hosts, bad_port).is_err());
        assert!(hosts.is_empty());

        assert_eq!(save_host(&mut hosts, host("a", "one.example")), Ok(true));
        assert_eq!(save_host(&mut hosts, host("a", "one.example")), Ok(false));
        assert_eq!(save_host(&mut hosts, host("a", "two.example")), Ok(true));
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].host, "two.example");
        assert!(remove_host(&mut hosts, "a"));
        assert!(hosts.is_empty());
    }

    #[test]
    fn an_invalid_host_is_refused_without_starting_ssh() {
        // A saved host whose name looks like an option must be refused by the
        // engine before it spawns anything; a binary that cannot exist proves
        // no process was launched.
        let dir = settings_dir("invalid");
        let stored = serde_json::json!({ KEY_REMOTE_HOSTS: [host("a", "-proxy")] });
        settings::set(&dir, KEY_REMOTE_HOSTS, &stored[KEY_REMOTE_HOSTS]).expect("a saved host");
        let mut manager = RemoteManager::with_options(
            dir,
            "/tmp/morrow/island.sock".into(),
            "definitely-not-ssh".into(),
            "hook".into(),
            "plugin".into(),
        );
        assert_eq!(manager.hosts().len(), 1);
        manager.finish_connect("a");
        assert_eq!(
            manager.statuses().get("a"),
            Some(&RemoteStatus::Failed("invalid host".into()))
        );
    }

    #[test]
    fn status_view_maps_every_engine_status() {
        assert_eq!(
            RemoteStatus::from(&TunnelStatus::Connecting),
            RemoteStatus::Connecting
        );
        assert_eq!(
            RemoteStatus::from(&TunnelStatus::Connected),
            RemoteStatus::Connected
        );
        assert_eq!(
            RemoteStatus::from(&TunnelStatus::Failed("no route".into())),
            RemoteStatus::Failed("no route".into())
        );
        let mut manager = RemoteManager::with_options(
            settings_dir("statuses"),
            "/tmp/morrow/island.sock".into(),
            "ssh".into(),
            "hook".into(),
            "plugin".into(),
        );
        assert_eq!(manager.save(host("a", "box.example")), Ok(true));
        // A host with no tunnel yet still draws a row, as Disconnected.
        assert_eq!(
            manager.statuses().get("a"),
            Some(&RemoteStatus::Disconnected)
        );
    }

    #[test]
    fn every_remote_command_has_a_frontend_caller() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut source = String::new();
        let mut stack = vec![root.join("../src")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("svelte" | "ts" | "js")
                ) {
                    source.push_str(&fs::read_to_string(&path).unwrap_or_default());
                    source.push('\n');
                }
            }
        }
        let commands: BTreeSet<&str> = [
            "remote_hosts",
            "remote_status",
            "remote_host_save",
            "remote_host_remove",
            "remote_connect",
            "remote_disconnect",
        ]
        .into();
        for command in commands {
            assert!(
                source.contains(&format!("'{command}'"))
                    || source.contains(&format!("\"{command}\"")),
                "{command} is registered but the frontend never invokes it"
            );
        }
    }
}
