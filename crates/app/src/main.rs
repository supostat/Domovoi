use std::env;
use std::process::ExitCode;

#[cfg(feature = "claude-code")]
use domovoi_agent::relay_mcp_stdio;
use domovoi_app::{
    CommandLine, ShellFixture, SnapshotOptions, development_fonts, run_end_to_end, run_interactive, write_snapshots,
};
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
    match CommandLine::parse(arguments)? {
        CommandLine::Interactive => run_interactive(windowed()?),
        CommandLine::Snapshot {
            screens,
            options,
            state,
        } => {
            let marker = windowed()?;
            let fixture = ShellFixture::read(&state)?;
            for screen in screens {
                let options = SnapshotOptions {
                    screen,
                    ..options.clone()
                };
                write_snapshots(marker, &fixture.shell_data(screen)?, &[screen], &options)?;
            }
            Ok(())
        }
        CommandLine::EndToEnd { scenario, report } => run_end_to_end(windowed()?, &scenario, &report),
        #[cfg(feature = "claude-code")]
        CommandLine::McpProxy { socket } => relay_mcp_stdio(&socket).map_err(|error| error.to_string()),
    }
}

fn windowed() -> Result<MainThreadMarker, String> {
    let marker = MainThreadMarker::new().ok_or("DOMOVOI runs on the main thread")?;
    if let Some(fonts) = development_fonts() {
        register_fonts(fonts)?;
    }
    Ok(marker)
}
