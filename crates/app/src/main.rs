use std::env;
use std::process::ExitCode;

use domovoi_app::{CommandLine, ShellFixture, development_fonts, run_end_to_end, run_interactive, write_snapshots};
use domovoi_render::register_fonts;
use objc2::MainThreadMarker;

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("domovoi: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<(), String> {
    let command = CommandLine::parse(arguments)?;
    let marker = MainThreadMarker::new().ok_or("DOMOVOI runs on the main thread")?;
    if let Some(fonts) = development_fonts() {
        register_fonts(fonts)?;
    }
    match command {
        CommandLine::Interactive => run_interactive(marker),
        CommandLine::Snapshot {
            screens,
            options,
            state,
        } => {
            let data = ShellFixture::read(&state)?.shell_data()?;
            write_snapshots(marker, &data, &screens, &options).map(drop)
        }
        CommandLine::EndToEnd { scenario, report } => run_end_to_end(marker, &scenario, &report),
    }
}
