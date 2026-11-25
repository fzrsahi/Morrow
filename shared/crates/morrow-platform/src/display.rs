//! Connected-screen enumeration for the per-display Settings rows (#465).
//!
//! The process stays here; the parsing and the migration live in `morrow-core::display`.
//! A tool that is missing or fails yields no displays rather than a fabricated
//! single screen — the Settings page then says so instead of offering a row
//! that cannot be applied.

use morrow_core::display::DisplayDescriptor;

use crate::runner::Runner;

/// `xrandr --query` on Linux, Win32 screens through PowerShell on Windows. The
/// PowerShell shape is `name|WxH|primary|builtin`, one screen per line.
pub fn command() -> (&'static str, Vec<&'static str>) {
    #[cfg(windows)]
    {
        (
            "powershell.exe",
            vec![
                "-NoProfile",
                "-Command",
                "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Screen]::AllScreens | ForEach-Object { \"$($_.DeviceName)|$($_.Bounds.Width)x$($_.Bounds.Height)|$($_.Primary)|$($_.DeviceName -match 'Integrated|Internal')\" }",
            ],
        )
    }
    #[cfg(not(windows))]
    {
        ("xrandr", vec!["--query"])
    }
}

/// Parse the output of [`command`] with the core's pure readers.
pub fn parse_output(output: &str) -> Vec<DisplayDescriptor> {
    #[cfg(windows)]
    {
        morrow_core::display::parse_powershell_screens(output)
    }
    #[cfg(not(windows))]
    {
        morrow_core::display::parse_xrandr(output)
    }
}

/// The connected screens in OS enumeration order. An empty result is a reader
/// the host cannot serve, not "one display at 0×0".
pub fn connected_displays(runner: &dyn Runner) -> Vec<DisplayDescriptor> {
    let (program, args) = command();
    match runner.run(program, &args) {
        Ok(output) => parse_output(&output),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeRunner {
        output: Result<String, String>,
    }

    impl Runner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            assert_eq!(program, command().0);
            assert_eq!(args, command().1);
            self.output.clone()
        }
    }

    #[test]
    fn a_failed_reader_returns_no_displays() {
        let runner = FakeRunner { output: Err("missing".to_owned()) };
        assert!(connected_displays(&runner).is_empty());
    }

    #[test]
    fn a_good_reader_is_parsed_by_the_core() {
        let output = if cfg!(windows) {
            "DISPLAY1|1920x1080|True|False\n".to_owned()
        } else {
            "eDP-1 connected primary 1920x1080+0+0\n".to_owned()
        };
        let runner = FakeRunner { output: Ok(output) };
        let displays = connected_displays(&runner);
        assert_eq!(displays.len(), 1);
        assert!(displays[0].is_primary);
    }
}
