#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Comment {
    pub(crate) line: usize,
    pub(crate) text: String,
}

pub(crate) fn comments(source: &str) -> Vec<Comment> {
    let mut scanner = Scanner {
        chars: source.chars().collect(),
        position: 0,
        line: 1,
        comments: Vec::new(),
    };
    scanner.scan();
    scanner.comments
}

struct Scanner {
    chars: Vec<char>,
    position: usize,
    line: usize,
    comments: Vec<Comment>,
}

impl Scanner {
    fn scan(&mut self) {
        while let Some(current) = self.peek(0) {
            match current {
                '/' if self.peek(1) == Some('/') => self.line_comment(),
                '/' if self.peek(1) == Some('*') => self.block_comment(),
                '"' => self.quoted_string(),
                'r' if self.starts_raw_string() => self.raw_string(),
                '\'' => self.char_literal_or_lifetime(),
                _ => self.advance(),
            }
        }
    }

    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.position + offset).copied()
    }

    fn advance(&mut self) {
        if self.peek(0) == Some('\n') {
            self.line += 1;
        }
        self.position += 1;
    }

    fn line_comment(&mut self) {
        let line = self.line;
        let mut text = String::new();
        while let Some(current) = self.peek(0) {
            if current == '\n' {
                break;
            }
            text.push(current);
            self.advance();
        }
        self.comments.push(Comment { line, text });
    }

    fn block_comment(&mut self) {
        let line = self.line;
        let mut text = String::new();
        let mut depth = 0usize;
        while let Some(current) = self.peek(0) {
            if current == '/' && self.peek(1) == Some('*') {
                depth += 1;
                text.push_str("/*");
                self.advance();
                self.advance();
            } else if current == '*' && self.peek(1) == Some('/') {
                depth -= 1;
                text.push_str("*/");
                self.advance();
                self.advance();
                if depth == 0 {
                    break;
                }
            } else {
                text.push(current);
                self.advance();
            }
        }
        self.comments.push(Comment { line, text });
    }

    fn quoted_string(&mut self) {
        self.advance();
        while let Some(current) = self.peek(0) {
            match current {
                '\\' => {
                    self.advance();
                    self.advance();
                }
                '"' => {
                    self.advance();
                    return;
                }
                _ => self.advance(),
            }
        }
    }

    fn starts_raw_string(&self) -> bool {
        if !self.raw_prefix_stands_alone() {
            return false;
        }
        let mut offset = 1;
        while self.peek(offset) == Some('#') {
            offset += 1;
        }
        self.peek(offset) == Some('"')
    }

    fn raw_prefix_stands_alone(&self) -> bool {
        let previous = self
            .position
            .checked_sub(1)
            .and_then(|index| self.chars.get(index))
            .copied();
        match previous {
            Some('b' | 'c') => {
                let before_prefix = self
                    .position
                    .checked_sub(2)
                    .and_then(|index| self.chars.get(index))
                    .copied();
                !before_prefix.is_some_and(is_identifier_char)
            }
            Some(other) => !is_identifier_char(other),
            None => true,
        }
    }

    fn raw_string(&mut self) {
        self.advance();
        let mut hashes = 0usize;
        while self.peek(0) == Some('#') {
            hashes += 1;
            self.advance();
        }
        self.advance();
        while self.peek(0).is_some() {
            if self.peek(0) == Some('"') && (1..=hashes).all(|offset| self.peek(offset) == Some('#')) {
                for _ in 0..=hashes {
                    self.advance();
                }
                return;
            }
            self.advance();
        }
    }

    fn char_literal_or_lifetime(&mut self) {
        if self.peek(1) == Some('\\') {
            self.advance();
            self.advance();
            self.advance();
            while let Some(current) = self.peek(0) {
                self.advance();
                if current == '\'' {
                    return;
                }
            }
        } else if self.peek(2) == Some('\'') {
            self.advance();
            self.advance();
            self.advance();
        } else {
            self.advance();
        }
    }
}

fn is_identifier_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

#[cfg(test)]
mod tests {
    use super::{Comment, comments};

    #[test]
    fn finds_line_and_block_comments_with_their_lines() {
        let source = "let a = 1; // first\n/* second\nspans */ let b = 2;\n";
        assert_eq!(
            comments(source),
            vec![
                Comment {
                    line: 1,
                    text: "// first".to_owned()
                },
                Comment {
                    line: 2,
                    text: "/* second\nspans */".to_owned()
                },
            ]
        );
    }

    #[test]
    fn ignores_comment_markers_inside_string_literals() {
        let source = "let url = \"https://example\"; let raw = r#\"// not \"a\" comment\"#; let byte = br\"/*\";";
        assert!(comments(source).is_empty());
    }

    #[test]
    fn handles_char_literals_and_lifetimes() {
        let source = "fn f<'a>(x: &'a str) -> char { let q = '\"'; let s = '\\''; '/' } // tail";
        assert_eq!(
            comments(source),
            vec![Comment {
                line: 1,
                text: "// tail".to_owned()
            }]
        );
    }

    #[test]
    fn handles_nested_block_comments() {
        let source = "/* outer /* inner */ still outer */ code";
        assert_eq!(
            comments(source),
            vec![Comment {
                line: 1,
                text: "/* outer /* inner */ still outer */".to_owned()
            }]
        );
    }
}
