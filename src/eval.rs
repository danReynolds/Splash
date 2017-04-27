use std::fs::File;
use std::io::{BufRead, BufReader};

use env::UserEnv;
use input::{prompt, parser, tokenizer};
use input::token::*;
use process::{self, BuiltinMap};
use util::write_err;

#[derive(Debug)]
pub enum InputReader {
    File(BufReader<File>),
    Command(Vec<String>),
    Stdin,
}

fn getline(input_method: &mut InputReader, cont: bool) -> Option<String> {
    match input_method {
        &mut InputReader::File(ref mut reader) => {
            let mut s = String::new();

            let res = reader.read_line(&mut s);
            if res.is_err() || res.unwrap() == 0 {
                None
            } else {
                Some(s)
            }
        },
        &mut InputReader::Command(ref mut lines) => {
            if lines.is_empty() {
                None
            } else {
                Some(lines.remove(0))
            }
        }
        &mut InputReader::Stdin => prompt::getline(cont),
    }
}

pub fn eval(mut input_reader: InputReader, mut builtins: BuiltinMap) {
    let mut user_env = UserEnv::new();
    let mut last_status = 0;
    let mut line = String::new();

    loop {
        let cont = !line.is_empty();
        if let Some(next_line) = getline(&mut input_reader, cont) {
            line.push_str(&next_line);
        } else {
            break;
        }

        let tokens: Vec<Token> = match tokenizer::tokenize(&line) {
            Ok(tokens) => {
                tokens
            },
            Err(e) => {
                if e != TokenError::Unterminated {
                    write_err(&format!("splash: {}", e));
                    line = String::new();
                }
                continue;
            },
        };
        line = String::new();

        let mut input: Vec<String> = Vec::new();
        let mut here_docs: Vec<(RedirOp, String)> = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            match tokens[i] {
                Token::Redir(_, ref o@RedirOp::DLESS) | Token::Redir(_, ref o@RedirOp::DLESSDASH) => {
                    if let Token::String(ref s) = tokens[i+1] {
                        here_docs.push((o.clone(), s.clone()));
                    } else {
                        write_err(&"splash: here docs must be strings".to_string());
                        continue;
                    }
                    i += 2;
                },
                _ => {
                    i += 1;
                },
            }
        }
        for (kind, here_doc) in here_docs {
            let mut content = String::new();
            loop {
                if let Some(mut s) = getline(&mut input_reader, true) {
                    if kind == RedirOp::DLESSDASH {
                        s = s.chars().skip_while(|c| c.is_whitespace()).collect::<String>();
                    }
                    if s == here_doc {
                        input.push(content);
                        break;
                    }
                    content.push_str(&s);
                    content.push_str("\n");
                } else {
                    // Replicate other shells' behaviour, just ignore this heredoc
                    error!("warning: here-document delimited by end-of-file (wanted `EOF')");
                    content.clear();
                    break;
                }
            }
        }

        let parsed = parser::parse(tokens, &mut input);

        if let Err(e) = parsed {
            write_err(&format!("splash: {}", e));
            continue;
        }

        let commands = parsed.unwrap();
        if commands.is_empty() {
            continue;
        }

        for command in commands {
            let res = process::run_processes(&mut builtins, command, &mut user_env);
            match res {
                Err(e) => {
                    write_err(&format!("splash: {}", e));
                },
                Ok(n) => {
                    last_status = n;
                },
            };
        }
    }

    ::std::process::exit(last_status);
}