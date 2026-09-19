//! Private one-shot worker for helper_processes::launch_helpers, not an actor CLI.
#![forbid(unsafe_code)]

#[cfg(unix)]
#[path = "fa-native-helper/config.rs"]
mod config;

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    use fa_reference::action::consequence::oversight::helper_client::native::process::NativeProcessStop;
    match config::run(std::env::args_os().skip(1).take(2).collect()) {
        Ok(report) if report.stop == NativeProcessStop::ReplySent => std::process::ExitCode::SUCCESS,
        Ok(report) => {
            eprintln!("fa-native-helper: stopped: {report:?}");
            std::process::ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("fa-native-helper: {error}");
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    eprintln!("fa-native-helper: the inherited Unix-socket profile is required");
    std::process::ExitCode::from(2)
}
