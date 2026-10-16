use std::fs;
use std::io;
use std::path::PathBuf;

use crate::attribute_reader::invalid;

const FIRMLINKS: &str = "/usr/share/firmlinks";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Firmlink {
    pub path: PathBuf,
    pub data_relative: PathBuf,
}

pub fn firmlinks() -> io::Result<Vec<Firmlink>> {
    parse(&fs::read_to_string(FIRMLINKS)?)
}

fn parse(listing: &str) -> io::Result<Vec<Firmlink>> {
    listing
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (path, data_relative) = line
                .split_once('\t')
                .ok_or_else(|| invalid("firmlink line has no tab separator"))?;
            Ok(Firmlink {
                path: PathBuf::from(path),
                data_relative: PathBuf::from(data_relative),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Firmlink, parse};

    #[test]
    fn parses_tab_separated_pairs() {
        let firmlinks = parse("/Users\tUsers\n/usr/local\tusr/local\n").expect("listing parses");
        assert_eq!(
            firmlinks,
            vec![
                Firmlink {
                    path: PathBuf::from("/Users"),
                    data_relative: PathBuf::from("Users"),
                },
                Firmlink {
                    path: PathBuf::from("/usr/local"),
                    data_relative: PathBuf::from("usr/local"),
                },
            ]
        );
    }

    #[test]
    fn rejects_a_line_without_a_tab() {
        assert!(parse("/Users Users").is_err());
    }
}
