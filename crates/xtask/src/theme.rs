use std::fs;
use std::io;
use std::path::PathBuf;

use scraper::{Html, Selector};

use crate::finding::Finding;
use crate::generate_mode::GenerateMode;
use crate::theme_codegen;
use crate::theme_stylesheet::theme_blocks;
use crate::theme_tokens::ThemeTokens;
use crate::workspace;

const DESIGN_BOOK: &str = "design/domovoi-design-book.html";
const GENERATED_MODULE: &str = "crates/theme/src/generated.rs";

pub(crate) fn run(mode: GenerateMode) -> io::Result<Vec<Finding>> {
    let root = workspace::root();
    let book = fs::read_to_string(root.join(DESIGN_BOOK))?;
    let tokens = ThemeTokens::from_blocks(&theme_blocks(&stylesheet(&book))?)?;
    let generated = theme_codegen::source(&tokens);
    let module_path = root.join(GENERATED_MODULE);
    let committed = match fs::read_to_string(&module_path) {
        Ok(committed) => committed,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    if committed == generated {
        return Ok(Vec::new());
    }
    match mode {
        GenerateMode::Write => {
            fs::write(&module_path, generated)?;
            Ok(Vec::new())
        }
        GenerateMode::Check => Ok(vec![stale_module(module_path, &committed, &generated)]),
    }
}

fn stylesheet(html: &str) -> String {
    let style = Selector::parse("style").expect("`style` is a valid selector");
    Html::parse_document(html)
        .select(&style)
        .map(|element| element.text().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn stale_module(path: PathBuf, committed: &str, generated: &str) -> Finding {
    let first_difference = committed
        .lines()
        .zip(generated.lines())
        .position(|(committed_line, generated_line)| committed_line != generated_line)
        .unwrap_or_else(|| committed.lines().count().min(generated.lines().count()));
    Finding {
        path,
        line: first_difference + 1,
        reason: "differs from the design book tokens; run `cargo xtask theme`".to_owned(),
    }
}
