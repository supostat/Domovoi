use std::path::PathBuf;

#[cfg(feature = "claude-code")]
use domovoi_agent::MCP_PROXY_FLAG;

use crate::screen::Screen;
use crate::snapshot_options::SnapshotOptions;

#[derive(Clone, Debug, PartialEq)]
pub enum CommandLine {
    Interactive,
    Snapshot {
        screens: Vec<Screen>,
        options: SnapshotOptions,
        state: PathBuf,
    },
    EndToEnd {
        scenario: PathBuf,
        report: PathBuf,
    },
    #[cfg(feature = "claude-code")]
    McpProxy {
        socket: PathBuf,
    },
}

impl CommandLine {
    pub fn parse(arguments: &[String]) -> Result<Self, String> {
        let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
        match words.as_slice() {
            [] => Ok(Self::Interactive),
            ["--e2e", scenario, "--report", report] => Ok(Self::EndToEnd {
                scenario: PathBuf::from(scenario),
                report: PathBuf::from(report),
            }),
            ["--snapshot", rest @ ..] => Self::snapshot(rest),
            #[cfg(feature = "claude-code")]
            [MCP_PROXY_FLAG, socket] => Ok(Self::McpProxy {
                socket: PathBuf::from(socket),
            }),
            _ => Err(usage()),
        }
    }

    fn snapshot(words: &[&str]) -> Result<Self, String> {
        let (target, rest) = words.split_first().ok_or_else(usage)?;
        let screens = if *target == "all" {
            Screen::ALL.to_vec()
        } else {
            Some(Screen::with_number(target))
                .filter(|screens| !screens.is_empty())
                .ok_or_else(|| format!("unknown screen {target}"))?
        };
        let mut dark = None;
        let mut scale = None;
        let mut state = None;
        let mut high_contrast = false;
        let mut directory = None;
        let mut position = 0;
        while position < rest.len() {
            match (rest[position], rest.get(position + 1)) {
                ("--theme", Some(theme)) => {
                    dark = Some(match *theme {
                        "dark" => true,
                        "light" => false,
                        other => return Err(format!("unknown theme {other}")),
                    });
                    position += 2;
                }
                ("--scale", Some(value)) => {
                    scale = Some(match *value {
                        "1" => 1.0,
                        "2" => 2.0,
                        other => return Err(format!("unsupported scale {other}")),
                    });
                    position += 2;
                }
                ("--state", Some(path)) => {
                    state = Some(PathBuf::from(path));
                    position += 2;
                }
                ("--contrast", Some(&"high")) => {
                    high_contrast = true;
                    position += 2;
                }
                (path, _) if directory.is_none() && !path.starts_with("--") => {
                    directory = Some(PathBuf::from(path));
                    position += 1;
                }
                _ => return Err(usage()),
            }
        }
        Ok(Self::Snapshot {
            screens: screens.clone(),
            options: SnapshotOptions {
                screen: screens[0],
                dark: dark.ok_or_else(usage)?,
                scale: scale.ok_or_else(usage)?,
                high_contrast,
                directory: directory.ok_or_else(usage)?,
            },
            state: state.ok_or_else(usage)?,
        })
    }
}

fn usage() -> String {
    "usage: domovoi [--snapshot <01|02|03|04|05|06|07|08|all> --theme <dark|light> --scale <1|2> --state <fixture.json> [--contrast high] <dir> | --e2e <scenario.json> --report <report.json>]".to_owned()
}
