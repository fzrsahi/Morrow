//! `morrow-app --hooks …`: the installer without the GUI (#243).
//!
//! The Settings window is the user's way in, and it stays that way. A gate
//! script has no way to click it, and the plan's target module for this work is
//! a `morrow-hooks` binary (`docs/plan-windows-linux.md` §5.1), with
//! `Morrow.Hooks.Cli` as the macOS prior art. This is the smallest honest
//! version of that: the same `morrow_platform` calls the Settings card makes,
//! printing one machine-readable line per source so a gate can read it.
//!
//! Deliberately not a second implementation: every branch below delegates.

use std::process::ExitCode;


#[cfg(unix)]
use morrow_platform::hooks;
use morrow_platform::hooks::Report;
use morrow_platform::kimi;
use morrow_platform::opencode;
use morrow_platform::yaml_hooks;

use crate::HooksSite;

const USAGE: &str = "\
usage: morrow-app --hooks <command>

  status                 one line per source: <source>: <installed|absent>
  install <source>       install or repair one source's hook
  uninstall <source>     take one source's hook back out
  install-all            every source detected on this machine
  uninstall-all          every source with a hook of ours

The sources are the ones Settings lists; `status` names them all.";

/// Runs the command and returns the process exit code, or `None` when the
/// arguments are not ours (the GUI should start).
pub fn run(args: &[String]) -> Option<ExitCode> {
    let mut args = args.iter();
    let _binary = args.next();
    if args.next().map(String::as_str) != Some("--hooks") {
        return None;
    }
    let command = args.next().map_or("", String::as_str);
    let source = args.next();
    let site = crate::hooks::site();
    let result = match (command, source.map(String::as_str)) {
        ("status", None) => {
            status(&site);
            Ok(())
        }
        ("install", Some(source)) => install(&site, source),
        ("uninstall", Some(source)) => uninstall(&site, source),
        ("install-all", None) => install_all(&site, true),
        ("uninstall-all", None) => install_all(&site, false),
        _ => {
            eprintln!("{USAGE}");
            return Some(ExitCode::FAILURE);
        }
    };
    match result {
        Ok(()) => Some(ExitCode::SUCCESS),
        Err(error) => {
            eprintln!("morrow-app --hooks {command}: {error}");
            Some(ExitCode::FAILURE)
        }
    }
}

fn status(site: &HooksSite) {
    #[cfg(unix)]
    {
        let hooks = hooks::status(&site.paths, None);
        println!(
            "claude: {}",
            if hooks.hooks_installed {
                "installed"
            } else {
                "absent"
            }
        );
        for target in hooks::targets_status(&site.paths) {
            println!(
                "{}: {}",
                target.source,
                match (target.detected, target.installed) {
                    (false, _) => "not-detected",
                    (true, true) => "installed",
                    (true, false) => "absent",
                }
            );
        }
    }
    #[cfg(windows)]
    {
        println!("claude: {}", windows_state(site));
        // The other CLIs' files are `<home>/<target.path>` here too, so the table
        // is the same one Linux walks.
        for target in hooks::targets_status(&site.paths) {
            println!(
                "{}: {}",
                target.source,
                match (target.detected, target.installed) {
                    (false, _) => "not-detected",
                    (true, true) => "installed",
                    (true, false) => "absent",
                }
            );
        }
        let _ = site;
        // The Windows build hooks [CC] only; the other eight arrive with their
        // Windows paths (#99's follow-up), which is what Settings says too.
        println!("claude: {}", windows_state(site));
    }
    let opencode = opencode::status(&site.opencode);
    println!(
        "opencode: {}",
        if opencode.plugin_installed {
            "installed"
        } else if opencode.detected {
            "absent"
        } else {
            "not-detected"
        }
    );
    let kimi = kimi::status(&site.kimi);
    println!(
        "kimi: {}",
        if kimi.installed {
            "installed"
        } else if kimi.detected {
            "absent"
        } else {
            "not-detected"
        }
    );
    for yaml in site.yaml.iter().map(yaml_hooks::status) {
        println!(
            "{}: {}",
            yaml.source,
            if yaml.installed {
                "installed"
            } else {
                "absent"
            }
        );
    }
}

#[cfg(windows)]
fn windows_state(site: &HooksSite) -> &'static str {
    let paths = crate::hooks::windows_paths(site);
    if morrow_platform::hooks_windows::status(&paths, None).hooks_installed {
        "installed"
    } else {
        "absent"
    }
}

/// One source, by the name `status` prints.
fn install(site: &HooksSite, source: &str) -> Result<(), String> {
    match source {
        "claude" => crate::hooks::install_claude(site).map(|report| print_report(source, &report)),
        "opencode" => simple(source, crate::hooks::install_opencode(site)),
        "kimi" => simple(source, crate::hooks::install_kimi(site)),
        _ if is_yaml(site, source) => simple(source, crate::hooks::install_yaml(site, source)),
        _ => target(site, source, true),
    }
}

fn uninstall(site: &HooksSite, source: &str) -> Result<(), String> {
    match source {
        "claude" => {
            crate::hooks::uninstall_claude(site).map(|report| print_report(source, &report))
        }
        "opencode" => simple(source, crate::hooks::uninstall_opencode(site)),
        "kimi" => simple(source, crate::hooks::uninstall_kimi(site)),
        _ if is_yaml(site, source) => simple(source, crate::hooks::uninstall_yaml(site, source)),
        _ => target(site, source, false),
    }
}

fn is_yaml(site: &HooksSite, source: &str) -> bool {
    site.yaml.iter().any(|entry| entry.source.source == source)
}

/// The sources whose installer reports a `bool` rather than a `Report`.
fn simple(source: &str, outcome: Result<bool, String>) -> Result<(), String> {
    outcome.map(|changed| {
        println!(
            "{source}: changed={}",
            if changed { "hook" } else { "nothing" }
        );
    })
}

/// One of the eight other CLIs, by the core's table.
#[cfg(unix)]
fn target(site: &HooksSite, source: &str, install: bool) -> Result<(), String> {
    let report = if install {
        hooks::install_target(&site.paths, source, &crate::hooks::stamp())
    } else {
        hooks::uninstall_target(&site.paths, source)
    };
    report
        .map(|report| match report {
            // The CLI is not on this machine: nothing to hook, not a failure.
            None => println!("{source}: not-detected"),
            Some(report) => print_report(source, &report),
        })
        .map_err(|error| error.to_string())
}

#[cfg(windows)]
fn target(site: &HooksSite, source: &str, install: bool) -> Result<(), String> {
    let report = if install {
        // `--hooks install` for a fork must leave the `.cmd` behind too, or the
        // command it writes names a file that is not there.
        let paths = crate::hooks::windows_paths(site);
        morrow_platform::hooks_windows::ensure_script(&paths).map_err(|error| error.to_string())?;
        hooks::install_target_with(
            &site.paths,
            source,
            &crate::hooks::stamp(),
            &morrow_platform::hooks_windows::command(&paths),
        )
    } else {
        hooks::uninstall_target(&site.paths, source)
    };
    report
        .map(|report| match report {
            None => println!("{source}: not-detected"),
            Some(report) => print_report(source, &report),
        })
        .map_err(|error| error.to_string())
}

/// Every source this machine has, in table order, so a caller does not have to
/// know the list.
fn install_all(site: &HooksSite, install: bool) -> Result<(), String> {
    let sources: Vec<String> = status_sources(site)
        .into_iter()
        .filter(|(_, state)| *state != "not-detected")
        .map(|(source, _)| source)
        .collect();
    let mut failures = Vec::new();
    for source in sources {
        let outcome = if install {
            self::install(site, &source)
        } else {
            uninstall(site, &source)
        };
        if let Err(error) = outcome {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

/// `status` as data, so `install-all` walks the same list the user reads.
fn status_sources(site: &HooksSite) -> Vec<(String, &'static str)> {
    let mut sources = Vec::new();
    #[cfg(unix)]
    {
        let hooks = hooks::status(&site.paths, None);
        sources.push((
            "claude".to_owned(),
            if hooks.hooks_installed {
                "installed"
            } else {
                "absent"
            },
        ));
        for target in hooks::targets_status(&site.paths) {
            sources.push((
                target.source.to_owned(),
                match (target.detected, target.installed) {
                    (false, _) => "not-detected",
                    (true, true) => "installed",
                    (true, false) => "absent",
                },
            ));
        }
    }
    #[cfg(windows)]
    {
        sources.push(("claude".to_owned(), windows_state(site)));
        for target in hooks::targets_status(&site.paths) {
            sources.push((
                target.source.to_owned(),
                match (target.detected, target.installed) {
                    (false, _) => "not-detected",
                    (true, true) => "installed",
                    (true, false) => "absent",
                },
            ));
        }
    }
    let opencode = opencode::status(&site.opencode);
    sources.push((
        "opencode".to_owned(),
        if opencode.plugin_installed {
            "installed"
        } else if opencode.detected {
            "absent"
        } else {
            "not-detected"
        },
    ));
    let kimi = kimi::status(&site.kimi);
    sources.push((
        "kimi".to_owned(),
        if kimi.installed {
            "installed"
        } else if kimi.detected {
            "absent"
        } else {
            "not-detected"
        },
    ));
    for yaml in site.yaml.iter().map(yaml_hooks::status) {
        sources.push((
            yaml.source.to_owned(),
            if yaml.installed {
                "installed"
            } else {
                "absent"
            },
        ));
    }
    sources
}

/// What one install or uninstall changed, on one line.
fn print_report(source: &str, report: &Report) {
    let mut changed = Vec::new();
    if report.script_written {
        changed.push("script");
    }
    if report.bridge_copied {
        changed.push("bridge");
    }
    if report.settings_changed {
        changed.push("settings");
    }
    if report.flag_enabled {
        changed.push("flag");
    }
    let changed = if changed.is_empty() {
        "nothing".to_owned()
    } else {
        changed.join(",")
    };
    match &report.backup {
        Some(backup) => println!("{source}: changed={changed} backup={}", backup.display()),
        None => println!("{source}: changed={changed}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_with(args: &[&str]) -> Option<ExitCode> {
        run(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>())
    }

    /// Anything that is not `--hooks` belongs to the island.
    #[test]
    fn only_hooks_starts_a_command_and_everything_else_starts_the_gui() {
        assert!(run_with(&["morrow-app"]).is_none());
        assert!(run_with(&["morrow-app", "--smoke"]).is_none());
        assert!(run_with(&["morrow-app", "--hooks"]).is_some());
    }

    /// A command it does not know is a failure, not a silent GUI launch: a gate
    /// that typed the wrong word has to go red.
    #[test]
    fn an_unknown_command_fails_rather_than_starting_the_island() {
        assert_eq!(
            run_with(&["morrow-app", "--hooks", "frobnicate"]),
            Some(ExitCode::FAILURE)
        );
        // `install` needs a source; `status` must not take one.
        assert_eq!(
            run_with(&["morrow-app", "--hooks", "install"]),
            Some(ExitCode::FAILURE)
        );
        assert_eq!(
            run_with(&["morrow-app", "--hooks", "status", "claude"]),
            Some(ExitCode::FAILURE)
        );
    }
}
