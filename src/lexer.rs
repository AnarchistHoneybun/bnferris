use std::fmt;

#[derive(Debug, Clone)]
pub struct Loc {
    pub file_path: String,
    pub row: usize,
    pub col: usize,
}

impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file_path, self.row + 1, self.col + 1)
    }
}

#[derive(Debug)]
pub struct DiagErr {
    pub loc: Loc,
    pub message: String,
}

impl fmt::Display for DiagErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ERROR: {}", self.loc, self.message)
    }
}

impl std::error::Error for DiagErr {}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Eol,
    Symbol,
    Definition,
    Alternation,
    String,
    BracketOpen,
    BracketClose,
    CurlyOpen,
    CurlyClose,
    ParenOpen,
    ParenClose,
    Ellipsis,
    Number,
    Asterisk,
    IncAlternative,
    ValueRange,
}

impl TokenKind {
    pub fn name(&self) -> &'static str {
        match self {
            TokenKind::Eol => "end of line",
            TokenKind::Symbol => "symbol",
            TokenKind::Definition => "definition symbol",
            TokenKind::Alternation => "alternation symbol",
            TokenKind::String => "string literal",
            TokenKind::BracketOpen => "open bracket",
            TokenKind::BracketClose => "close bracket",
            TokenKind::CurlyOpen => "open curly",
            TokenKind::CurlyClose => "close curly",
            TokenKind::ParenOpen => "open paren",
            TokenKind::ParenClose => "close paren",
            TokenKind::Ellipsis => "ellipsis",
            TokenKind::Number => "number",
            TokenKind::Asterisk => "asterisk",
            TokenKind::IncAlternative => "incremental alternative",
            TokenKind::ValueRange => "value range",
        }
    }
}

#[derive(Debug)]
struct LiteralToken {
    text: &'static str,
    kind: TokenKind,
}

const LITERAL_TOKENS: &[LiteralToken] = &[
    LiteralToken { text: "::=", kind: TokenKind::Definition },
    LiteralToken { text: "=/", kind: TokenKind::IncAlternative },
    LiteralToken { text: "=", kind: TokenKind::Definition },
    LiteralToken { text: "|", kind: TokenKind::Alternation },
    LiteralToken { text: "/", kind: TokenKind::Alternation },
    LiteralToken { text: "[", kind: TokenKind::BracketOpen },
    LiteralToken { text: "]", kind: TokenKind::BracketClose },
    LiteralToken { text: "{", kind: TokenKind::CurlyOpen },
    LiteralToken { text: "}", kind: TokenKind::CurlyClose },
    LiteralToken { text: "(", kind: TokenKind::ParenOpen },
    LiteralToken { text: ")", kind: TokenKind::ParenClose },
    LiteralToken { text: "...", kind: TokenKind::Ellipsis },
    LiteralToken { text: "*", kind: TokenKind::Asterisk },
];

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub number: Option<u32>,
    pub loc: Loc,
}

pub struct Lexer {
    content: Vec<char>,
    file_path: String,
    row: usize,
    col: usize,
    peek_buf: Option<Token>,
}

impl Lexer {
    pub fn new(content: String, file_path: String, row: usize) -> Self {
        Lexer {
            content: content.chars().collect(),
            file_path,
            row,
            col: 0,
            peek_buf: None,
        }
    }

    fn trim(&mut self) {
        while self.col < self.content.len() && self.content[self.col].is_whitespace() {
            self.col += 1;
        }
    }

    fn has_prefix(&self, prefix: &str) -> bool {
        if self.col + prefix.len() > self.content.len() {
            return false;
        }
        prefix.chars().enumerate().all(|(i, c)| self.content[self.col + i] == c)
    }

    fn loc(&self) -> Loc {
        Loc {
            file_path: self.file_path.clone(),
            row: self.row,
            col: self.col,
        }
    }

    fn chop_hex_byte_value(&mut self) -> Result<char, DiagErr> {
        let mut result: u32 = 0;
        for i in 0..2 {
            if self.col >= self.content.len() {
                return Err(DiagErr {
                    loc: self.loc(),
                    message: format!("Unfinished hexadecimal value of a byte. Expected 2 hex digits, but got {}.", i),
                });
            }
            let x = self.content[self.col];
            result = result * 0x10 + match x {
                '0'..='9' => x as u32 - '0' as u32,
                'a'..='f' => x as u32 - 'a' as u32 + 10,
                'A'..='F' => x as u32 - 'A' as u32 + 10,
                _ => return Err(DiagErr {
                    loc: self.loc(),
                    message: format!("Expected hex digit, but got `{}`", x),
                }),
            };
            self.col += 1;
        }
        Ok(char::from_u32(result).unwrap())
    }

    fn chop_str_lit(&mut self) -> Result<String, DiagErr> {
        if self.col >= self.content.len() {
            return Ok(String::new());
        }

        let quote = self.content[self.col];
        self.col += 1;
        let begin = self.col;
        let mut lit = String::new();

        while self.col < self.content.len() {
            if self.content[self.col] == '\\' {
                self.col += 1;
                if self.col >= self.content.len() {
                    return Err(DiagErr {
                        loc: self.loc(),
                        message: "Unfinished escape sequence".to_string(),
                    });
                }

                match self.content[self.col] {
                    '0' => {
                        lit.push('\0');
                        self.col += 1;
                    }
                    'n' => {
                        lit.push('\n');
                        self.col += 1;
                    }
                    'r' => {
                        lit.push('\r');
                        self.col += 1;
                    }
                    '\\' => {
                        lit.push('\\');
                        self.col += 1;
                    }
                    'x' => {
                        self.col += 1;
                        let value = self.chop_hex_byte_value()?;
                        lit.push(value);
                    }
                    c if c == quote => {
                        lit.push(quote);
                        self.col += 1;
                    }
                    c => {
                        return Err(DiagErr {
                            loc: self.loc(),
                            message: format!("Unknown escape sequence starting with {}", c),
                        });
                    }
                }
            } else {
                if self.content[self.col] == quote {
                    break;
                }
                lit.push(self.content[self.col]);
                self.col += 1;
            }
        }

        if self.col >= self.content.len() || self.content[self.col] != quote {
            return Err(DiagErr {
                loc: Loc {
                    file_path: self.file_path.clone(),
                    row: self.row,
                    col: begin,
                },
                message: format!("Expected '{}' at the end of this string literal", quote),
            });
        }
        self.col += 1;

        Ok(lit)
    }

    fn is_symbol_start(ch: char) -> bool {
        ch.is_alphabetic() || ch == '-' || ch == '_'
    }

    fn is_symbol(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '-' || ch == '_'
    }

    pub fn chop_token(&mut self) -> Result<Token, DiagErr> {
        self.trim();

        if self.has_prefix("//") || self.has_prefix(";") {
            self.col = self.content.len();
        }

        let token_loc = self.loc();

        if self.col >= self.content.len() {
            return Ok(Token {
                kind: TokenKind::Eol,
                text: String::new(),
                number: None,
                loc: token_loc,
            });
        }

        if self.content[self.col].is_numeric() {
            let begin = self.col;
            let mut number = 0u32;
            while self.col < self.content.len() && self.content[self.col].is_numeric() {
                number = number * 10 + self.content[self.col].to_digit(10).unwrap();
                self.col += 1;
            }
            return Ok(Token {
                kind: TokenKind::Number,
                text: self.content[begin..self.col].iter().collect(),
                number: Some(number),
                loc: token_loc,
            });
        }

        if Self::is_symbol_start(self.content[self.col]) {
            let begin = self.col;
            while self.col < self.content.len() && Self::is_symbol(self.content[self.col]) {
                self.col += 1;
            }
            return Ok(Token {
                kind: TokenKind::Symbol,
                text: self.content[begin..self.col].iter().collect(),
                number: None,
                loc: token_loc,
            });
        }

        if self.content[self.col] == '<' {
            let begin = self.col + 1;
            self.col = begin;
            while self.col < self.content.len() && self.content[self.col] != '>' {
                let ch = self.content[self.col];
                if !Self::is_symbol(ch) {
                    return Err(DiagErr {
                        loc: self.loc(),
                        message: format!("Unexpected character in symbol name {}", ch),
                    });
                }
                self.col += 1;
            }
            if self.col >= self.content.len() {
                return Err(DiagErr {
                    loc: self.loc(),
                    message: "Expected '>' at the end of the symbol name".to_string(),
                });
            }

            let text: String = self.content[begin..self.col].iter().collect();
            self.col += 1;
            return Ok(Token {
                kind: TokenKind::Symbol,
                text,
                number: None,
                loc: token_loc,
            });
        }

        if self.content[self.col] == '"' || self.content[self.col] == '\'' {
            let lit = self.chop_str_lit()?;
            return Ok(Token {
                kind: TokenKind::String,
                text: lit,
                number: None,
                loc: token_loc,
            });
        }

        if self.has_prefix("%x") {
            self.col += 2;

            let value = self.chop_hex_byte_value()?;
            let mut text = String::new();
            text.push(value);

            if self.has_prefix("-") {
                self.col += 1;
                let value = self.chop_hex_byte_value()?;
                text.push(value);
                return Ok(Token {
                    kind: TokenKind::ValueRange,
                    text,
                    number: None,
                    loc: token_loc,
                });
            } else {
                return Ok(Token {
                    kind: TokenKind::String,
                    text,
                    number: None,
                    loc: token_loc,
                });
            }
        }

        for literal in LITERAL_TOKENS {
            if self.has_prefix(literal.text) {
                self.col += literal.text.len();
                return Ok(Token {
                    kind: literal.kind.clone(),
                    text: literal.text.to_string(),
                    number: None,
                    loc: token_loc,
                });
            }
        }

        Err(DiagErr {
            loc: token_loc,
            message: "Invalid token".to_string(),
        })
    }

    pub fn peek(&mut self) -> Result<Token, DiagErr> {
        if let Some(token) = &self.peek_buf {
            Ok(token.clone())
        } else {
            let token = self.chop_token()?;
            self.peek_buf = Some(token.clone());
            Ok(token)
        }
    }

    pub fn next(&mut self) -> Result<Token, DiagErr> {
        if let Some(token) = self.peek_buf.take() {
            Ok(token)
        } else {
            self.chop_token()
        }
    }
}