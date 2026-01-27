use std::ffi::{OsStr, OsString};
use std::process::Command;

/// Outputs debug information for early logging before Tauri logger initialization.
///
/// In debug builds, uses eprintln for console output.
/// In Windows release builds (windows_subsystem = "windows"), uses OutputDebugStringW
/// which can be viewed with tools like DebugView or Visual Studio debugger.
#[cfg(target_os = "windows")]
fn debug_output(msg: &str) {
    #[cfg(debug_assertions)]
    {
        eprintln!("{}", msg);
    }

    #[cfg(not(debug_assertions))]
    {
        use std::ffi::OsStr as WinOsStr;
        use std::os::windows::ffi::OsStrExt;
        extern "system" {
            fn OutputDebugStringW(lp_output_string: *const u16);
        }

        let wide: Vec<u16> = WinOsStr::new(msg)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            OutputDebugStringW(wide.as_ptr());
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn debug_output(msg: &str) {
    eprintln!("{}", msg);
}

/// Represents a plan to launch the actual game from wrapper mode.
#[derive(Debug, PartialEq)]
pub struct WrapperLaunchPlan {
    pub program: OsString,
    pub args: Vec<OsString>,
}

/// Validates if the given path looks like an executable.
///
/// On Windows, checks if the path ends with .exe (case-insensitive).
/// On other platforms, accepts any non-empty path.
fn is_executable_path(path: &OsStr) -> bool {
    #[cfg(target_os = "windows")]
    {
        let path_str = path.to_string_lossy().to_lowercase();
        path_str.ends_with(".exe")
    }

    #[cfg(not(target_os = "windows"))]
    {
        !path.is_empty()
    }
}

/// Parses the command-line arguments to determine if wrapper mode is active.
///
/// Wrapper mode is detected when Steam passes:
/// Index 0: Path to the manager executable
/// Index 1: Path to the actual game executable (must look like an executable)
/// Index 2+: Additional game arguments
///
/// Returns Some(WrapperLaunchPlan) if wrapper mode is detected, None otherwise.
pub fn parse_wrapper_args_os<I>(args: I) -> Option<WrapperLaunchPlan>
where
    I: IntoIterator<Item = OsString>,
{
    let args_vec: Vec<OsString> = args.into_iter().collect();

    // Need at least 2 args: [manager_path, game_path]
    if args_vec.len() < 2 {
        return None;
    }

    // Validate that arg[1] looks like an executable path
    if !is_executable_path(&args_vec[1]) {
        return None;
    }

    // Skip index 0 (manager path), take index 1 as program
    let program = args_vec[1].clone();

    // Index 2+ are forwarded args
    let forwarded_args = args_vec[2..].to_vec();

    Some(WrapperLaunchPlan {
        program,
        args: forwarded_args,
    })
}

/// Executes the wrapper launch plan by spawning the game process.
///
/// This function spawns the game process and returns immediately without waiting.
/// Uses debug_output for logging since the Tauri logger plugin is not yet initialized.
pub fn launch_wrapper_plan(plan: &WrapperLaunchPlan) -> Result<(), String> {
    debug_output(&format!(
        "Wrapper mode: Launching game: {:?} with {} args",
        plan.program,
        plan.args.len()
    ));

    let mut cmd = Command::new(&plan.program);
    cmd.args(&plan.args);

    cmd.spawn()
        .map_err(|e| format!("Failed to spawn game process: {}", e))?;

    debug_output("Game process spawned successfully");
    Ok(())
}

/// Checks command-line arguments and launches game if in wrapper mode.
///
/// Returns true if wrapper mode was detected and game was launched.
/// Returns false if not in wrapper mode (normal manager startup).
///
/// Note: Uses debug_output for logging since the Tauri logger plugin is not yet initialized.
/// In Windows release builds, output is sent to OutputDebugString (viewable via DebugView).
pub fn maybe_launch_from_wrapper_args() -> bool {
    let args: Vec<OsString> = std::env::args_os().collect();

    if let Some(plan) = parse_wrapper_args_os(args) {
        debug_output("Wrapper mode detected");

        match launch_wrapper_plan(&plan) {
            Ok(()) => {
                debug_output("Wrapper launch completed successfully");
                return true;
            }
            Err(e) => {
                debug_output(&format!("Wrapper launch failed: {}", e));
                return false;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wrapper_args_os_returns_none_when_only_manager_path() {
        let args = vec![OsString::from("C:\\Manager.exe")];
        let result = parse_wrapper_args_os(args);
        assert_eq!(result, None);
    }

    #[test]
    fn parse_wrapper_args_os_returns_program_when_no_additional_args() {
        let args = vec![
            OsString::from("C:\\Manager.exe"),
            OsString::from("C:\\Game.exe"),
        ];
        let result = parse_wrapper_args_os(args);
        assert_eq!(
            result,
            Some(WrapperLaunchPlan {
                program: OsString::from("C:\\Game.exe"),
                args: vec![],
            })
        );
    }

    #[test]
    fn parse_wrapper_args_os_forwards_all_game_args() {
        let args = vec![
            OsString::from("C:\\Manager.exe"),
            OsString::from("C:\\Game.exe"),
            OsString::from("-windowed"),
            OsString::from("-width"),
            OsString::from("1920"),
        ];
        let result = parse_wrapper_args_os(args);
        assert_eq!(
            result,
            Some(WrapperLaunchPlan {
                program: OsString::from("C:\\Game.exe"),
                args: vec![
                    OsString::from("-windowed"),
                    OsString::from("-width"),
                    OsString::from("1920"),
                ],
            })
        );
    }

    #[test]
    fn parse_wrapper_args_os_handles_empty_args() {
        let args: Vec<OsString> = vec![];
        let result = parse_wrapper_args_os(args);
        assert_eq!(result, None);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn parse_wrapper_args_os_rejects_non_executable_flag() {
        let args = vec![OsString::from("C:\\Manager.exe"), OsString::from("--debug")];
        let result = parse_wrapper_args_os(args);
        assert_eq!(result, None);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn parse_wrapper_args_os_accepts_uppercase_exe() {
        let args = vec![
            OsString::from("C:\\Manager.exe"),
            OsString::from("C:\\Game.EXE"),
        ];
        let result = parse_wrapper_args_os(args);
        assert_eq!(
            result,
            Some(WrapperLaunchPlan {
                program: OsString::from("C:\\Game.EXE"),
                args: vec![],
            })
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn parse_wrapper_args_os_rejects_non_exe_extension() {
        let args = vec![
            OsString::from("C:\\Manager.exe"),
            OsString::from("C:\\Game.txt"),
        ];
        let result = parse_wrapper_args_os(args);
        assert_eq!(result, None);
    }
}
