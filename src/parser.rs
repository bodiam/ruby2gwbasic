use crate::{
    Error,
    lexer::{Kind, Token},
};

#[derive(Clone, Debug)]
pub(crate) struct Expr {
    pub kind: ExprKind,
    pub token: Token,
}

#[derive(Clone, Debug)]
pub(crate) enum ExprKind {
    Number(String),
    Text(String),
    Bool(bool),
    Variable(String),
    Unary(&'static str, Box<Expr>),
    Binary(Box<Expr>, &'static str, Box<Expr>),
}

#[derive(Debug)]
pub(crate) enum Stmt {
    Assign(Token, Expr),
    Print(bool, Vec<Expr>),
    If(Vec<(Expr, Vec<Stmt>)>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
}

pub(crate) fn parse(tokens: Vec<Token>) -> Result<Vec<Stmt>, Error> {
    let mut parser = Parser { tokens, pos: 0 };
    let statements = parser.block()?;
    if parser.current().kind != Kind::Eof {
        return Err(parser.error("unexpected block delimiter"));
    }
    Ok(statements)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }
    fn error(&self, message: impl Into<String>) -> Error {
        self.current().error(message)
    }
    fn at_word(&self, word: &str) -> bool {
        self.current().kind == Kind::Word(word.into())
    }
    fn op(&mut self, op: &str) -> bool {
        if matches!(&self.current().kind, Kind::Op(value) if *value == op) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_op(&mut self, op: &str) -> Result<(), Error> {
        if self.op(op) {
            Ok(())
        } else {
            Err(self.error(format!("expected '{op}'")))
        }
    }
    fn line_end(&mut self) -> Result<(), Error> {
        match self.current().kind {
            Kind::Newline => {
                self.pos += 1;
                Ok(())
            }
            Kind::Eof => Ok(()),
            _ => Err(self.error("expected end of line; only one statement per line is supported")),
        }
    }
    fn block(&mut self) -> Result<Vec<Stmt>, Error> {
        let mut statements = Vec::new();
        loop {
            if self.current().kind == Kind::Newline {
                self.pos += 1;
                continue;
            }
            if self.current().kind == Kind::Eof
                || ["end", "else", "elsif"].iter().any(|w| self.at_word(w))
            {
                break;
            }
            statements.push(self.statement()?);
        }
        Ok(statements)
    }
    fn end_block(&mut self) -> Result<(), Error> {
        if !self.at_word("end") {
            return Err(self.error("expected 'end' to close block"));
        }
        self.pos += 1;
        self.line_end()
    }
    fn statement(&mut self) -> Result<Stmt, Error> {
        let token = self.current().clone();
        let Kind::Word(word) = &token.kind else {
            return Err(self.error("expected an assignment, puts, print, if, or while"));
        };
        self.pos += 1;
        match word.as_str() {
            "puts" | "print" => {
                // Adjacent parentheses delimit a call; after whitespace they
                // belong to the argument expression (puts (2 + 3) * 4).
                let parens = self.current().line == token.line
                    && self.current().column == token.column + word.len()
                    && self.op("(");
                let mut args = Vec::new();
                if !matches!(
                    self.current().kind,
                    Kind::Newline | Kind::Eof | Kind::Op(")")
                ) {
                    loop {
                        args.push(self.expression(0)?);
                        if !self.op(",") {
                            break;
                        }
                    }
                }
                if parens {
                    self.expect_op(")")?;
                }
                self.line_end()?;
                Ok(Stmt::Print(word == "puts", args))
            }
            "if" => {
                let mut branches = Vec::new();
                loop {
                    let condition = self.expression(0)?;
                    self.line_end()?;
                    branches.push((condition, self.block()?));
                    if !self.at_word("elsif") {
                        break;
                    }
                    self.pos += 1;
                }
                let otherwise = if self.at_word("else") {
                    self.pos += 1;
                    self.line_end()?;
                    self.block()?
                } else {
                    Vec::new()
                };
                self.end_block()?;
                Ok(Stmt::If(branches, otherwise))
            }
            "while" => {
                let condition = self.expression(0)?;
                self.line_end()?;
                let body = self.block()?;
                self.end_block()?;
                Ok(Stmt::While(condition, body))
            }
            name if reserved(name) => {
                Err(token.error(format!("unsupported Ruby statement '{name}'")))
            }
            _ => {
                let operator = match self.current().kind {
                    Kind::Op(op @ ("=" | "+=" | "-=" | "*=" | "/=" | "%=")) => op,
                    _ => return Err(self.error("expected assignment (=, +=, -=, *=, /=, %=); method calls are not supported")),
                };
                if !word.starts_with(|c: char| c.is_ascii_lowercase() || c == '_') {
                    return Err(token.error("only local variables starting with a lowercase letter or underscore are supported"));
                }
                self.pos += 1;
                let mut value = self.expression(0)?;
                if operator != "=" {
                    let op = match operator {
                        "+=" => "+",
                        "-=" => "-",
                        "*=" => "*",
                        "/=" => "/",
                        _ => "%",
                    };
                    value = Expr {
                        kind: ExprKind::Binary(
                            Box::new(Expr {
                                kind: ExprKind::Variable(word.clone()),
                                token: token.clone(),
                            }),
                            op,
                            Box::new(value),
                        ),
                        token: token.clone(),
                    };
                }
                self.line_end()?;
                Ok(Stmt::Assign(token, value))
            }
        }
    }
    fn expression(&mut self, min_precedence: u8) -> Result<Expr, Error> {
        let token = self.current().clone();
        let kind = match &token.kind {
            Kind::Number(n) => ExprKind::Number(n.clone()),
            Kind::Text(s) => ExprKind::Text(s.clone()),
            Kind::Word(w) if w == "true" || w == "false" => ExprKind::Bool(w == "true"),
            Kind::Word(w) if !reserved(w) => ExprKind::Variable(w.clone()),
            Kind::Op(op @ ("+" | "-" | "!")) => {
                self.pos += 1;
                let rhs = self.expression(7)?;
                return self.binary_tail(
                    Expr {
                        kind: ExprKind::Unary(op, Box::new(rhs)),
                        token,
                    },
                    min_precedence,
                );
            }
            Kind::Op("(") => {
                self.pos += 1;
                let inner = self.expression(0)?;
                self.expect_op(")")?;
                return self.binary_tail(inner, min_precedence);
            }
            _ => {
                return Err(
                    self.error("expected a number, string, variable, or parenthesized expression")
                );
            }
        };
        self.pos += 1;
        self.binary_tail(Expr { kind, token }, min_precedence)
    }
    fn binary_tail(&mut self, mut lhs: Expr, min_precedence: u8) -> Result<Expr, Error> {
        while let Kind::Op(op) = self.current().kind {
            let precedence = match op {
                "||" => 1,
                "&&" => 2,
                "==" | "!=" => 3,
                "<" | "<=" | ">" | ">=" => 4,
                "+" | "-" => 5,
                "*" | "/" | "%" => 6,
                _ => break,
            };
            if precedence < min_precedence {
                break;
            }
            self.pos += 1;
            let rhs = self.expression(precedence + 1)?;
            let token = lhs.token.clone();
            lhs = Expr {
                kind: ExprKind::Binary(Box::new(lhs), op, Box::new(rhs)),
                token,
            };
        }
        Ok(lhs)
    }
}

fn reserved(word: &str) -> bool {
    matches!(
        word,
        "if" | "elsif"
            | "else"
            | "end"
            | "while"
            | "puts"
            | "print"
            | "true"
            | "false"
            | "nil"
            | "def"
            | "class"
            | "module"
            | "for"
            | "in"
            | "do"
            | "unless"
            | "until"
            | "return"
            | "break"
            | "next"
            | "then"
            | "and"
            | "or"
            | "not"
            | "begin"
            | "rescue"
            | "ensure"
            | "case"
            | "when"
            | "yield"
            | "self"
            | "super"
            | "redo"
            | "retry"
            | "alias"
            | "defined"
            | "undef"
            | "END"
            | "BEGIN"
    )
}
