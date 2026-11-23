use crate::diagnostic::{Diagnostic, Location};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Name(String),
    String(String),
    Bool(bool),
    Number(u64),
    Null,
    Semicolon,
    OpenBranch,
    CloseBranch,
    OpenMap,
    CloseMap,
    OpenList,
    CloseList,
    Colon,
    Comma,
    Arrow,
    Dot,
    Range,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    Question,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    End,
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub kind: Kind,
    pub location: Location,
    pub end: usize,
}

pub(crate) fn tokenize(source: &str) -> Result<Vec<Token>, Diagnostic> {
    crate::limits::input(source, true)?;
    Lexer {
        source,
        offset: 0,
        line: 1,
        column: 1,
    }
    .tokens(true)
}

pub(crate) fn validate_source(source: &str) -> Result<(), Diagnostic> {
    crate::limits::input(source, true)?;
    Lexer {
        source,
        offset: 0,
        line: 1,
        column: 1,
    }
    .tokens(false)?;
    Ok(())
}

struct Lexer<'a> {
    source: &'a str,
    offset: usize,
    line: usize,
    column: usize,
}

impl Lexer<'_> {
    fn location(&self) -> Location {
        Location {
            offset: self.offset,
            line: self.line,
            column: self.column,
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(ch) = self.peek() {
            let after_cr = self.offset > 0 && self.source.as_bytes()[self.offset - 1] == b'\r';
            self.offset += ch.len_utf8();
            match ch {
                '\r' => {
                    self.line += 1;
                    self.column = 1;
                }
                '\n' => {
                    self.line += usize::from(!after_cr);
                    self.column = 1;
                }
                _ => self.column += 1,
            }
        }
    }

    fn tokens(mut self, collect: bool) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        let mut count = 0;
        while let Some(ch) = self.peek() {
            if matches!(ch, ' ' | '\t' | '\r' | '\n') {
                self.advance();
                continue;
            }
            if self.source[self.offset..].starts_with("//") {
                while self.peek().is_some_and(|ch| ch != '\n' && ch != '\r') {
                    self.advance();
                }
                continue;
            }
            let location = self.location();
            let kind = match ch {
                '"' => self.string()?,
                ch if ch.is_ascii_digit() => self.number()?,
                ch if ch.is_ascii_alphabetic() || ch == '_' => self.name(),
                '-' if self.source[self.offset..].starts_with("->") => {
                    self.advance();
                    self.advance();
                    Kind::Arrow
                }
                '.' if self.source[self.offset..].starts_with("..") => {
                    self.advance();
                    self.advance();
                    Kind::Range
                }
                '=' | '!' | '<' | '>' => {
                    self.advance();
                    let equal = self.peek() == Some('=');
                    if equal {
                        self.advance();
                    }
                    match (ch, equal) {
                        ('=', false) => Kind::Assign,
                        ('=', true) => Kind::Eq,
                        ('!', true) => Kind::Ne,
                        ('<', false) => Kind::Lt,
                        ('<', true) => Kind::Le,
                        ('>', false) => Kind::Gt,
                        ('>', true) => Kind::Ge,
                        _ => return Err(Diagnostic::new("syntax", "expected !=", Some(location))),
                    }
                }
                _ => {
                    let kind = match ch {
                        ';' => Kind::Semicolon,
                        '{' => Kind::OpenBranch,
                        '}' => Kind::CloseBranch,
                        '(' => Kind::OpenMap,
                        ')' => Kind::CloseMap,
                        '[' => Kind::OpenList,
                        ']' => Kind::CloseList,
                        ':' => Kind::Colon,
                        ',' => Kind::Comma,
                        '.' => Kind::Dot,
                        '?' => Kind::Question,
                        '+' => Kind::Plus,
                        '-' => Kind::Minus,
                        '*' => Kind::Star,
                        '/' => Kind::Slash,
                        '%' => Kind::Percent,
                        _ => {
                            return Err(Diagnostic::new(
                                "syntax",
                                format!("unexpected character {ch:?}"),
                                Some(location),
                            ));
                        }
                    };
                    self.advance();
                    kind
                }
            };
            if count == crate::limits::MAX_TOKENS {
                return Err(Diagnostic::new(
                    "resource_limit",
                    "Input exceeds 262144 tokens",
                    Some(location),
                ));
            }
            count += 1;
            if collect {
                tokens.push(Token {
                    kind,
                    location,
                    end: self.offset,
                });
            }
        }
        if collect {
            tokens.push(Token {
                kind: Kind::End,
                location: self.location(),
                end: self.offset,
            });
        }
        Ok(tokens)
    }

    fn name(&mut self) -> Kind {
        let start = self.offset;
        while self
            .peek()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            self.advance();
        }
        match &self.source[start..self.offset] {
            "true" => Kind::Bool(true),
            "false" => Kind::Bool(false),
            "null" => Kind::Null,
            name => Kind::Name(name.to_owned()),
        }
    }

    fn number(&mut self) -> Result<Kind, Diagnostic> {
        let start = self.offset;
        let location = self.location();
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.advance();
        }
        let text = &self.source[start..self.offset];
        if text.len() > 1 && text.starts_with('0') {
            return Err(Diagnostic::new(
                "syntax",
                "integer literals cannot have leading zeros",
                Some(location),
            ));
        }
        text.parse()
            .map(Kind::Number)
            .map_err(|_| Diagnostic::new("syntax", "integer literal exceeds u64", Some(location)))
    }

    fn string(&mut self) -> Result<Kind, Diagnostic> {
        let location = self.location();
        let start = self.offset;
        self.advance();
        while let Some(ch) = self.peek() {
            self.advance();
            if ch == '"' {
                return serde_json::from_str(&self.source[start..self.offset])
                    .map(Kind::String)
                    .map_err(|error| {
                        Diagnostic::new("invalid_string", error.to_string(), Some(location))
                    });
            }
            if ch == '\\' {
                self.advance();
            }
        }
        Err(Diagnostic::new(
            "invalid_string",
            "unterminated string",
            Some(location),
        ))
    }
}

pub(crate) fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        "smsl"
            | "event"
            | "branch"
            | "mutable"
            | "fact"
            | "data"
            | "derive"
            | "abstract"
            | "label"
            | "initial"
            | "activate"
            | "child"
            | "read"
            | "state"
            | "on"
            | "when"
            | "set"
            | "emit"
            | "action"
            | "outcomes"
            | "import"
            | "export"
            | "from"
            | "as"
            | "and"
            | "or"
            | "not"
            | "is_unknown"
            | "true"
            | "false"
            | "null"
            | "unknown"
            | "bool"
            | "string"
            | "time"
    )
}
