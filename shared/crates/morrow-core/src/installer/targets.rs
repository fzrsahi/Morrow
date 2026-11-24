//! The hook targets besides Claude Code, and the shape each one's file takes
//! (`ConfigInstaller.swift`'s `builtInCLIs` for Codex, Gemini, Cursor, Qoder,
//! Factory/Droid, CodeBuddy and Copilot CLI).
//!
//! A plan over text, like [`super::claude_hooks`]: the platform names the
//! command, reads the file and writes the result.

use serde_json::{Map, Value, json};

use super::claude_hooks::{contains_our_hook, remove_managed_entries};
use super::json_editor::{delete_top_level_key, set_top_level_value};
use super::settings::{object, same_document};

/// How one CLI's file holds its hooks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// `{matcher, hooks: [{type, command, timeout}]}` — Claude forks.
    Claude,
    /// `{hooks: [{type, command, timeout}]}` — Codex, Gemini.
    Nested,
    /// `{command}` — Cursor.
    Flat,
    /// `{type, bash, timeoutSec}` in a file of its own — Copilot CLI.
    Copilot,
    /// `{matcher, loop_limit, hooks: [...]}` — Trae's IDE, which wants the Claude
    /// shape plus a loop limit, and names the event on the command line.
    TraeIde,
    /// `{command, matcher, timeout_ms}` — Kiro's agent file, beside Claude's
    /// naming a `matcher` on every entry.
    KiroAgent,
    /// Google Antigravity: a named-config wrapper whose entries take **two
    /// different shapes** depending on the event family.
    AntigravityNamed,
}

/// One hook event to register, with the timeout the CLI expects (seconds,
/// except Gemini, which counts milliseconds).
pub struct TargetEvent {
    pub name: &'static str,
    pub timeout: i64,
    pub async_: bool,
}

const fn event(name: &'static str, timeout: i64, async_: bool) -> TargetEvent {
    TargetEvent {
        name,
        timeout,
        async_,
    }
}

/// Where a target's file hangs from.
///
/// Almost every CLI keeps its config in the user's home, so `<home>/<path>` is the
/// whole answer. Grok is the exception that makes this a field rather than an
/// assumption: its file lives under `$GROK_HOME`, which a second account points
/// somewhere else entirely — and writing to `~/.grok` then would hook an account
/// the user is not running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Root {
    /// `<home>/<path>`.
    Home,
    /// `$GROK_HOME`, expanded, or `<home>/.grok` when it is unset, blank or `~`.
    GrokHome,
}

impl Root {
    /// The directory the rest of the path hangs from.
    ///
    /// `env` is injected rather than read here: this module is pure (core has no
    /// business reading the process environment), and a test can then hand it
    /// `GROK_HOME=/work/grok` without mutating anything global.
    pub fn directory(
        &self,
        home: &std::path::Path,
        env: impl Fn(&str) -> Option<String>,
    ) -> std::path::PathBuf {
        match self {
            Root::Home => home.to_path_buf(),
            Root::GrokHome => {
                let raw = env("GROK_HOME").unwrap_or_default().trim().to_owned();
                if raw.is_empty() {
                    return home.join(".grok");
                }
                // `~` and `~/x` are documented forms and reach the process
                // unexpanded: writing beside a literal `~` is the bug #258 fixed
                // for Claude's config dir.
                if raw == "~" {
                    return home.to_path_buf();
                }
                if let Some(rest) = raw.strip_prefix("~/") {
                    return home.join(rest);
                }
                std::path::PathBuf::from(raw)
            }
        }
    }
}

/// A CLI Morrow can hook, and where its file is.
pub struct Target {
    /// The `--source` the bridge is told.
    pub source: &'static str,
    pub name: &'static str,
    /// Relative to [`Target::root`].
    pub path: &'static str,
    /// Where `path` hangs from.
    pub root: Root,
    /// The top-level key the hooks live under.
    pub key: &'static str,
    pub format: Format,
    pub events: &'static [TargetEvent],
    /// Some CLIs are reported under another source (`--source` in the entry).
    pub bridge_source: Option<&'static str>,
    /// A file Copilot reads needs `version: 1` at the top level.
    pub seeds_version: bool,
}

impl Target {
    pub fn bridge_source(&self) -> &'static str {
        self.bridge_source.unwrap_or(self.source)
    }

    /// Whether `event` is one of the tool events, which Antigravity wraps
    /// differently from the rest.
    pub fn is_tool_event(&self, event: &str) -> bool {
        matches!(event, "PreToolUse" | "PostToolUse")
    }

    /// The file this target's hooks live in.
    pub fn file(
        &self,
        home: &std::path::Path,
        env: impl Fn(&str) -> Option<String>,
    ) -> std::path::PathBuf {
        self.root.directory(home, env).join(self.path)
    }
}

/// `defaultEvents(for: .claude)`: the same list Claude Code gets, for the forks
/// that share its schema.
const CLAUDE_EVENTS: &[TargetEvent] = &[
    event("UserPromptSubmit", 5, true),
    event("PreToolUse", 5, false),
    event("PostToolUse", 5, true),
    event("SessionStart", 5, false),
    event("SessionEnd", 5, true),
    event("Stop", 5, true),
    event("SubagentStart", 5, true),
    event("SubagentStop", 5, true),
    event("Notification", 86_400, false),
    event("PreCompact", 5, true),
];

/// Qoder's own event list, shared by the international build, the China build and
/// QoderWork: a Claude Code fork with its own documented `PermissionRequest` hook.
const QODER_EVENTS: &[TargetEvent] = &[
    event("UserPromptSubmit", 5, true),
    event("PreToolUse", 5, false),
    event("PostToolUse", 5, true),
    event("PostToolUseFailure", 5, true),
    event("PermissionRequest", 86_400, false),
    event("Stop", 5, true),
    event("SubagentStart", 5, true),
    event("SubagentStop", 5, true),
    event("SessionStart", 5, false),
    event("SessionEnd", 5, true),
    event("Notification", 86_400, false),
    event("PreCompact", 5, true),
];

/// Qwen Code counts **milliseconds**, like Gemini, where every other Claude fork
/// here counts seconds. Copying `CLAUDE_EVENTS` would give it a five-second
/// timeout that expires in five thousandths of a second.
const QWEN_EVENTS: &[TargetEvent] = &[
    event("UserPromptSubmit", 5_000, true),
    event("PreToolUse", 5_000, false),
    event("PostToolUse", 5_000, true),
    event("PostToolUseFailure", 5_000, true),
    event("PermissionRequest", 86_400_000, false),
    event("Stop", 5_000, true),
    event("SubagentStart", 5_000, true),
    event("SubagentStop", 5_000, true),
    event("SessionStart", 5_000, false),
    event("SessionEnd", 5_000, true),
    event("Notification", 86_400_000, false),
    event("PreCompact", 5_000, true),
];

/// `defaultEvents(for: .traeIDE)`: the six TraeCode fires. Both editions document
/// the same list; the Cursor-style camelCase names a `.flat` target uses never
/// fire there.
const TRAE_IDE_EVENTS: &[TargetEvent] = &[
    event("SessionStart", 5, false),
    event("UserPromptSubmit", 5, false),
    event("PreToolUse", 5, false),
    event("PostToolUse", 5, false),
    event("Stop", 5, false),
    // Trae ignores Notification's stdout, so it gets the same short timeout as
    // the status events rather than the long one a blocking hook would want.
    event("Notification", 5, false),
];

/// `defaultEvents(for: .kiroAgent)`: Kiro's camelCase events, five of them, with
/// seconds stored in the table and milliseconds written to the file.
const KIRO_AGENT_EVENTS: &[TargetEvent] = &[
    event("agentSpawn", 5, false),
    event("userPromptSubmit", 5, true),
    event("preToolUse", 5, false),
    event("postToolUse", 5, true),
    event("stop", 5, true),
];

/// `defaultEvents(for: .antigravityNamed)`: the three actionable events.
///
/// `PreInvocation`/`PostInvocation` are pass-through with no meaning for a card, so
/// they are left out. `PreToolUse` keeps the documented 30-second default rather
/// than a day-long ceiling: Antigravity counts a killed hook as a denial, and a
/// bridge that has to be killed should not be able to sit on an approval.
const ANTIGRAVITY_EVENTS: &[TargetEvent] = &[
    event("PreToolUse", 30, false),
    event("PostToolUse", 5, false),
    event("Stop", 5, false),
];

/// Grok's managed events (`GrokHookForwardingPolicy.managedHookEvents`).
///
/// Grok imports hooks from Claude and Cursor configs, so this list is wider than a
/// Claude fork's: `PermissionDenied`, `StopFailure` and `PostCompact` exist only
/// here. All five seconds, none async.
const GROK_EVENTS: &[TargetEvent] = &[
    event("SessionStart", 5, false),
    event("UserPromptSubmit", 5, false),
    event("PreToolUse", 5, false),
    event("PostToolUse", 5, false),
    event("PostToolUseFailure", 5, false),
    event("PermissionDenied", 5, false),
    event("Stop", 5, false),
    event("StopFailure", 5, false),
    event("Notification", 5, false),
    event("SubagentStart", 5, false),
    event("SubagentStop", 5, false),
    event("PreCompact", 5, false),
    event("PostCompact", 5, false),
    event("SessionEnd", 5, false),
];

/// `traecliNextEvents()`: Trae CLI Next's names, which are Claude's with compact
/// events on both sides of the turn.
const TRAECLI_NEXT_EVENTS: &[TargetEvent] = &[
    event("SessionStart", 5, false),
    event("SessionEnd", 5, true),
    event("UserPromptSubmit", 5, true),
    event("PreToolUse", 5, false),
    event("PostToolUse", 5, true),
    event("PostToolUseFailure", 5, true),
    event("PermissionRequest", 86_400, false),
    event("Notification", 86_400, false),
    event("SubagentStart", 5, true),
    event("SubagentStop", 5, true),
    event("Stop", 5, true),
    event("PreCompact", 5, true),
    event("PostCompact", 5, true),
];

/// Every target this module can install. Claude Code is not here: it has its
/// own installer, because its events and its script came first.
pub const TARGETS: &[Target] = &[
    Target {
        source: "codex",
        name: "Codex",
        path: ".codex/hooks.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Nested,
        events: &[
            event("SessionStart", 5, false),
            event("SessionEnd", 3, true),
            event("UserPromptSubmit", 5, false),
            event("PreToolUse", 5, false),
            event("PostToolUse", 5, false),
            // Codex fires PermissionRequest before shell escalation; without it
            // the panel stays on "running" and the approval sound never plays.
            event("PermissionRequest", 86_400, false),
            event("PreCompact", 5, false),
            event("PostCompact", 5, false),
            event("SubagentStart", 5, false),
            event("SubagentStop", 5, false),
            event("Stop", 5, false),
            event("Interrupt", 3, false),
        ],
        bridge_source: None,
        seeds_version: false,
    },
    // Gemini counts milliseconds, and its hook names are its own.
    Target {
        source: "gemini",
        name: "Gemini",
        path: ".gemini/settings.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Nested,
        events: &[
            event("SessionStart", 10_000, false),
            event("SessionEnd", 10_000, false),
            event("BeforeTool", 86_400_000, false),
            event("AfterTool", 10_000, false),
            event("BeforeAgent", 10_000, false),
            event("AfterAgent", 10_000, false),
        ],
        bridge_source: None,
        seeds_version: false,
    },
    Target {
        source: "cursor",
        name: "Cursor",
        path: ".cursor/hooks.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Flat,
        events: &[
            event("beforeSubmitPrompt", 5, false),
            event("beforeShellExecution", 5, false),
            event("afterShellExecution", 5, false),
            event("beforeReadFile", 5, false),
            event("afterFileEdit", 5, false),
            event("beforeMCPExecution", 5, false),
            event("afterMCPExecution", 5, false),
            event("afterAgentThought", 5, false),
            event("afterAgentResponse", 5, false),
            event("stop", 5, false),
        ],
        bridge_source: None,
        seeds_version: false,
    },
    // A Claude Code fork with its own PermissionRequest hook.
    Target {
        source: "qoder",
        name: "Qoder",
        path: ".qoder/settings.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Claude,
        events: QODER_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    // Factory's Droid and CodeBuddy both speak Claude Code's schema.
    Target {
        source: "droid",
        name: "Factory",
        path: ".factory/settings.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Claude,
        events: CLAUDE_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    Target {
        source: "codebuddy",
        name: "CodeBuddy",
        path: ".codebuddy/settings.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Claude,
        events: CLAUDE_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    // Copilot CLI's stdin carries no session or event name, so the event is a
    // flag, and the file must say which schema it is.
    Target {
        source: "copilot",
        name: "Copilot",
        path: ".copilot/hooks/codeisland.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Copilot,
        events: &[
            event("sessionStart", 5, false),
            event("sessionEnd", 5, true),
            event("userPromptSubmitted", 5, false),
            event("preToolUse", 5, false),
            event("postToolUse", 5, true),
            event("errorOccurred", 5, true),
        ],
        bridge_source: None,
        seeds_version: true,
    },
    // The seven Claude-schema forks of #234 batch 1. Each was 40 lines of table
    // and path in the Swift installer, and each is the same 40 lines here — which
    // is why they are one appended block and not seven slices.
    //
    // Qoder CN (国行) is the China build: same hook contract, own config root, and
    // its own CLI name. Its events are reported under the **shared** `qoder-cli`
    // source, as macOS does (#289) — a card keyed by `qoder-cn` would be a second
    // card for one session.
    Target {
        source: "qoder-cn",
        name: "Qoder CN",
        path: ".qoder-cn/settings.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Claude,
        events: QODER_EVENTS,
        bridge_source: Some("qoder-cli"),
        seeds_version: false,
    },
    // QoderWork is Qoder's standalone desktop assistant, not the IDE: user-level
    // `~/.qoderwork/settings.json` only, and it reads its hooks at launch — the
    // user has to restart it after an install or uninstall.
    Target {
        source: "qoderwork",
        name: "QoderWork",
        path: ".qoderwork/settings.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Claude,
        events: QODER_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    // CodeBuddy's China variant, and the same schema.
    Target {
        source: "codybuddycn",
        name: "CodyBuddyCN",
        path: ".codybuddycn/settings.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Claude,
        events: CLAUDE_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    Target {
        source: "stepfun",
        name: "StepFun",
        path: ".stepfun/settings.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Claude,
        events: CLAUDE_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    // AntiGravity — a Claude Code fork. **Not** `google-antigravity`, which is a
    // different product with a named-config schema and a different file; writing
    // one into the other's path would put hooks where nothing reads them. That
    // one is #234 batch 4.
    Target {
        source: "antigravity",
        name: "AntiGravity",
        path: ".antigravity/settings.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Claude,
        events: CLAUDE_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    Target {
        source: "workbuddy",
        name: "WorkBuddy",
        path: ".workbuddy/settings.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Claude,
        events: CLAUDE_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    // Qwen Code: Claude's schema, Gemini's unit.
    Target {
        source: "qwen",
        name: "Qwen Code",
        path: ".qwen/settings.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Claude,
        events: QWEN_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    // Grok Build CLI (#234 batch 2). A JSON file of its own under `$GROK_HOME`,
    // and `.nested` rather than the Claude format because Grok rejects `matcher`
    // on lifecycle events such as SessionStart and Stop.
    Target {
        source: "grok",
        name: "Grok CLI",
        path: "hooks/codeisland.json",
        root: Root::GrokHome,
        key: "hooks",
        format: Format::Nested,
        events: GROK_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    // Trae CLI Next moved its hooks under `~/.trae/cli` and reads TraeX event
    // names, but reports them under the same `traecli` source as the older CLI —
    // one product, two config layouts.
    // Trae / Trae CN: the desktop IDE, whose hooks file is `~/.trae{,-cn}/hooks.json`.
    // Two editions, one format, two sources — the user can have both installed.
    Target {
        source: "trae",
        name: "Trae",
        path: ".trae/hooks.json",
        root: Root::Home,
        key: "hooks",
        format: Format::TraeIde,
        events: TRAE_IDE_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    Target {
        source: "traecn",
        name: "Trae CN",
        path: ".trae-cn/hooks.json",
        root: Root::Home,
        key: "hooks",
        format: Format::TraeIde,
        events: TRAE_IDE_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    // Kiro's agent file: one file per agent under `~/.kiro/agents`, so its name is
    // ours and removing it is removing our own file.
    Target {
        source: "kiro",
        name: "Kiro",
        path: ".kiro/agents/codeisland.json",
        root: Root::Home,
        key: "hooks",
        format: Format::KiroAgent,
        events: KIRO_AGENT_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    // Google Antigravity — **not** the `antigravity` Claude fork above. It reads a
    // standalone `~/.gemini/config/hooks.json` (not Gemini-CLI's settings file),
    // wrapped in a named config keyed `codeisland`, and its stdin carries no event
    // name, so every command needs `--event`.
    Target {
        source: "google-antigravity",
        name: "Google Antigravity",
        path: ".gemini/config/hooks.json",
        root: Root::Home,
        key: "codeisland",
        format: Format::AntigravityNamed,
        events: ANTIGRAVITY_EVENTS,
        bridge_source: None,
        seeds_version: false,
    },
    Target {
        source: "traecli-next",
        name: "Trae CLI Next",
        path: ".trae/cli/hooks.json",
        root: Root::Home,
        key: "hooks",
        format: Format::Nested,
        events: TRAECLI_NEXT_EVENTS,
        bridge_source: Some("traecli"),
        seeds_version: false,
    },
];

/// The target Morrow installs for `--source`, if it has one.
pub fn target_for(source: &str) -> Option<&'static Target> {
    TARGETS.iter().find(|target| target.source == source)
}

/// What an install came to.
pub struct Outcome {
    pub changed: bool,
    pub text: String,
}

/// The command one event's entry carries.
///
/// `bridge` is the hook script with no arguments (e.g.
/// `~/.codeisland/codeisland-hook.sh`); a path with a space is quoted, as the
/// Swift installer does.
pub fn command_for(target: &Target, bridge: &str, event: &str) -> String {
    let bridge = if bridge.contains(' ') {
        format!("\"{bridge}\"")
    } else {
        bridge.to_owned()
    };
    let base = format!("{bridge} --source {}", target.bridge_source());
    match target.format {
        // Gemini and Copilot do not report their event on stdin.
        Format::Nested if target.source == "gemini" => format!("{base} --event {event}"),
        Format::Flat | Format::Copilot | Format::TraeIde | Format::AntigravityNamed => {
            format!("{base} --event {event}")
        }
        _ => base,
    }
}

fn entry_for(target: &Target, bridge: &str, event: &'static TargetEvent) -> Value {
    let command = command_for(target, bridge, event.name);
    match target.format {
        Format::Claude => json!({
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": command,
                "timeout": event.timeout,
            }],
        }),
        Format::Nested => json!({
            "hooks": [{
                "type": "command",
                "command": command,
                "timeout": event.timeout,
            }],
        }),
        Format::Flat => json!({"command": command}),
        Format::Copilot => json!({
            "type": "command",
            "bash": command,
            "timeoutSec": event.timeout,
        }),
        // Trae's IDE wants Claude's shape plus a loop limit: an entry that keeps
        // re-firing is what `loop_limit` bounds, and Trae refuses the file without
        // it.
        Format::TraeIde => json!({
            "matcher": "*",
            "loop_limit": 5,
            "hooks": [{
                "type": "command",
                "command": command,
                "timeout": event.timeout,
            }],
        }),
        // Antigravity has two event families and **two entry shapes**, and getting
        // it wrong is silent: a tool event wrapped the model-event way never runs,
        // and a model event wrapped the tool way never fires either. Tool events
        // take a matcher and a `hooks` array; the rest are the handler itself.
        Format::AntigravityNamed if target.is_tool_event(event.name) => json!({
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": command,
                "timeout": event.timeout,
            }],
        }),
        Format::AntigravityNamed => json!({
            "type": "command",
            "command": command,
            "timeout": event.timeout,
        }),
        // Kiro's agent file names no `hooks` array and takes **milliseconds**
        // where every other target here takes seconds. The table stores seconds
        // for consistency; the conversion happens here, once.
        Format::KiroAgent => json!({
            "command": command,
            "matcher": "*",
            "timeout_ms": event.timeout * 1_000,
        }),
    }
}

/// Whether every event of the target carries our hook (`isHooksInstalled`).
pub fn is_installed(target: &Target, existing: &str) -> bool {
    let Some(settings) = object(existing) else {
        return false;
    };
    let hooks = settings
        .get(target.key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    target.events.iter().all(|expected| {
        hooks
            .get(expected.name)
            .and_then(Value::as_array)
            .is_some_and(|entries| entries.iter().any(contains_our_hook))
    })
}

/// Install or repair one target. `None` when the file does not parse as a JSON
/// object: the caller writes nothing (#89).
pub fn install(target: &Target, existing: Option<&str>, bridge: &str) -> Option<Outcome> {
    let source = existing.filter(|text| !text.is_empty()).unwrap_or("{}");
    let settings = object(source)?;
    let mut hooks = settings
        .get(target.key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    for expected in target.events {
        let mut entries = hooks
            .get(expected.name)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        // Ours goes first, so a reinstall replaces rather than accumulates.
        entries.retain(|entry| !contains_our_hook(entry));
        entries.push(entry_for(target, bridge, expected));
        hooks.insert(expected.name.to_owned(), Value::Array(entries));
    }

    let mut text = set_top_level_value(source, target.key, &Value::Object(hooks))?;
    // Copilot reads a schema version; only insert one the user has not set.
    if target.seeds_version {
        text = seed_version(&text, &settings)?;
    }
    Some(Outcome {
        changed: !same_document(source, &text),
        text,
    })
}

/// Take our entries back out; an emptied key goes altogether, so a file that
/// held only ours comes back byte for byte.
pub fn uninstall(target: &Target, existing: &str) -> Option<Outcome> {
    let settings = object(existing)?;
    let hooks = settings
        .get(target.key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let cleaned = remove_managed_entries(&hooks);
    let text = if cleaned.is_empty() {
        delete_top_level_key(existing, target.key)?
    } else {
        set_top_level_value(existing, target.key, &Value::Object(cleaned))?
    };
    Some(Outcome {
        changed: text != existing,
        text,
    })
}

/// Copilot reads a schema version. One the user set themselves — a future
/// v2 — is left alone (`installExternalHooks`).
fn seed_version(installed: &str, before: &Map<String, Value>) -> Option<String> {
    if before.contains_key("version") || object(installed)?.contains_key("version") {
        return Some(installed.to_owned());
    }
    set_top_level_value(installed, "version", &json!(1))
}
