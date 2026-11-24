//! What Codex's own settings say about a permission request
//! (`macos/Sources/Morrow/CodexPermissionRules.swift`).
//!
//! Codex has two reviewers — a human, or an automatic one that approves on its own
//! (`auto_review` / `guardian_subagent`). When the automatic one is on, the island must
//! **hand the request back** instead of putting up a card: a card would sit in the panel
//! waiting while the terminal has already moved on. That hand-back is the whole subject
//! of this module, and every way of getting it wrong is silent:
//!
//! - The reviewer can be written three ways — the request's own payload, the rollout's
//!   newest `turn_context`, or `config.toml` — and they override each other in that
//!   order. Desktop selections never reach `config.toml` at all.
//! - A remote Codex reads its own host's config. Nothing here describes it, and nothing
//!   written here reaches it, so a remote request is never handed back: a wrong hand-back
//!   leaves it waiting in a terminal on another machine, while a card can always be
//!   answered.
//! - The "Always allow" rule is a **prefix of the command**, up to three words — and it
//!   is read from the command, never from a `prefix_rule` field in the payload, because
//!   that one is written by the model. Taken from there, `["bash"]` was saved for a card
//!   that showed `ls -la`, and allowed every command after it.
//!
//! The expectations in the tests are copied out of
//! `macos/Tests/MorrowTests/AppStatePermissionFlowTests.swift` — the repaired
//! `davinci-resolve` table, the refused ambiguous pair, the untouched unstamped table,
//! the profile-over-top-level reviewer — so the two implementations are judged on the
//! same values.
//!
//! What is *not* here: reading the rollout off disk and writing `config.toml`. Both need
//! a file system and a root the caller resolves; the decisions they feed are here, and
//! #238 AC 5 asks for exactly those to pass their fixtures before the approval card uses
//! them.

use serde::Serialize;

/// The exact comment stamped above every approval table this crate writes. Only tables
/// carrying it are ours to repair.
pub const MCP_TOOL_APPROVAL_COMMENT: &str =
    r#"# Added by CodeIsland when "Always Allow" is clicked for a Codex MCP tool."#;

/// The stamp above every prefix rule this crate writes.
pub const PREFIX_RULE_COMMENT: &str =
    r#"# Added by CodeIsland when "Always Allow" is clicked for Codex."#;

/// The file a prefix rule is written to, inside a Codex root's `rules/` folder.
pub const PREFIX_RULE_FILE: &str = "codeisland.rules";

/// How many words of a command a prefix rule remembers.
pub const PREFIX_MAX_TOKENS: usize = 3;

/// What the island should do with a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    /// Put up a card: Codex is waiting for a person.
    Ask,
    /// Hand it back to Codex: its own reviewer will decide.
    Defer,
}

/// Whether "Always allow" saves a standing Codex rule, as the card's link claims, or only
/// holds for this session. A remote request's "Always" holds for its session, and so does
/// a local one no rule can describe — an `apply_patch`, a command starting with a
/// variable.
pub fn always_saves_rule(
    is_codex: bool,
    is_remote: bool,
    tool_name: Option<&str>,
    command: Option<&str>,
) -> bool {
    is_codex
        && !is_remote
        && (mcp_tool_approval_target(is_codex, tool_name).is_some()
            || prefix_pattern(tool_name, command).is_some())
}

/// `(server, tool)` from an `mcp__<server>__<tool>` tool name.
pub fn mcp_tool_approval_target(
    is_codex: bool,
    tool_name: Option<&str>,
) -> Option<(String, String)> {
    if !is_codex {
        return None;
    }
    let raw = tool_name?;
    let rest = raw.strip_prefix("mcp__")?;
    let (server, tool) = rest.split_once("__")?;
    if server.is_empty() || tool.is_empty() {
        return None;
    }
    Some((server.to_owned(), tool.to_owned()))
}

/// The prefix rule "Always" saves for a Bash command: its first words, up to three.
pub fn prefix_pattern(tool_name: Option<&str>, command: Option<&str>) -> Option<Vec<String>> {
    if tool_name != Some("Bash") {
        return None;
    }
    shell_prefix(command?, PREFIX_MAX_TOKENS)
}

/// The first `max_tokens` words of a shell command, or `None` when no rule could describe
/// it.
///
/// The scan stops at anything that makes the rest unpredictable: a subshell (`$(`), a
/// pipe, a newline, a redirect, a background job — and an environment assignment in front
/// of the command (`FOO=1 make`) is not a rule at all, so nothing is returned.
pub fn shell_prefix(command: &str, max_tokens: usize) -> Option<Vec<String>> {
    let chars: Vec<char> = command.chars().collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaping = false;
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        let next = index + 1;

        if escaping {
            current.push(ch);
            escaping = false;
            index = next;
            continue;
        }
        if ch == '\\' {
            escaping = true;
            index = next;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            } else {
                current.push(ch);
            }
            index = next;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            index = next;
            continue;
        }
        if ch == '$' && chars.get(next) == Some(&'(') {
            push_token(&mut tokens, &mut current);
            break;
        }
        if matches!(ch, '\n' | '|' | ';' | '<' | '>' | '&') {
            push_token(&mut tokens, &mut current);
            break;
        }
        if ch.is_whitespace() {
            push_token(&mut tokens, &mut current);
            if tokens.len() >= max_tokens {
                break;
            }
        } else {
            current.push(ch);
        }
        index = next;
    }
    push_token(&mut tokens, &mut current);

    tokens.truncate(max_tokens);
    if tokens.is_empty() || looks_like_environment_assignment(&tokens[0]) {
        return None;
    }
    Some(tokens)
}

fn push_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

fn looks_like_environment_assignment(token: &str) -> bool {
    match token.split_once('=') {
        Some((name, _)) if !name.is_empty() => {
            name.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
        }
        _ => false,
    }
}

/// The reviewer named three ways in a payload: `approvals_reviewer`,
/// `approvalsReviewer`, `_approvals_reviewer`.
pub fn event_reviewer(payload: &serde_json::Value) -> Option<String> {
    for key in [
        "approvals_reviewer",
        "approvalsReviewer",
        "_approvals_reviewer",
    ] {
        if let Some(value) = payload.get(key).and_then(|value| value.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}

/// Whether a reviewer value means "Codex decides by itself".
pub fn is_auto_review_reviewer(value: &str) -> bool {
    let normalized = value
        .trim()
        .trim_matches(|ch| ch == '"' || ch == '\'')
        .to_lowercase();
    normalized == "auto_review" || normalized == "guardian_subagent"
}

/// The verdict before the rollout and `config.toml` are consulted.
///
/// A request that names its reviewer is answered by that name alone. A remote request
/// with no reviewer is a card: nothing here can say Codex would review it, and a card can
/// always be answered.
pub fn verdict_from_request(
    is_codex: bool,
    is_remote: bool,
    payload: &serde_json::Value,
) -> Option<Verdict> {
    if !is_codex {
        return None;
    }
    if let Some(reviewer) = event_reviewer(payload) {
        return Some(if is_auto_review_reviewer(&reviewer) {
            Verdict::Defer
        } else {
            Verdict::Ask
        });
    }
    if is_remote {
        return Some(Verdict::Ask);
    }
    None
}

/// What `config.toml` says: the selected profile's `approvals_reviewer` first, then the
/// top-level one.
pub fn config_enables_auto_review(contents: &str) -> bool {
    let mut section: Option<String> = None;
    let mut selected_profile: Option<String> = None;
    let mut top_level_reviewer: Option<String> = None;
    let mut profile_reviewers: Vec<(String, String)> = Vec::new();

    for raw_line in contents.lines() {
        let line = strip_toml_comment(raw_line).trim().to_owned();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = toml_table_name(&line) {
            section = Some(name);
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = toml_scalar_string(raw_value);
        match (&section, key) {
            (None, "profile") => selected_profile = Some(value),
            (None, "approvals_reviewer") => top_level_reviewer = Some(value),
            (Some(name), "approvals_reviewer") => {
                if let Some(profile) = profile_name(name) {
                    profile_reviewers.push((profile, value));
                }
            }
            _ => {}
        }
    }

    if let Some(selected) = selected_profile {
        if let Some((_, reviewer)) = profile_reviewers.iter().find(|(name, _)| *name == selected) {
            return is_auto_review_reviewer(reviewer);
        }
    }
    match top_level_reviewer {
        Some(reviewer) => is_auto_review_reviewer(&reviewer),
        None => false,
    }
}

// MARK: - TOML, as little of it as this needs

/// A line with its comment removed. A `#` inside quotes is part of the value.
pub fn strip_toml_comment(line: &str) -> String {
    let mut result = String::new();
    let mut quote: Option<char> = None;
    let mut escaping = false;
    for ch in line.chars() {
        if let Some(active) = quote {
            result.push(ch);
            if escaping {
                escaping = false;
            } else if active == '"' && ch == '\\' {
                escaping = true;
            } else if ch == active {
                quote = None;
            }
            continue;
        }
        if ch == '#' {
            break;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
        }
        result.push(ch);
    }
    result
}

/// The table name of `[a.b]`, or `None` for anything else (including `[[array]]`).
pub fn toml_table_name(line: &str) -> Option<String> {
    let trimmed = strip_toml_comment(line).trim().to_owned();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') || trimmed.starts_with("[[") {
        return None;
    }
    let body = &trimmed[1..trimmed.len() - 1];
    Some(body.trim().to_owned())
}

/// The dotted segments of a table header, with quoted segments unquoted.
pub fn toml_table_segments(line: &str) -> Option<Vec<String>> {
    let trimmed = strip_toml_comment(line).trim().to_owned();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') || trimmed.starts_with("[[") {
        return None;
    }
    let body = &trimmed[1..trimmed.len() - 1];
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaping = false;
    for ch in body.chars() {
        if let Some(active) = quote {
            if escaping {
                current.push(ch);
                escaping = false;
            } else if active == '"' && ch == '\\' {
                escaping = true;
            } else if ch == active {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '.' => {
                let segment = current.trim();
                if !segment.is_empty() {
                    segments.push(segment.to_owned());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let segment = current.trim();
    if !segment.is_empty() {
        segments.push(segment.to_owned());
    }
    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

/// The key of `key = value`, or `None` for a line that is not an assignment.
pub fn toml_key(line: &str) -> Option<String> {
    let stripped = strip_toml_comment(line);
    let (key, _) = stripped.trim().split_once('=')?;
    Some(key.trim().to_owned())
}

/// A scalar with its surrounding quotes removed.
pub fn toml_scalar_string(raw: &str) -> String {
    let trimmed = raw.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() >= 2
        && (chars[0] == '"' || chars[0] == '\'')
        && chars[0] == chars[chars.len() - 1]
    {
        return chars[1..chars.len() - 1].iter().collect();
    }
    trimmed.to_owned()
}

fn profile_name(section: &str) -> Option<String> {
    let raw = section.strip_prefix("profiles.")?;
    let name = toml_scalar_string(raw);
    if name.is_empty() { None } else { Some(name) }
}

/// How a server name reaches us inside a tool name: anything that is not a TOML bare-key
/// character becomes `_`, because `__` separates the fields.
pub fn sanitized_tool_namespace(server_id: &str) -> String {
    server_id
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// The table key for a segment: bare when TOML allows it, quoted otherwise.
pub fn toml_key_segment(value: &str) -> String {
    let bare = !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-');
    if bare {
        return value.to_owned();
    }
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Every `[mcp_servers.<id>]` that declares a transport.
///
/// A table with no `command` or `url` is not a server, and writing a tool approval under
/// it makes Codex reject the **whole file** — which shows up as unrelated settings
/// failing to save days later.
pub fn declared_mcp_server_ids(contents: &str) -> Vec<String> {
    let lines: Vec<&str> = contents.lines().collect();
    let mut ids: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(segments) = toml_table_segments(line) else {
            continue;
        };
        if segments.len() != 2 || segments[0] != "mcp_servers" {
            continue;
        }
        let end = lines[index + 1..]
            .iter()
            .position(|body| toml_table_segments(body).is_some())
            .map_or(lines.len(), |offset| index + 1 + offset);
        let declares_transport = lines[index + 1..end]
            .iter()
            .any(|body| matches!(toml_key(body).as_deref(), Some("command") | Some("url")));
        if declares_transport && !ids.contains(&segments[1]) {
            ids.push(segments[1].clone());
        }
    }
    ids
}

/// Whether `[mcp_servers.<server_id>]` declares a transport.
pub fn config_declares_mcp_server_transport(contents: &str, server_id: &str) -> bool {
    let lines: Vec<&str> = contents.lines().collect();
    let Some(header) = lines.iter().position(|line| {
        toml_table_segments(line).as_deref()
            == Some(&["mcp_servers".to_owned(), server_id.to_owned()][..])
    }) else {
        return false;
    };
    let end = lines[header + 1..]
        .iter()
        .position(|body| toml_table_segments(body).is_some())
        .map_or(lines.len(), |offset| header + 1 + offset);
    lines[header + 1..end]
        .iter()
        .any(|line| matches!(toml_key(line).as_deref(), Some("command") | Some("url")))
}

/// The config.toml key to write a tool approval under, for the namespace inside an
/// `mcp__<namespace>__<tool>` name.
///
/// The namespace is not always the server key: `davinci-resolve` arrives as
/// `davinci_resolve`. Writing the namespace verbatim creates a second, transport-less
/// table and Codex then refuses to load the file. An exact match wins; a sanitised one is
/// used only when exactly one declared server matches, because approving the wrong server
/// is worse than not persisting.
pub fn declared_mcp_server_id(contents: &str, tool_namespace: &str) -> Option<String> {
    if config_declares_mcp_server_transport(contents, tool_namespace) {
        return Some(tool_namespace.to_owned());
    }
    let candidates: Vec<String> = declared_mcp_server_ids(contents)
        .into_iter()
        .filter(|id| sanitized_tool_namespace(id) == tool_namespace)
        .collect();
    match candidates.len() {
        1 => candidates.into_iter().next(),
        _ => None,
    }
}

/// `contents` with an approval table under `table_path`, or the existing table's
/// `approval_mode` switched to `approve`.
pub fn config_with_mcp_tool_approval(contents: &str, table_path: &[&str], comment: &str) -> String {
    let mut lines: Vec<String> = contents.lines().map(str::to_owned).collect();
    let segments: Vec<String> = table_path.iter().map(|part| (*part).to_owned()).collect();

    if let Some(header) = lines
        .iter()
        .position(|line| toml_table_segments(line).as_deref() == Some(&segments[..]))
    {
        let end = lines[header + 1..]
            .iter()
            .position(|body| toml_table_segments(body).is_some())
            .map_or(lines.len(), |offset| header + 1 + offset);
        match lines[header + 1..end]
            .iter()
            .position(|line| toml_key(line).as_deref() == Some("approval_mode"))
        {
            Some(offset) => {
                lines[header + 1 + offset] = r#"approval_mode = "approve""#.to_owned();
            }
            None => lines.insert(header + 1, r#"approval_mode = "approve""#.to_owned()),
        }
    } else {
        if lines.last().is_some_and(|line| !line.is_empty()) {
            lines.push(String::new());
        }
        lines.push(format!("# {comment}"));
        let header: Vec<String> = segments.iter().map(|part| toml_key_segment(part)).collect();
        lines.push(format!("[{}]", header.join(".")));
        lines.push(r#"approval_mode = "approve""#.to_owned());
    }

    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Heals a `config.toml` already broken by writing an approval under a sanitised
/// namespace.
///
/// Only tables stamped with [`MCP_TOOL_APPROVAL_COMMENT`] are touched: an approval the
/// user wrote by hand is theirs, however it is spelled. A table whose namespace maps onto
/// exactly one declared server is re-pointed at the real key, so the approval survives;
/// otherwise the comment, header and body are dropped together.
///
/// `None` when there is nothing to change, so a caller never rewrites a healthy file.
pub fn repairing_orphaned_mcp_tool_tables(contents: &str) -> Option<String> {
    let mut lines: Vec<String> = contents.lines().map(str::to_owned).collect();
    let declared = declared_mcp_server_ids(contents);
    let mut changed = false;
    let mut index = 0;

    while index < lines.len() {
        let is_orphan = toml_table_segments(&lines[index])
            .filter(|segments| {
                segments.len() == 4
                    && segments[0] == "mcp_servers"
                    && segments[2] == "tools"
                    && index > 0
                    && lines[index - 1].trim() == MCP_TOOL_APPROVAL_COMMENT
                    && !config_declares_mcp_server_transport(contents, &segments[1])
            })
            .map(|segments| (segments[1].clone(), segments[3].clone()));
        let Some((namespace, tool)) = is_orphan else {
            index += 1;
            continue;
        };

        let candidates: Vec<&String> = declared
            .iter()
            .filter(|id| sanitized_tool_namespace(id) == namespace)
            .collect();
        if candidates.len() == 1 {
            lines[index] = format!(
                "[{}]",
                [
                    "mcp_servers".to_owned(),
                    candidates[0].clone(),
                    "tools".to_owned(),
                    tool,
                ]
                .iter()
                .map(|part| toml_key_segment(part))
                .collect::<Vec<_>>()
                .join(".")
            );
            changed = true;
            index += 1;
            continue;
        }

        // Nothing to re-point it at: drop the comment, the header and the body.
        let mut end = index + 1;
        while end < lines.len() && toml_table_segments(&lines[end]).is_none() {
            end += 1;
        }
        lines.drain((index - 1)..end);
        index -= 1;
        changed = true;
    }

    if !changed {
        return None;
    }

    // Collapse the blank runs a removal can leave behind, without touching spacing the
    // user wrote elsewhere.
    let mut collapsed: Vec<String> = Vec::new();
    for line in lines {
        if line.trim().is_empty() && collapsed.last().is_some_and(|last| last.trim().is_empty()) {
            continue;
        }
        collapsed.push(line);
    }
    while collapsed.last().is_some_and(|line| line.trim().is_empty()) {
        collapsed.pop();
    }
    let mut text = collapsed.join("\n");
    text.push('\n');
    Some(text)
}

/// The rule block "Always" appends to `rules/codeisland.rules`.
pub fn rule_block(pattern: &[String]) -> String {
    let quoted: Vec<String> = pattern
        .iter()
        .map(|token| quoted_rule_string(token))
        .collect();
    format!(
        "{PREFIX_RULE_COMMENT}\nprefix_rule(\n    pattern = [{}],\n    decision = \"allow\",\n    justification = \"Allowed from CodeIsland Always Allow\",\n)\n\n",
        quoted.join(", ")
    )
}

/// The line a rules file is searched for before appending a duplicate.
pub fn pattern_line(pattern: &[String]) -> String {
    let quoted: Vec<String> = pattern
        .iter()
        .map(|token| quoted_rule_string(token))
        .collect();
    format!("pattern = [{}]", quoted.join(", "))
}

fn quoted_rule_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_command_rule_is_its_first_three_words() {
        assert_eq!(
            shell_prefix("ls -la /tmp", 3),
            Some(vec!["ls".to_owned(), "-la".to_owned(), "/tmp".to_owned()])
        );
        assert_eq!(
            shell_prefix("git commit -m 'a message here'", 3),
            Some(vec!["git".to_owned(), "commit".to_owned(), "-m".to_owned()])
        );
        // Short commands keep everything they have.
        assert_eq!(shell_prefix("ls", 3), Some(vec!["ls".to_owned()]));
    }

    #[test]
    fn a_pipeline_or_a_subshell_is_where_the_prefix_stops() {
        assert_eq!(
            shell_prefix("cat file | grep x", 3),
            Some(vec!["cat".to_owned(), "file".to_owned()])
        );
        assert_eq!(
            shell_prefix("echo $(date)", 3),
            Some(vec!["echo".to_owned()])
        );
        assert_eq!(
            shell_prefix("make build; rm -rf /", 3),
            Some(vec!["make".to_owned(), "build".to_owned()])
        );
        assert_eq!(
            shell_prefix("curl x > out.txt", 3),
            Some(vec!["curl".to_owned(), "x".to_owned()])
        );
    }

    #[test]
    fn an_environment_assignment_is_not_a_rule() {
        assert_eq!(shell_prefix("FOO=1 make test", 3), None);
        assert_eq!(shell_prefix("   ", 3), None);
        assert_eq!(shell_prefix("", 3), None);
        // A value containing `=` after the command word is still a command.
        assert_eq!(
            shell_prefix("make FOO=1", 3),
            Some(vec!["make".to_owned(), "FOO=1".to_owned()])
        );
    }

    #[test]
    fn quotes_are_stripped_and_escapes_are_followed() {
        assert_eq!(
            shell_prefix(r#"echo "hello world" done"#, 3),
            Some(vec![
                "echo".to_owned(),
                "hello world".to_owned(),
                "done".to_owned()
            ])
        );
        assert_eq!(
            shell_prefix(r"echo a\ b", 3),
            Some(vec!["echo".to_owned(), "a b".to_owned()])
        );
    }

    #[test]
    fn only_a_bash_command_gets_a_prefix_rule() {
        assert!(prefix_pattern(Some("Bash"), Some("ls -la")).is_some());
        assert!(prefix_pattern(Some("apply_patch"), Some("ls -la")).is_none());
        assert!(prefix_pattern(Some("Bash"), None).is_none());
    }

    #[test]
    fn an_mcp_tool_name_splits_into_server_and_tool() {
        assert_eq!(
            mcp_tool_approval_target(true, Some("mcp__github__get_me")),
            Some(("github".to_owned(), "get_me".to_owned()))
        );
        assert_eq!(
            mcp_tool_approval_target(true, Some("mcp__davinci_resolve__project_manager")),
            Some(("davinci_resolve".to_owned(), "project_manager".to_owned()))
        );
        assert_eq!(mcp_tool_approval_target(true, Some("mcp__github")), None);
        assert_eq!(mcp_tool_approval_target(true, Some("mcp____tool")), None);
        assert_eq!(mcp_tool_approval_target(true, Some("Bash")), None);
        // Not Codex: nothing of Codex's is written.
        assert_eq!(
            mcp_tool_approval_target(false, Some("mcp__github__get_me")),
            None
        );
    }

    #[test]
    fn always_saves_a_rule_only_when_a_rule_can_describe_it() {
        assert!(always_saves_rule(true, false, Some("Bash"), Some("ls -la")));
        assert!(always_saves_rule(
            true,
            false,
            Some("mcp__github__get_me"),
            None
        ));
        // A remote request holds for its session: its host's config is not ours.
        assert!(!always_saves_rule(true, true, Some("Bash"), Some("ls -la")));
        // Nothing a rule could save.
        assert!(!always_saves_rule(true, false, Some("apply_patch"), None));
        assert!(!always_saves_rule(
            true,
            false,
            Some("Bash"),
            Some("FOO=1 make")
        ));
        assert!(!always_saves_rule(
            false,
            false,
            Some("Bash"),
            Some("ls -la")
        ));
    }

    #[test]
    fn the_reviewer_is_read_from_any_of_its_three_spellings() {
        assert_eq!(
            event_reviewer(&json!({"approvals_reviewer": "auto_review"})),
            Some("auto_review".to_owned())
        );
        assert_eq!(
            event_reviewer(&json!({"approvalsReviewer": " guardian_subagent "})),
            Some("guardian_subagent".to_owned())
        );
        assert_eq!(
            event_reviewer(&json!({"_approvals_reviewer": "human"})),
            Some("human".to_owned())
        );
        assert_eq!(event_reviewer(&json!({"approvals_reviewer": "   "})), None);
        assert_eq!(event_reviewer(&json!({})), None);
    }

    #[test]
    fn only_two_reviewer_names_mean_codex_decides_itself() {
        assert!(is_auto_review_reviewer("auto_review"));
        assert!(is_auto_review_reviewer("  GUARDIAN_SUBAGENT "));
        assert!(is_auto_review_reviewer("\"auto_review\""));
        assert!(!is_auto_review_reviewer("human"));
        assert!(!is_auto_review_reviewer("auto"));
        assert!(!is_auto_review_reviewer(""));
    }

    #[test]
    fn a_request_that_names_its_reviewer_is_answered_by_that_name() {
        assert_eq!(
            verdict_from_request(true, false, &json!({"approvals_reviewer": "auto_review"})),
            Some(Verdict::Defer)
        );
        assert_eq!(
            verdict_from_request(true, false, &json!({"approvals_reviewer": "human"})),
            Some(Verdict::Ask)
        );
        // A remote request with no reviewer is a card: nothing here can vouch for it.
        assert_eq!(
            verdict_from_request(true, true, &json!({})),
            Some(Verdict::Ask)
        );
        // No reviewer named, local: the rollout and the config still get a say.
        assert_eq!(verdict_from_request(true, false, &json!({})), None);
        assert_eq!(verdict_from_request(false, false, &json!({})), None);
    }

    #[test]
    fn a_profiles_reviewer_beats_the_top_level_one() {
        let config = r#"
approvals_reviewer = "human"
profile = "work"

[profiles.work]
approvals_reviewer = "auto_review"
"#;
        assert!(config_enables_auto_review(config));

        let human_profile = r#"
approvals_reviewer = "auto_review"
profile = "careful"

[profiles.careful]
approvals_reviewer = "human"
"#;
        assert!(!config_enables_auto_review(human_profile));
    }

    #[test]
    fn the_top_level_reviewer_is_used_when_no_profile_is_selected() {
        assert!(config_enables_auto_review(
            "approvals_reviewer = \"auto_review\"\n"
        ));
        assert!(!config_enables_auto_review(
            "approvals_reviewer = \"human\"\n"
        ));
        assert!(!config_enables_auto_review(""));
        // A quoted key is not the key Codex reads.
        assert!(!config_enables_auto_review(
            "\"approvals_reviewer\" = \"auto_review\"\n"
        ));
        // A commented-out line decides nothing.
        assert!(!config_enables_auto_review(
            "# approvals_reviewer = \"auto_review\"\n"
        ));
        // A `#` inside a value is not a comment.
        assert!(!config_enables_auto_review(
            "approvals_reviewer = \"hu#man\"\n"
        ));
    }

    #[test]
    fn a_reviewer_under_an_unselected_profile_decides_nothing() {
        let config = r#"
profile = "quiet"

[profiles.other]
approvals_reviewer = "auto_review"
"#;
        assert!(!config_enables_auto_review(config));
    }

    #[test]
    fn an_exact_server_key_wins_over_a_sanitised_match() {
        let contents = "[mcp_servers.davinci_resolve]\ncommand = \"exact\"\n\n[mcp_servers.davinci-resolve]\ncommand = \"hyphenated\"\n";
        assert_eq!(
            declared_mcp_server_id(contents, "davinci_resolve").as_deref(),
            Some("davinci_resolve")
        );
    }

    #[test]
    fn two_servers_that_sanitise_alike_are_a_refusal() {
        let contents = "[mcp_servers.davinci-resolve]\ncommand = \"one\"\n\n[mcp_servers.\"davinci resolve\"]\ncommand = \"two\"\n";
        assert_eq!(declared_mcp_server_id(contents, "davinci_resolve"), None);
    }

    #[test]
    fn a_transportless_table_is_not_a_server() {
        let contents =
            "[mcp_servers.codex_app.tools.send_message_to_thread]\napproval_mode = \"approve\"\n";
        assert_eq!(declared_mcp_server_id(contents, "codex_app"), None);
        assert_eq!(declared_mcp_server_ids(contents), Vec::<String>::new());
    }

    #[test]
    fn an_approval_is_written_under_the_declared_server_key() {
        let contents = "[mcp_servers.davinci-resolve]\ncommand = \"/opt/venv/bin/python\"\n";
        let server = declared_mcp_server_id(contents, "davinci_resolve").expect("a server");
        let updated = config_with_mcp_tool_approval(
            contents,
            &["mcp_servers", &server, "tools", "project_manager"],
            &MCP_TOOL_APPROVAL_COMMENT[2..],
        );
        assert!(
            updated.contains("[mcp_servers.davinci-resolve.tools.project_manager]"),
            "{updated}"
        );
        assert!(
            !updated.contains("[mcp_servers.davinci_resolve"),
            "{updated}"
        );
        assert!(updated.contains(r#"approval_mode = "approve""#));
    }

    #[test]
    fn writing_the_same_approval_twice_does_not_add_a_second_table() {
        let contents = "[mcp_servers.github]\ncommand = \"gh\"\n";
        let once = config_with_mcp_tool_approval(
            contents,
            &["mcp_servers", "github", "tools", "get_me"],
            "Added by CodeIsland",
        );
        let twice = config_with_mcp_tool_approval(
            &once,
            &["mcp_servers", "github", "tools", "get_me"],
            "Added by CodeIsland",
        );
        assert_eq!(once, twice);
        assert_eq!(twice.matches("approval_mode").count(), 1);
    }

    #[test]
    fn an_existing_approval_mode_is_replaced_not_duplicated() {
        let contents = "[mcp_servers.github.tools.get_me]\napproval_mode = \"prompt\"\n";
        let updated = config_with_mcp_tool_approval(
            contents,
            &["mcp_servers", "github", "tools", "get_me"],
            "x",
        );
        assert_eq!(updated.matches("approval_mode").count(), 1);
        assert!(updated.contains(r#"approval_mode = "approve""#));
    }

    #[test]
    fn the_repair_repoints_an_orphaned_approval_at_the_declared_server() {
        let contents = format!(
            "[mcp_servers.davinci-resolve]\ncommand = \"/opt/venv/bin/python\"\n\n{MCP_TOOL_APPROVAL_COMMENT}\n[mcp_servers.davinci_resolve.tools.project_manager]\napproval_mode = \"approve\"\n"
        );
        let repaired = repairing_orphaned_mcp_tool_tables(&contents).expect("repaired");
        assert!(
            repaired.contains("[mcp_servers.davinci-resolve.tools.project_manager]"),
            "{repaired}"
        );
        assert!(
            !repaired.contains("[mcp_servers.davinci_resolve"),
            "{repaired}"
        );
        assert!(
            repaired.contains(r#"approval_mode = "approve""#),
            "the approval survives"
        );
        assert!(repaired.contains(r#"command = "/opt/venv/bin/python""#));
    }

    #[test]
    fn the_repair_drops_an_approval_with_no_server_to_repoint_at() {
        let contents = format!(
            "[mcp_servers.github]\ncommand = \"github-mcp-server\"\n\n{MCP_TOOL_APPROVAL_COMMENT}\n[mcp_servers.codex_app.tools.send_message_to_thread]\napproval_mode = \"approve\"\n"
        );
        let repaired = repairing_orphaned_mcp_tool_tables(&contents).expect("repaired");
        assert!(!repaired.contains("codex_app"), "{repaired}");
        assert!(
            !repaired.contains(MCP_TOOL_APPROVAL_COMMENT),
            "the stamp goes with its table"
        );
        assert!(repaired.contains("[mcp_servers.github]"));
        assert!(repaired.contains(r#"command = "github-mcp-server""#));
    }

    #[test]
    fn the_repair_leaves_an_unstamped_table_and_a_healthy_file_alone() {
        // An approval the user typed is theirs, however it is spelled.
        assert_eq!(
            repairing_orphaned_mcp_tool_tables(
                "[mcp_servers.codex_app.tools.send_message_to_thread]\napproval_mode = \"approve\"\n"
            ),
            None
        );
        let healthy = format!(
            "[mcp_servers.github]\ncommand = \"gh\"\n\n{MCP_TOOL_APPROVAL_COMMENT}\n[mcp_servers.github.tools.get_me]\napproval_mode = \"approve\"\n"
        );
        assert_eq!(repairing_orphaned_mcp_tool_tables(&healthy), None);
    }

    #[test]
    fn a_table_header_with_quoted_segments_keeps_its_key_whole() {
        assert_eq!(
            toml_table_segments(r#"[mcp_servers."davinci resolve"]"#),
            Some(vec!["mcp_servers".to_owned(), "davinci resolve".to_owned()])
        );
        assert_eq!(toml_key_segment("davinci resolve"), "\"davinci resolve\"");
        assert_eq!(toml_key_segment("davinci-resolve"), "davinci-resolve");
        assert!(toml_table_segments("[[array]]").is_none());
        assert!(toml_table_segments("not a header").is_none());
    }

    #[test]
    fn the_rule_block_quotes_what_it_writes() {
        let block = rule_block(&["echo".to_owned(), "a\"b".to_owned()]);
        assert!(block.starts_with(PREFIX_RULE_COMMENT));
        assert!(block.contains(r#"pattern = ["echo", "a\"b"]"#), "{block}");
        assert!(block.contains(r#"decision = "allow""#));
        assert_eq!(
            pattern_line(&["git".to_owned(), "commit".to_owned()]),
            r#"pattern = ["git", "commit"]"#
        );
    }
}
