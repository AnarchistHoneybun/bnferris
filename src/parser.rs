use std::fmt;
use crate::lexer::{Lexer, Token, TokenKind, Loc, DiagErr};

#[derive(Debug, Clone)]
pub enum Expr {
    Symbol {
        loc: Loc,
        name: String,
    },
    String {
        loc: Loc,
        text: String,
    },
    Alternation {
        loc: Loc,
        variants: Vec<Expr>,
    },
    Concat {
        loc: Loc,
        elements: Vec<Expr>,
    },
    Repetition {
        loc: Loc,
        body: Box<Expr>,
        lower: u32,
        upper: u32,
    },
    Range {
        loc: Loc,
        lower: char,
        upper: char,
    },
}

impl Expr {
    pub fn get_loc(&self) -> Loc {
        match self {
            Expr::Symbol { loc, .. } => loc.clone(),
            Expr::String { loc, .. } => loc.clone(),
            Expr::Alternation { loc, .. } => loc.clone(),
            Expr::Concat { loc, .. } => loc.clone(),
            Expr::Repetition { loc, .. } => loc.clone(),
            Expr::Range { loc, .. } => loc.clone(),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Symbol { name, .. } => write!(f, "{}", name),

            Expr::String { text, .. } => {
                write!(f, "\"")?;
                for ch in text.chars() {
                    match ch {
                        '\n' => write!(f, "\\n")?,
                        '\r' => write!(f, "\\r")?,
                        '\\' => write!(f, "\\\\")?,
                        '"' => write!(f, "\\\"")?,
                        ch if ch.is_control() => write!(f, "\\x{:02x}", ch as u32)?,
                        ch => write!(f, "{}", ch)?,
                    }
                }
                write!(f, "\"")
            }

            Expr::Alternation { variants, .. } => {
                let mut first = true;
                for variant in variants {
                    if !first {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", variant)?;
                    first = false;
                }
                Ok(())
            }

            Expr::Concat { elements, .. } => {
                let mut first = true;
                for elem in elements {
                    if !first {
                        write!(f, " ")?;
                    }
                    match elem {
                        Expr::Alternation { .. } => write!(f, "( {} )", elem)?,
                        _ => write!(f, "{}", elem)?,
                    }
                    first = false;
                }
                Ok(())
            }

            Expr::Repetition { lower, upper, body, .. } => {
                if *lower == 0 && *upper == 1 {
                    write!(f, "[ {} ]", body)
                } else if lower == upper {
                    write!(f, "{}( {} )", lower, body)
                } else {
                    write!(f, "{}*{}( {} )", lower, upper, body)
                }
            }

            Expr::Range { lower, upper, .. } => {
                write!(f, "%x{:02X}-{:02X}", *lower as u32, *upper as u32)
            }
        }
    }
}

pub const MAX_UNSPECIFIED_UPPER_REPETITION_BOUND: u32 = 20;

pub fn expect_token(lexer: &mut Lexer, kind: TokenKind) -> Result<Token, DiagErr> {
    let token = lexer.next()?;
    if token.kind != kind {
        return Err(DiagErr {
            loc: token.loc,
            message: format!("Expected {} but got {}", kind.name(), token.kind.name()),
        });
    }
    Ok(token)
}

pub fn parse_primary_expr(lexer: &mut Lexer) -> Result<Expr, DiagErr> {
    let token = lexer.next()?;

    match token.kind {
        TokenKind::ParenOpen => {
            let expr = parse_expr(lexer)?;
            expect_token(lexer, TokenKind::ParenClose)?;
            Ok(expr)
        }

        TokenKind::CurlyOpen => {
            let body = parse_expr(lexer)?;
            expect_token(lexer, TokenKind::CurlyClose)?;
            Ok(Expr::Repetition {
                loc: token.loc,
                body: Box::new(body),
                lower: 0,
                upper: MAX_UNSPECIFIED_UPPER_REPETITION_BOUND,
            })
        }

        TokenKind::BracketOpen => {
            let body = parse_expr(lexer)?;
            expect_token(lexer, TokenKind::BracketClose)?;
            Ok(Expr::Repetition {
                loc: token.loc,
                body: Box::new(body),
                lower: 0,
                upper: 1,
            })
        }

        TokenKind::Symbol => Ok(Expr::Symbol {
            loc: token.loc,
            name: token.text,
        }),

        TokenKind::ValueRange => {
            let chars: Vec<char> = token.text.chars().collect();
            if chars.len() != 2 {
                return Err(DiagErr {
                    loc: token.loc,
                    message: format!("Value range is expected to have 2 bounds but got {}", chars.len()),
                });
            }
            Ok(Expr::Range {
                loc: token.loc,
                lower: chars[0],
                upper: chars[1],
            })
        }

        TokenKind::String => {
            let peek = lexer.peek()?;
            if peek.kind != TokenKind::Ellipsis {
                return Ok(Expr::String {
                    loc: token.loc,
                    text: token.text,
                });
            }

            if token.text.chars().count() != 1 {
                return Err(DiagErr {
                    loc: token.loc,
                    message: format!(
                        "The lower boundary of the range is expected to be 1 symbol string. Got {} instead.",
                        token.text.chars().count()
                    ),
                });
            }

            lexer.next()?; // consume ellipsis
            let upper = expect_token(lexer, TokenKind::String)?;

            if upper.text.chars().count() != 1 {
                return Err(DiagErr {
                    loc: upper.loc,
                    message: format!(
                        "The upper boundary of the range is expected to be 1 symbol string. Got {} instead.",
                        upper.text.chars().count()
                    ),
                });
            }

            Ok(Expr::Range {
                loc: token.loc,
                lower: token.text.chars().next().unwrap(),
                upper: upper.text.chars().next().unwrap(),
            })
        }

        TokenKind::Asterisk => {
            let upper = lexer.peek()?;
            if upper.kind != TokenKind::Number {
                let body = parse_primary_expr(lexer)?;
                return Ok(Expr::Repetition {
                    loc: token.loc,
                    lower: 0,
                    upper: MAX_UNSPECIFIED_UPPER_REPETITION_BOUND,
                    body: Box::new(body),
                });
            }

            let upper_num = upper.number.unwrap();
            lexer.next()?; // consume number

            let body = parse_primary_expr(lexer)?;
            Ok(Expr::Repetition {
                loc: token.loc,
                lower: 0,
                upper: upper_num,
                body: Box::new(body),
            })
        }

        TokenKind::Number => {
            let num = token.number.unwrap();
            let peek = lexer.peek()?;

            match peek.kind {
                TokenKind::Asterisk => {
                    lexer.next()?; // consume asterisk
                    let upper = lexer.peek()?;

                    if upper.kind != TokenKind::Number {
                        let body = parse_primary_expr(lexer)?;
                        return Ok(Expr::Repetition {
                            loc: token.loc,
                            lower: num,
                            upper: MAX_UNSPECIFIED_UPPER_REPETITION_BOUND,
                            body: Box::new(body),
                        });
                    }

                    let upper_num = upper.number.unwrap();
                    lexer.next()?; // consume number

                    let body = parse_primary_expr(lexer)?;
                    Ok(Expr::Repetition {
                        loc: token.loc,
                        lower: num,
                        upper: upper_num,
                        body: Box::new(body),
                    })
                }
                _ => {
                    let body = parse_primary_expr(lexer)?;
                    Ok(Expr::Repetition {
                        loc: token.loc,
                        lower: num,
                        upper: num,
                        body: Box::new(body),
                    })
                }
            }
        }

        _ => Err(DiagErr {
            loc: token.loc,
            message: format!("Expected start of an expression, but got {}", token.kind.name()),
        }),
    }
}

fn is_primary_start(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Symbol
            | TokenKind::String
            | TokenKind::BracketOpen
            | TokenKind::CurlyOpen
            | TokenKind::ParenOpen
            | TokenKind::Number
            | TokenKind::Asterisk
            | TokenKind::ValueRange
    )
}

pub fn parse_concat_expr(lexer: &mut Lexer) -> Result<Expr, DiagErr> {
    let primary = parse_primary_expr(lexer)?;

    let peek = lexer.peek()?;
    if !is_primary_start(&peek.kind) {
        return Ok(primary);
    }

    let mut elements = vec![primary];

    while let Ok(token) = lexer.peek() {
        if !is_primary_start(&token.kind) {
            break;
        }

        let child = parse_primary_expr(lexer)?;
        elements.push(child);
    }

    Ok(Expr::Concat {
        loc: elements[0].get_loc(),
        elements,
    })
}

pub fn parse_alt_expr(lexer: &mut Lexer) -> Result<Expr, DiagErr> {
    let concat = parse_concat_expr(lexer)?;

    let peek = lexer.peek()?;
    if peek.kind != TokenKind::Alternation {
        return Ok(concat);
    }

    let mut variants = vec![concat];

    while let Ok(token) = lexer.peek() {
        if token.kind != TokenKind::Alternation {
            break;
        }

        lexer.next()?; // consume alternation token
        let child = parse_concat_expr(lexer)?;
        variants.push(child);
    }

    Ok(Expr::Alternation {
        loc: variants[0].get_loc(),
        variants,
    })
}

pub fn parse_expr(lexer: &mut Lexer) -> Result<Expr, DiagErr> {
    parse_alt_expr(lexer)
}