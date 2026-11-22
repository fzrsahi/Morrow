//! `morrow-app`: the Tauri v2 shell on Linux (ADR-0002). Listens at the
//! endpoint, reduces each hook into a session list, and shows it in a
//! tight-fit, nonactivating panel, which answers permission requests.

mod adapters;
mod completion;
mod feed;
mod hooks;
mod hotkey;
mod island;
mod listener;
mod music;
mod peek_window;
mod permissions;
mod position;
mod prune;
mod sessions;
mod settings;
mod settings_window;
mod sound;
mod transcripts;
mod tray;
#[cfg(windows)]
mod windows_panel;

use std::process::ExitCode;
use std::sync::Arc;

use morrow_ipc::{Error, Server, current_endpoint};
use tauri::State;
use tauri::ipc::Channel;

use crate::feed::{Feed, Snapshot};
use crate::island::Smoke;
use crate::permissions::Decision;

/// Where the hook installer works: this user's home and the Claude Code config
/// directory, resolved once at launch.
pub struct HooksSite {
    pub paths: morrow_platform::hooks::Paths,
    pub opencode: morrow_platform::opencode::Site,
    pub kimi: morrow_platform::kimi::Site,
    pub yaml: Vec<morrow_platform::yaml_hooks::Site>,
    pub bridge_source: Option<std::path::PathBuf>,
}

/// The frontend's channel for snapshots.
#[tauri::command]
fn subscribe(feed: State<'_, Arc<Feed>>, channel: Channel<Snapshot>) {
    feed.subscribe(channel);
}

/// #197: a locked screen suspends content probes (transcript tails) but does
/// not hide the island window. The frontend owns the lock watch; the shell
/// only gates its own probes.
#[tauri::command]
fn set_probes_active(feed: State<'_, Arc<Feed>>, active: bool) {
    feed.set_probes_active(active);
}

/// The user's Approve or Deny on card `id`. False if the request is no
/// longer held (answered already, or its hook gone).
#[tauri::command]
fn answer_permission(feed: State<'_, Arc<Feed>>, id: u64, decision: Decision) -> bool {
    feed.answer(id, decision)
}

/// #188: one option of a Question card. The shell keys the reply by question
/// text (not header) and writes `updatedInput.answers` exactly as macOS does.
#[tauri::command]
fn answer_question(feed: State<'_, Arc<Feed>>, id: u64, answer: String) -> bool {
    feed.answer_question(id, &answer)
}

/// #190: the wizard's answers, one string per question in request order. The
/// reply is built by the same `question_reply`, so its bytes are #188's shape.
#[tauri::command]
fn answer_questions(feed: State<'_, Arc<Feed>>, id: u64, answers: Vec<String>) -> bool {
    feed.answer_questions(id, answers)
}

/// #188: Skip closes the card without answering it — deny for AskUserQuestion.
#[tauri::command]
fn skip_question(feed: State<'_, Arc<Feed>>, id: u64) -> bool {
    feed.skip_question(id)
}

/// #185: the ✕ on a session card removes it from the shell's store.
#[tauri::command]
fn session_dismiss(feed: State<'_, Arc<Feed>>, session_id: String) {
    feed.dismiss(&session_id);
}

fn main() -> ExitCode {
    let smoke = std::env::var("MORROW_SMOKE").is_ok_and(|v| v == "1");

    let endpoint = current_endpoint();
    let server = match Server::bind(&endpoint) {
        Ok(server) => server,
        Err(Error::AlreadyRunning { path }) => {
            eprintln!("morrow-app: an island already listens at {path}, exiting");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("morrow-app: cannot listen at {endpoint}: {error}");
            return ExitCode::FAILURE;
        }
    };
    if smoke {
        println!("morrow-smoke: listening {}", server.path());
    }

    let hooks_site = hooks::site();
    let position = position::Store::new(&hooks_site.paths.codeisland_dir);
    let feed = Arc::new(Feed::default());
    listener::spawn(server, Arc::clone(&feed));
    Feed::spawn_pruner(Arc::clone(&feed));
    Feed::spawn_pusher(Arc::clone(&feed), move |sessions, cards| {
        if smoke {
            println!("morrow-smoke: pushed sessions={sessions} cards={cards}");
        }
    });

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(feed)
        .manage(hooks_site)
        .manage(Smoke(smoke))
        .manage(position)
        .invoke_handler(tauri::generate_handler![
            subscribe,
            set_probes_active,
            answer_permission,
            answer_question,
            answer_questions,
            skip_question,
            session_dismiss,
            island::set_island_size,
            island::move_island,
            island::commit_island_position,
            island::reset_island_position,
            island::smoke_card,
            island::quit,
            island::island_visible,
            hooks::hooks_status,
            hooks::hooks_install,
            hooks::hooks_uninstall,
            hooks::target_install,
            hooks::target_uninstall,
            hooks::opencode_install,
            hooks::opencode_uninstall,
            hooks::kimi_install,
            hooks::kimi_uninstall,
            hooks::yaml_install,
            hooks::yaml_uninstall,
            settings::capabilities,
            settings::autostart_status,
            settings::autostart_set,
            settings::settings_load,
            settings::settings_set,
            settings::settings_path,
            sound::sound_cues,
            sound::sound_preview,
            settings_window::open_settings,
            peek_window::close_peek,
            music::music_track,
            music::music_position,
            music::music_play_pause,
            music::music_next,
            music::music_previous,
            music::music_seek,
            music::music_volume,
            music::music_volume_set,
            music::music_volume_mute,
            music::music_lyrics,
            adapters::terminal_activate,
            adapters::process_watch,
            adapters::process_alive,
            adapters::screen_locked,
            adapters::fullscreen,
            adapters::quiet_hours,
            adapters::pointer_position,
            adapters::notify_send,
            island::island_question_focus,
            island::anchoring
        ])
        .setup(|app| {
            island::create(app)?;
            peek_window::maybe_show(app)?;
            tray::create(app);
            hotkey::install(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!());
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("morrow-app: {error}");
            ExitCode::FAILURE
        }
    }
}
