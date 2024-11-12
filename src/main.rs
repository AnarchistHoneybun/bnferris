use std::collections::HashMap;
use std::fs;
use std::process;
use clap::Parser;
use rand::Rng;

mod lexer;
mod parser;

use lexer::{Lexer, Token, TokenKind, DiagErr};
use parser::Expr;

#[derive(Parser, Debug)]
#[command(version, about = "A program to generate random messages based on their BNF definition")]
struct BNFuzzerArgs {
    /// Path to the BNF grammar file
    #[arg(short, long, value_name = "FILE", required = true)]
    file: String,

    /// The symbol name to start generating from.  
    /// Use '!' to list all available symbols
    #[arg(short, long, value_name = "ENTRY", required = true)]
    entry: String,

    /// How many messages to generate
    #[arg(short, long, default_value_t = 1)]
    count: u32,

    /// Verify that all the symbols are defined
    #[arg(long)]
    verify: bool,

    /// Verify that all the symbols are used
    #[arg(long)]
    unused: bool,

    /// Dump the text representation of the entry symbol
    #[arg(long)]
    dump: bool,
}

#[derive(Debug, Clone)]
struct Rule {
    head: Token,
    body: Expr,
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ::= {}", self.head.text, self.body)
    }
}

fn generate_random_message(grammar: &HashMap<String, Rule>, expr: &Expr) -> Result<String, DiagErr> {
    let mut rng = rand::thread_rng();

    match expr {
        Expr::String { text, .. } => Ok(text.clone()),

        Expr::Symbol { name, loc, .. } => {
            let next_expr = grammar.get(name).ok_or_else(|| DiagErr {
                loc: loc.clone(),
                message: format!("Symbol <{}> is not defined", name),
            })?;
            generate_random_message(grammar, &next_expr.body)
        }

        Expr::Concat { elements, .. } => {
            let mut message = String::new();
            for element in elements {
                message.push_str(&generate_random_message(grammar, element)?);
            }
            Ok(message)
        }

        Expr::Alternation { variants, .. } => {
            let i = rng.gen_range(0..variants.len());
            generate_random_message(grammar, &variants[i])
        }

        Expr::Repetition { lower, upper, body, loc, .. } => {
            if lower > upper {
                return Err(DiagErr {
                    loc: loc.clone(),
                    message: "Upper bound of the repetition is lower than the lower one.".to_string(),
                });
            }

            let n = rng.gen_range(*lower..=*upper);
            let mut message = String::new();
            for _ in 0..n {
                message.push_str(&generate_random_message(grammar, body)?);
            }
            Ok(message)
        }

        Expr::Range { lower, upper, loc, .. } => {
            if lower > upper {
                return Err(DiagErr {
                    loc: loc.clone(),
                    message: "Upper bound of the range is lower than the lower one.".to_string(),
                });
            }

            let random_char = rng.gen_range(*lower as u32..=*upper as u32);
            Ok(char::from_u32(random_char).unwrap().to_string())
        }
    }
}

fn verify_all_symbols_defined_in_expr(grammar: &HashMap<String, Rule>, expr: &Expr) -> bool {
    let mut ok = true;

    match expr {
        Expr::Symbol { name, loc, .. } => {
            if !grammar.contains_key(name) {
                eprintln!("{}: ERROR: Symbol {} is not defined", loc, name);
                ok = false;
            }
        }

        Expr::Alternation { variants, .. } => {
            for variant in variants {
                if !verify_all_symbols_defined_in_expr(grammar, variant) {
                    ok = false;
                }
            }
        }

        Expr::Concat { elements, .. } => {
            for element in elements {
                if !verify_all_symbols_defined_in_expr(grammar, element) {
                    ok = false;
                }
            }
        }

        Expr::Repetition { body, .. } => {
            if !verify_all_symbols_defined_in_expr(grammar, body) {
                ok = false;
            }
        }

        Expr::String { .. } | Expr::Range { .. } => {}
    }

    ok
}

fn verify_all_symbols_defined(grammar: &HashMap<String, Rule>) -> bool {
    let mut ok = true;
    for rule in grammar.values() {
        if !verify_all_symbols_defined_in_expr(grammar, &rule.body) {
            ok = false;
        }
    }
    ok
}

fn walk_symbols_in_expr(
    grammar: &HashMap<String, Rule>,
    expr: &Expr,
    visited: &mut HashMap<String, bool>,
) -> Result<(), DiagErr> {
    match expr {
        Expr::Symbol { name, loc, .. } => {
            if !visited.contains_key(name) {
                visited.insert(name.clone(), true);
                let rule = grammar.get(name).ok_or_else(|| DiagErr {
                    loc: loc.clone(),
                    message: format!("Symbol <{}> is not defined", name),
                })?;
                walk_symbols_in_expr(grammar, &rule.body, visited)?;
            }
            Ok(())
        }

        Expr::String { .. } => Ok(()),

        Expr::Alternation { variants, .. } => {
            for variant in variants {
                walk_symbols_in_expr(grammar, variant, visited)?;
            }
            Ok(())
        }

        Expr::Concat { elements, .. } => {
            for element in elements {
                walk_symbols_in_expr(grammar, element, visited)?;
            }
            Ok(())
        }

        Expr::Repetition { body, .. } => walk_symbols_in_expr(grammar, body, visited),

        Expr::Range { .. } => Ok(()),
    }
}

fn main() {
    let args = BNFuzzerArgs::parse();

    let content = match fs::read_to_string(&args.file) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("ERROR: {}", err);
            process::exit(1);
        }
    };

    let mut grammar: HashMap<String, Rule> = HashMap::new();
    let mut parsing_error = false;

    for (row, line) in content.lines().enumerate() {
        let mut lexer = Lexer::new(line.to_string(), args.file.clone(), row);

        // Skip empty lines
        if let Ok(token) = lexer.peek() {
            if token.kind == TokenKind::Eol {
                continue;
            }
        }

        // Parse rule head
        let head = match parser::expect_token(&mut lexer, TokenKind::Symbol) {
            Ok(head) => head,
            Err(err) => {
                eprintln!("{}", err);
                parsing_error = true;
                continue;
            }
        };

        // Parse definition token
        let def = match lexer.next() {
            Ok(def) => def,
            Err(err) => {
                eprintln!("{}", err);
                parsing_error = true;
                continue;
            }
        };

        let symbol = head.text.clone();
        let existing_rule = grammar.get(&symbol);

        match def.kind {
            TokenKind::Definition => {
                if existing_rule.is_some() {
                    eprintln!("{}: ERROR: redefinition of the rule {}", head.loc, symbol);
                    if let Some(rule) = existing_rule {
                        eprintln!("{}: NOTE: the first definition is located here", rule.head.loc);
                    }
                    parsing_error = true;
                    continue;
                }

                let body = match parser::parse_expr(&mut lexer) {
                    Ok(body) => body,
                    Err(err) => {
                        eprintln!("{}", err);
                        parsing_error = true;
                        continue;
                    }
                };

                grammar.insert(symbol, Rule { head, body });
            }

            TokenKind::IncAlternative => {
                if existing_rule.is_none() {
                    eprintln!(
                        "{}: ERROR: can't apply incremental alternative to a non-existing rule {}. You need to define it first.",
                        head.loc, symbol
                    );
                    parsing_error = true;
                    continue;
                }

                let body = match parser::parse_expr(&mut lexer) {
                    Ok(body) => body,
                    Err(err) => {
                        eprintln!("{}", err);
                        parsing_error = true;
                        continue;
                    }
                };

                let mut rule = existing_rule.unwrap().clone();
                match &mut rule.body {
                    Expr::Alternation { ref mut variants, .. } => {
                        variants.push(body);
                    }
                    _ => {
                        let loc = rule.body.get_loc();
                        rule.body = Expr::Alternation {
                            loc,
                            variants: vec![rule.body.clone(), body],
                        };
                    }
                }
                grammar.insert(symbol, rule);
            }

            _ => {
                eprintln!(
                    "{}: ERROR: Expected {} or {} but got {}",
                    def.loc,
                    TokenKind::Definition.name(),
                    TokenKind::IncAlternative.name(),
                    def.kind.name()
                );
                parsing_error = true;
                continue;
            }
        }

        if let Err(err) = parser::expect_token(&mut lexer, TokenKind::Eol) {
            eprintln!("{}", err);
            parsing_error = true;
        }
    }

    if parsing_error {
        process::exit(1);
    }

    if args.verify && !verify_all_symbols_defined(&grammar) {
        process::exit(1);
    }

    if args.entry == "!" {
        let mut names: Vec<String> = grammar.keys().cloned().collect();
        names.sort();

        if args.dump {
            for name in names {
                let rule = &grammar[&name];
                println!("{}: {}", rule.head.loc, rule);
            }
            return;
        }

        for name in names {
            println!("{}", name);
        }
        return;
    }

    let rule = match grammar.get(&args.entry) {
        Some(rule) => rule,
        None => {
            eprintln!(
                "ERROR: Symbol {} is not defined. Pass -entry '!' to get the list of defined symbols.",
                args.entry
            );
            process::exit(1);
        }
    };

    if args.unused {
        let mut visited = HashMap::new();
        visited.insert(args.entry.clone(), true);

        if let Err(err) = walk_symbols_in_expr(&grammar, &rule.body, &mut visited) {
            eprintln!("{}", err);
            process::exit(1);
        }

        let mut ok = true;
        for (name, rule) in &grammar {
            if !visited.contains_key(name) {
                eprintln!("{}: {} is unused", rule.head.loc, name);
                ok = false;
            }
        }
        if !ok {
            process::exit(1);
        }
    }

    if args.dump {
        println!("{}: {}", rule.head.loc, rule);
        return;
    }

    for _ in 0..args.count {
        match generate_random_message(&grammar, &rule.body) {
            Ok(message) => println!("{}", message),
            Err(err) => {
                eprintln!("{}", err);
                process::exit(1);
            }
        }
    }
}