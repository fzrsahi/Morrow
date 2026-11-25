//! CPU, memory and battery readings for Linux and Windows (#437).
//!
//! Swift's `SystemMonitorManager` samples CPU ticks, physical memory and the
//! power source. The same three numbers come from procfs/sysfs on Linux and
//! from PowerShell CIM on Windows. Parsing is pure and total: a missing or
//! malformed reading is `None`, never a panic or a fabricated number.

use std::path::Path;

use crate::capability::{Capability, Os};
use crate::runner::Runner;

pub const PROC_STAT: &str = "/proc/stat";
pub const PROC_MEMINFO: &str = "/proc/meminfo";
pub const POWER_SUPPLY: &str = "/sys/class/power_supply";
pub const BATTERIES: [&str; 2] = ["BAT0", "BAT1"];
pub const POWERSHELL: &str = "powershell.exe";

/// CPU jiffies since boot, split the way `host_cpu_load_info` splits them:
/// idle time stays out of `busy`, and `iowait` counts as idle on Linux.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuTicks {
    pub busy: u64,
    pub idle: u64,
}

/// Physical memory in KiB, the unit `/proc/meminfo` and CIM both report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Memory {
    pub total_kb: u64,
    pub available_kb: u64,
}

impl Memory {
    pub fn used_kb(&self) -> u64 {
        self.total_kb.saturating_sub(self.available_kb)
    }
}

/// The battery row. `None` means no reading: a desktop has no battery, and
/// Swift's fallback is to act as if it were full and on AC.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Battery {
    pub level: u8,
    pub charging: bool,
    pub ac_connected: bool,
}

/// One sample. An unavailable reading stays `None` instead of becoming zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct SystemStats {
    pub cpu_usage_percent: Option<f64>,
    pub memory: Option<Memory>,
    pub battery: Option<Battery>,
}

/// Parse the aggregate `cpu` line of `/proc/stat`; `cpu0` and other lines are
/// per-core repeats, not the total. Missing trailing fields default to zero.
pub fn parse_cpu_stat(stat: &str) -> Option<CpuTicks> {
    let line = stat
        .lines()
        .find(|line| line.split_whitespace().next() == Some("cpu"))?;
    let mut fields = line
        .split_whitespace()
        .skip(1)
        .map(|field| field.parse::<u64>().ok());
    let mut number = || fields.next().flatten();
    let user = number()?;
    let nice = number()?;
    let system = number()?;
    let idle = number()?;
    let iowait = number().unwrap_or(0);
    let irq = number().unwrap_or(0);
    let softirq = number().unwrap_or(0);
    let steal = number().unwrap_or(0);
    Some(CpuTicks {
        busy: user
            .saturating_add(nice)
            .saturating_add(system)
            .saturating_add(irq)
            .saturating_add(softirq)
            .saturating_add(steal),
        idle: idle.saturating_add(iowait),
    })
}

/// Busy fraction between two samples, in percent. A counter that went
/// backwards (a reboot or a replaced file) has no usable delta.
pub fn cpu_usage(previous: &CpuTicks, current: &CpuTicks) -> Option<f64> {
    let busy = current.busy.checked_sub(previous.busy)?;
    let idle = current.idle.checked_sub(previous.idle)?;
    let total = busy.checked_add(idle)?;
    if total == 0 {
        return None;
    }
    Some(((busy as f64 / total as f64) * 100.0).clamp(0.0, 100.0))
}

fn kilobytes(field: &str) -> Option<u64> {
    field.split_whitespace().next()?.parse::<u64>().ok()
}

/// `MemTotal` and `MemAvailable` are enough for Swift's used/total/percent
/// triple; `used` is the difference rather than a third guess.
pub fn parse_meminfo(text: &str) -> Option<Memory> {
    let mut total = None;
    let mut available = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim() {
            "MemTotal" => total = kilobytes(value),
            "MemAvailable" => available = kilobytes(value),
            _ => {}
        }
    }
    let total = total?;
    Some(Memory {
        total_kb: total,
        available_kb: available?.min(total),
    })
}

/// `/sys/class/power_supply/BAT*/capacity` and `status`. Any other status,
/// including `Full`, means the machine is on AC but not charging right now.
pub fn parse_battery(capacity: Option<&str>, status: Option<&str>) -> Option<Battery> {
    let level = capacity?
        .split_whitespace()
        .next()?
        .parse::<u8>()
        .ok()?
        .min(100);
    let status = status?;
    if status.trim().is_empty() {
        return None;
    }
    Some(Battery {
        level,
        charging: status.trim().eq_ignore_ascii_case("Charging"),
        ac_connected: !status.trim().eq_ignore_ascii_case("Discharging"),
    })
}

/// PowerShell prints one number for `Win32_Processor.LoadPercentage`.
pub fn parse_windows_percent(output: &str) -> Option<f64> {
    let value = output.trim().lines().last()?.trim().parse::<f64>().ok()?;
    ((0.0..=100.0).contains(&value)).then_some(value)
}

/// `TotalVisibleMemorySize,FreePhysicalMemory` from CIM, both in KiB.
pub fn parse_windows_memory(output: &str) -> Option<Memory> {
    let line = output
        .lines()
        .map(str::trim)
        .find(|line| line.contains(','))?;
    let (total, available) = line.split_once(',')?;
    let total = total.trim().parse::<u64>().ok()?;
    let available = available.trim().parse::<u64>().ok()?;
    Some(Memory {
        total_kb: total,
        available_kb: available.min(total),
    })
}

fn windows_battery_command() -> &'static str {
    "$b = Get-CimInstance Win32_Battery | Select-Object -First 1; if ($b) { $charging = $b.BatteryStatus -in 6,7,8,9; $ac = $b.BatteryStatus -in 2,3,6,7,8,9,11; '{0},{1},{2}' -f $b.EstimatedChargeRemaining,$charging,$ac }"
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim() {
        "True" | "true" | "1" => Some(true),
        "False" | "false" | "0" => Some(false),
        _ => None,
    }
}

/// `EstimatedChargeRemaining,charging,ac` from CIM. `Win32_Battery.BatteryStatus`
/// codes 6/7/8/9 mean charging and 2/3/6/7/8/9/11 mean on AC; codes 4/5/10 are
/// discharging low/critical or undefined and do not claim either state.
pub fn parse_windows_battery(output: &str) -> Option<Battery> {
    let line = output
        .lines()
        .map(str::trim)
        .find(|line| line.matches(',').count() >= 2)?;
    let mut fields = line.split(',');
    let level = fields.next()?.trim().parse::<u8>().ok()?.min(100);
    let charging = parse_bool(fields.next()?)?;
    let ac_connected = parse_bool(fields.next()?)?;
    Some(Battery {
        level,
        charging,
        ac_connected,
    })
}

/// Live reader. File reads and commands are injected, so tests supply fixtures
/// for either OS without touching the machine they run on.
pub struct SystemReader {
    read_file: Box<dyn Fn(&Path) -> Option<String>>,
    runner: Box<dyn Runner>,
    previous_cpu: Option<CpuTicks>,
}

impl SystemReader {
    pub fn system() -> Self {
        Self::new(
            |path| std::fs::read_to_string(path).ok(),
            crate::runner::SystemRunner,
        )
    }

    pub fn new(
        read_file: impl Fn(&Path) -> Option<String> + 'static,
        runner: impl Runner + 'static,
    ) -> Self {
        Self {
            read_file: Box::new(read_file),
            runner: Box::new(runner),
            previous_cpu: None,
        }
    }

    pub fn read(&mut self) -> SystemStats {
        self.read_for(if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        })
    }

    /// `read()` picks the host OS; callers that know which OS they model can
    /// name it, as the tests do.
    pub fn read_for(&mut self, os: Os) -> SystemStats {
        match os {
            Os::Linux => self.read_linux(),
            Os::Windows => self.read_windows(),
        }
    }

    fn read_linux(&mut self) -> SystemStats {
        let ticks = (self.read_file)(Path::new(PROC_STAT)).and_then(|text| parse_cpu_stat(&text));
        let cpu_usage_percent = match (self.previous_cpu, ticks) {
            (Some(previous), Some(current)) => cpu_usage(&previous, &current),
            _ => None,
        };
        self.previous_cpu = ticks;
        let memory =
            (self.read_file)(Path::new(PROC_MEMINFO)).and_then(|text| parse_meminfo(&text));
        let battery = BATTERIES.iter().find_map(|name| {
            let dir = Path::new(POWER_SUPPLY).join(name);
            let capacity = (self.read_file)(&dir.join("capacity"))?;
            let status = (self.read_file)(&dir.join("status"))?;
            parse_battery(Some(&capacity), Some(&status))
        });
        SystemStats {
            cpu_usage_percent,
            memory,
            battery,
        }
    }

    fn read_windows(&mut self) -> SystemStats {
        let cpu = self
            .runner
            .run(
                POWERSHELL,
                &[
                    "-NoProfile",
                    "-Command",
                    "(Get-CimInstance Win32_Processor | Select-Object -First 1).LoadPercentage",
                ],
            )
            .ok()
            .and_then(|text| parse_windows_percent(&text));
        let memory = self
            .runner
            .run(
                POWERSHELL,
                &[
                    "-NoProfile",
                    "-Command",
                    "$o = Get-CimInstance Win32_OperatingSystem; '{0},{1}' -f $o.TotalVisibleMemorySize,$o.FreePhysicalMemory",
                ],
            )
            .ok()
            .and_then(|text| parse_windows_memory(&text));
        let battery = self
            .runner
            .run(
                POWERSHELL,
                &[
                    "-NoProfile",
                    "-Command",
                    windows_battery_command(),
                ],
            )
            .ok()
            .and_then(|text| parse_windows_battery(&text));
        SystemStats {
            cpu_usage_percent: cpu,
            memory,
            battery,
        }
    }
}

/// Linux always has procfs, but only a laptop exposes a battery. A desktop is
/// `Degraded`, not `Unsupported`: CPU and memory still work. The closure is
/// injected so a test can model each case.
pub fn linux_capability(exists: impl Fn(&Path) -> bool) -> Capability {
    let cpu = exists(Path::new(PROC_STAT));
    let memory = exists(Path::new(PROC_MEMINFO));
    if !cpu && !memory {
        return Capability::Unsupported(
            "no /proc/stat or /proc/meminfo: CPU and memory cannot be read".into(),
        );
    }
    let battery = BATTERIES
        .iter()
        .any(|name| exists(&Path::new(POWER_SUPPLY).join(name).join("capacity")));
    if cpu && memory {
        if battery {
            Capability::Supported
        } else {
            Capability::Degraded("no battery found: CPU and memory still read from /proc".into())
        }
    } else {
        Capability::Degraded("only one of /proc/stat and /proc/meminfo is available".into())
    }
}

/// PowerShell ships with Windows and CIM answers all three questions; a
/// machine without a battery simply leaves that field empty.
pub fn windows_capability() -> Capability {
    Capability::Supported
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::Path;
    use std::rc::Rc;

    use super::*;

    struct FakeRunner;

    impl Runner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            assert_eq!(program, POWERSHELL);
            let script = args.last().copied().unwrap_or("");
            if script.contains("Win32_Processor") {
                Ok("42\r\n".into())
            } else if script.contains("Win32_OperatingSystem") {
                Ok("16384,4096\r\n".into())
            } else if script.contains("Win32_Battery") {
                Ok("72,True,True\r\n".into())
            } else {
                Err("unexpected command".into())
            }
        }
    }

    #[test]
    fn proc_stat_gives_the_aggregate_ticks_not_a_core() {
        let stat = parse_cpu_stat(
            "cpu  100 10 40 850 20 2 3 0\ncpu0 50 5 20 425 10 1 1 0\nintr 123 0 0\n",
        )
        .unwrap();
        assert_eq!(stat.busy, 155);
        assert_eq!(stat.idle, 870);
        assert_eq!(parse_cpu_stat("cpu\nbroken\n"), None);
        assert_eq!(parse_cpu_stat("cpu0 1 2 3 4\n"), None);
    }

    #[test]
    fn cpu_percent_is_a_delta_and_never_panics_on_a_reset() {
        let first = CpuTicks {
            busy: 150,
            idle: 850,
        };
        let second = CpuTicks {
            busy: 190,
            idle: 910,
        };
        assert_eq!(cpu_usage(&first, &second), Some(40.0));
        assert_eq!(cpu_usage(&first, &first), None);
        assert_eq!(cpu_usage(&second, &first), None);
    }

    #[test]
    fn meminfo_reads_total_and_available() {
        let memory = parse_meminfo(
            "MemTotal:       16384 kB\nMemFree:         2048 kB\nMemAvailable:    4096 kB\n",
        )
        .unwrap();
        assert_eq!(memory.total_kb, 16384);
        assert_eq!(memory.used_kb(), 12288);
        assert_eq!(parse_meminfo("MemTotal: 16384 kB\n"), None);
        assert_eq!(
            parse_meminfo("MemTotal: many kB\nMemAvailable: 12 kB\n"),
            None
        );
    }

    #[test]
    fn battery_status_decides_charging_and_ac() {
        assert_eq!(
            parse_battery(Some(" 72 \n"), Some("Charging\n")),
            Some(Battery {
                level: 72,
                charging: true,
                ac_connected: true
            })
        );
        assert_eq!(
            parse_battery(Some("40\n"), Some("Discharging\n")),
            Some(Battery {
                level: 40,
                charging: false,
                ac_connected: false
            })
        );
        assert_eq!(
            parse_battery(Some("100\n"), Some("Full\n")),
            Some(Battery {
                level: 100,
                charging: false,
                ac_connected: true
            })
        );
        assert_eq!(parse_battery(Some("oops\n"), Some("Full\n")), None);
        assert_eq!(parse_battery(Some("50\n"), None), None);
    }

    #[test]
    fn windows_cim_output_is_parsed_without_a_windows_host() {
        assert_eq!(parse_windows_percent("42\r\n"), Some(42.0));
        assert_eq!(parse_windows_percent("150\n"), None);
        assert_eq!(parse_windows_percent("NaN\n"), None);
        let memory = parse_windows_memory("16384,4096\r\n").unwrap();
        assert_eq!(memory.used_kb(), 12288);
        assert_eq!(parse_windows_memory("16384,not-a-number\n"), None);
        assert_eq!(
            parse_windows_battery("72,True,True\r\n"),
            Some(Battery {
                level: 72,
                charging: true,
                ac_connected: true
            })
        );
        assert_eq!(parse_windows_battery("72,True\n"), None);
        assert_eq!(parse_windows_battery(""), None);
    }

    #[test]
    fn windows_battery_command_maps_charging_and_ac_codes() {
        let command = windows_battery_command();
        assert!(command.contains("-in 6,7,8,9"), "status 6 is Charging");
        assert!(command.contains("-in 2,3,6,7,8,9,11"), "status 2 is AC");
    }

    #[test]
    fn windows_battery_parser_reads_status_6_and_2_output() {
        // Win32_Battery status 6 (Charging) emits "level,True,True".
        assert_eq!(
            parse_windows_battery("55,True,True\r\n"),
            Some(Battery {
                level: 55,
                charging: true,
                ac_connected: true
            })
        );
        // Win32_Battery status 2 (AC power, no discharge) emits "level,False,True".
        assert_eq!(
            parse_windows_battery("100,False,True\r\n"),
            Some(Battery {
                level: 100,
                charging: false,
                ac_connected: true
            })
        );
    }

    #[test]
    fn linux_reader_needs_two_samples_for_cpu_but_not_the_rest() {
        let stat = Rc::new(Cell::new("cpu  100 0 50 850 0 0 0 0\ncpu0 1 2 3 4\n"));
        let source = stat.clone();
        let mut reader = SystemReader::new(
            move |path| {
                if path == Path::new(PROC_STAT) {
                    Some(source.get().to_owned())
                } else if path == Path::new(PROC_MEMINFO) {
                    Some("MemTotal: 16384 kB\nMemAvailable: 4096 kB\n".into())
                } else if path.to_string_lossy().ends_with("BAT0/capacity") {
                    Some("80\n".into())
                } else if path.to_string_lossy().ends_with("BAT0/status") {
                    Some("Discharging\n".into())
                } else {
                    None
                }
            },
            FakeRunner,
        );
        let first = reader.read_for(Os::Linux);
        assert_eq!(first.cpu_usage_percent, None);
        assert_eq!(first.memory.unwrap().used_kb(), 12288);
        assert_eq!(
            first.battery,
            Some(Battery {
                level: 80,
                charging: false,
                ac_connected: false
            })
        );

        stat.set("cpu  130 0 60 910 0 0 0 0\n");
        let second = reader.read_for(Os::Linux);
        assert_eq!(second.cpu_usage_percent, Some(40.0));
    }

    #[test]
    fn windows_reader_uses_the_runner_not_the_file_source() {
        let mut reader = SystemReader::new(|_| None, FakeRunner);
        let stats = reader.read_for(Os::Windows);
        assert_eq!(stats.cpu_usage_percent, Some(42.0));
        assert_eq!(stats.memory.unwrap().total_kb, 16384);
        assert!(stats.battery.unwrap().charging);
    }

    #[test]
    fn capability_follows_the_files_a_desktop_does_not_have() {
        let proc = |path: &Path| path == Path::new(PROC_STAT) || path == Path::new(PROC_MEMINFO);
        assert!(matches!(linux_capability(&proc), Capability::Degraded(_)));
        let with_battery =
            |path: &Path| proc(path) || path.to_string_lossy().ends_with("BAT0/capacity");
        assert_eq!(linux_capability(&with_battery), Capability::Supported);
        assert!(matches!(
            linux_capability(|_| false),
            Capability::Unsupported(_)
        ));
        assert!(matches!(
            linux_capability(|path| path == Path::new(PROC_STAT)),
            Capability::Degraded(_)
        ));
        assert_eq!(windows_capability(), Capability::Supported);
    }
}
