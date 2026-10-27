use crate::Error;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Word(String),
    Number(String),
    Text(String),
    Op(&'static str),
    Newline,
    Eof,
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub kind: Kind,
    pub line: usize,
    pub column: usize,
}

pub(crate) fn lex(source: &str) -> Result<Vec<Token>, Error> {
    let chars: Vec<char> = source.chars().collect();
    let (mut i, mut line, mut column) = (0, 1, 1);
    let mut tokens = Vec::new();
    while i < chars.len() {
        let (start_line, start_column) = (line, column);
        let c = chars[i];
        let kind = match c {
            ' ' | '\t' => {
                i += 1;
                column += 1;
                continue;
            }
            '#' => {
                while i < chars.len() && !matches!(chars[i], '\n' | '\r') {
                    i += 1;
                    column += 1;
                }
                continue;
            }
            '\n' | '\r' => {
                i += 1;
                if c == '\r' && chars.get(i) == Some(&'\n') {
                    i += 1;
                }
                line += 1;
                column = 1;
                Kind::Newline
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                    column += 1;
                }
                Kind::Word(chars[start..i].iter().collect())
            }
            '0'..='9' => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                    column += 1;
                }
                if chars.get(i) == Some(&'.') && chars.get(i + 1).is_some_and(char::is_ascii_digit)
                {
                    i += 1;
                    column += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                        column += 1;
                    }
                }
                Kind::Number(chars[start..i].iter().collect())
            }
            '\'' | '"' => {
                let quote = c;
                let mut value = String::new();
                i += 1;
                column += 1;
                while i < chars.len() && chars[i] != quote {
                    let ch = chars[i];
                    if matches!(ch, '\r' | '\n') {
                        return Err(Error::new(
                            start_line,
                            start_column,
                            "multiline strings are not supported",
                        ));
                    }
                    if ch == '#' && quote == '"' && chars.get(i + 1) == Some(&'{') {
                        return Err(Error::new(
                            line,
                            column,
                            "string interpolation is not supported; use string concatenation",
                        ));
                    }
                    if ch == '\\' {
                        i += 1;
                        column += 1;
                        let next = *chars.get(i).ok_or_else(|| {
                            Error::new(start_line, start_column, "unterminated string")
                        })?;
                        match next {
                            '\\' => value.push('\\'),
                            '\'' if quote == '\'' => value.push('\''),
                            '"' if quote == '"' => value.push('"'),
                            'n' if quote == '"' => value.push('\n'),
                            'r' if quote == '"' => value.push('\r'),
                            't' if quote == '"' => value.push('\t'),
                            _ if quote == '\'' && !matches!(next, '\r' | '\n') => {
                                value.push('\\');
                                value.push(next);
                            }
                            _ => return Err(Error::new(line, column, "unsupported string escape")),
                        }
                    } else {
                        value.push(ch);
                    }
                    i += 1;
                    column += 1;
                }
                if i == chars.len() {
                    return Err(Error::new(start_line, start_column, "unterminated string"));
                }
                if !value.is_ascii() {
                    return Err(Error::new(
                        start_line,
                        start_column,
                        "only ASCII strings are supported",
                    ));
                }
                if value.len() > 255 {
                    return Err(Error::new(
                        start_line,
                        start_column,
                        "GW-BASIC strings cannot exceed 255 bytes",
                    ));
                }
                i += 1;
                column += 1;
                Kind::Text(value)
            }
            _ => {
                let pair = chars.get(i + 1).map(|next| format!("{c}{next}"));
                let double = match pair.as_deref() {
                    Some("==") => Some("=="),
                    Some("!=") => Some("!="),
                    Some("<=") => Some("<="),
                    Some(">=") => Some(">="),
                    Some("+=") => Some("+="),
                    Some("-=") => Some("-="),
                    Some("*=") => Some("*="),
                    Some("/=") => Some("/="),
                    Some("%=") => Some("%="),
                    Some("&&") => Some("&&"),
                    Some("||") => Some("||"),
                    _ => None,
                };
                if let Some(op) = double {
                    i += 2;
                    column += 2;
                    Kind::Op(op)
                } else {
                    let op = match c {
                        '+' => "+",
                        '-' => "-",
                        '*' => "*",
                        '/' => "/",
                        '%' => "%",
                        '=' => "=",
                        '<' => "<",
                        '>' => ">",
                        '!' => "!",
                        '(' => "(",
                        ')' => ")",
                        ',' => ",",
                        _ => {
                            return Err(Error::new(
                                line,
                                column,
                                format!("unsupported character '{c}'"),
                            ));
                        }
                    };
                    i += 1;
                    column += 1;
                    Kind::Op(op)
                }
            }
        };
        tokens.push(Token {
            kind,
            line: start_line,
            column: start_column,
        });
    }
    tokens.push(Token {
        kind: Kind::Eof,
        line,
        column,
    });
    Ok(tokens)
}
