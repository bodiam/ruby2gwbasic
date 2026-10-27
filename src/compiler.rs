use std::collections::{HashMap, HashSet};

use crate::{
    Error,
    lexer::{Kind, Token},
    parser::{Expr, ExprKind, Stmt},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Type {
    Integer,
    Float,
    Text,
    Bool,
}

impl Type {
    fn numeric(self) -> bool {
        matches!(self, Self::Integer | Self::Float)
    }
}

struct Variable {
    name: String,
    kind: Type,
}

enum Instruction {
    Line(String),
    Label(usize),
    Goto(usize),
    Branch(String, usize),
}

#[derive(Default)]
struct Compiler {
    variables: HashMap<String, Variable>,
    assigned: HashSet<String>,
    instructions: Vec<Instruction>,
    labels: usize,
}

pub(crate) fn compile(statements: &[Stmt]) -> Result<String, Error> {
    let mut compiler = Compiler::default();
    compiler.block(statements)?;
    compiler.line("END");
    compiler.finish()
}

impl Compiler {
    fn line(&mut self, line: impl Into<String>) {
        self.instructions.push(Instruction::Line(line.into()));
    }
    fn label(&mut self) -> usize {
        let id = self.labels;
        self.labels += 1;
        id
    }
    fn mark(&mut self, label: usize) {
        self.instructions.push(Instruction::Label(label));
    }
    fn jump(&mut self, label: usize) {
        self.instructions.push(Instruction::Goto(label));
    }

    fn kind(&self, expr: &Expr) -> Result<Type, Error> {
        let fail = |message: &str| expr.token.error(message);
        match &expr.kind {
            ExprKind::Number(value) => {
                let n = value.parse::<f64>().map_err(|_| fail("invalid number"))?;
                if !n.is_finite() || n > 1.7e38 {
                    return Err(fail("number exceeds GW-BASIC's numeric range"));
                }
                if value.contains('.') {
                    Ok(Type::Float)
                } else if n > 9_007_199_254_740_991.0 {
                    Err(fail(
                        "integer literal exceeds the supported exact range (2^53 - 1)",
                    ))
                } else {
                    Ok(Type::Integer)
                }
            }
            ExprKind::Text(_) => Ok(Type::Text),
            ExprKind::Bool(_) => Ok(Type::Bool),
            ExprKind::Variable(name) => {
                if !self.assigned.contains(name) {
                    return Err(fail(&format!(
                        "variable '{name}' must be assigned on every path before use"
                    )));
                }
                Ok(self.variables[name].kind)
            }
            ExprKind::Unary(op, value) => {
                let kind = self.kind(value)?;
                if *op == "!" {
                    if kind == Type::Bool {
                        Ok(Type::Bool)
                    } else {
                        Err(fail("'!' requires a boolean condition in this subset"))
                    }
                } else if kind.numeric() {
                    Ok(kind)
                } else {
                    Err(fail("unary '+' and '-' require numbers"))
                }
            }
            ExprKind::Binary(lhs, op, rhs) => {
                let (left, right) = (self.kind(lhs)?, self.kind(rhs)?);
                match *op {
                    "&&" | "||" if left == Type::Bool && right == Type::Bool => Ok(Type::Bool),
                    "&&" | "||" => Err(fail(
                        "logical operators require boolean conditions in this subset",
                    )),
                    "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                        if (left.numeric() && right.numeric())
                            || (left == Type::Text && right == Type::Text)
                        {
                            Ok(Type::Bool)
                        } else {
                            Err(fail("comparisons require two numbers or two strings"))
                        }
                    }
                    "+" if left == Type::Text && right == Type::Text => Ok(Type::Text),
                    _ if left.numeric() && right.numeric() => {
                        if left == Type::Integer && right == Type::Integer {
                            Ok(Type::Integer)
                        } else {
                            Ok(Type::Float)
                        }
                    }
                    _ => Err(fail(
                        "arithmetic requires numbers; '+' also accepts two strings",
                    )),
                }
            }
        }
    }

    fn expression(&self, expr: &Expr) -> Result<String, Error> {
        let kind = self.kind(expr)?;
        match &expr.kind {
            ExprKind::Number(n) => Ok(format!("{n}#")),
            ExprKind::Text(s) => Ok(basic_string(s)),
            ExprKind::Variable(name) => Ok(self.variables[name].name.clone()),
            ExprKind::Unary(op, value) if *op != "!" => {
                Ok(format!("({op}{})", self.expression(value)?))
            }
            ExprKind::Binary(lhs, op, rhs) if !matches!(*op, "&&" | "||") => {
                let (a, b) = (self.expression(lhs)?, self.expression(rhs)?);
                let value = match *op {
                    "/" if kind == Type::Integer => format!("INT({a}/{b})"),
                    "%" => format!("({a}-{b}*INT({a}/{b}))"),
                    "==" => format!("({a}={b})"),
                    "!=" => format!("({a}<>{b})"),
                    _ => format!("({a}{op}{b})"),
                };
                Ok(value)
            }
            _ => Err(expr
                .token
                .error("booleans are only supported in if/while conditions")),
        }
    }

    fn condition(&mut self, expr: &Expr, yes: usize, no: usize) -> Result<(), Error> {
        if self.kind(expr)? != Type::Bool {
            return Err(expr.token.error("conditions must be comparisons or booleans; implicit Ruby truthiness is not supported"));
        }
        match &expr.kind {
            ExprKind::Bool(value) => self.jump(if *value { yes } else { no }),
            ExprKind::Unary("!", value) => self.condition(value, no, yes)?,
            ExprKind::Binary(lhs, "&&", rhs) => {
                let next = self.label();
                self.condition(lhs, next, no)?;
                self.mark(next);
                self.condition(rhs, yes, no)?;
            }
            ExprKind::Binary(lhs, "||", rhs) => {
                let next = self.label();
                self.condition(lhs, yes, next)?;
                self.mark(next);
                self.condition(rhs, yes, no)?;
            }
            _ => {
                let test = self.expression(expr)?;
                self.instructions.push(Instruction::Branch(test, yes));
                self.jump(no);
            }
        }
        Ok(())
    }

    fn block(&mut self, statements: &[Stmt]) -> Result<(), Error> {
        for statement in statements {
            match statement {
                Stmt::Assign(token, expr) => self.assign(token, expr)?,
                Stmt::Print(newline, args) => {
                    if args.is_empty() {
                        self.line(if *newline { "PRINT" } else { "PRINT \"\";" });
                    }
                    for expr in args {
                        let kind = self.kind(expr)?;
                        if kind == Type::Bool {
                            return Err(expr.token.error("printing booleans is not supported"));
                        }
                        let mut text = self.expression(expr)?;
                        if kind.numeric() {
                            self.line(format!("N#={text}"));
                            // STR$ reserves a leading sign space for nonnegative values.
                            // GW-BASIC comparisons yield -1 for true and 0 for false.
                            text = "MID$(STR$(N#),2+(N#<0#))".into();
                        }
                        if *newline
                            && kind == Type::Text
                            && !matches!(&expr.kind, ExprKind::Text(s) if !s.ends_with('\n'))
                        {
                            // Ruby puts adds a newline only if the value lacks one.
                            self.line(format!("P$={text}"));
                            self.line("PRINT P$;");
                            self.line("IF RIGHT$(P$,1)<>CHR$(10) THEN PRINT");
                        } else {
                            self.line(format!("PRINT {text}{}", if *newline { "" } else { ";" }));
                        }
                    }
                }
                Stmt::If(branches, otherwise) => {
                    let before = self.assigned.clone();
                    let end = self.label();
                    let mut assigned_after = Vec::new();
                    for (test, body) in branches {
                        self.assigned = before.clone();
                        let (yes, no) = (self.label(), self.label());
                        self.condition(test, yes, no)?;
                        self.mark(yes);
                        self.block(body)?;
                        assigned_after.push(self.assigned.clone());
                        self.jump(end);
                        self.mark(no);
                    }
                    self.assigned = before;
                    self.block(otherwise)?;
                    for assigned in assigned_after {
                        self.assigned.retain(|name| assigned.contains(name));
                    }
                    self.mark(end);
                }
                Stmt::While(test, body) => {
                    let before = self.assigned.clone();
                    let (start, yes, end) = (self.label(), self.label(), self.label());
                    self.mark(start);
                    self.condition(test, yes, end)?;
                    self.mark(yes);
                    self.block(body)?;
                    self.jump(start);
                    self.mark(end);
                    self.assigned = before;
                }
            }
        }
        Ok(())
    }

    fn assign(&mut self, token: &Token, expr: &Expr) -> Result<(), Error> {
        let Kind::Word(name) = &token.kind else {
            unreachable!()
        };
        let kind = self.kind(expr)?;
        if kind == Type::Bool {
            return Err(token.error(
                "boolean variables are not supported; use booleans directly in conditions",
            ));
        }
        let value = self.expression(expr)?;
        if let Some(variable) = self.variables.get(name) {
            if variable.kind != kind {
                return Err(token.error(format!(
                    "variable '{name}' cannot change type from {:?} to {kind:?}",
                    variable.kind
                )));
            }
        } else {
            let basic_name = format!(
                "V{}{}",
                self.variables.len() + 1,
                if kind == Type::Text { "$" } else { "#" }
            );
            self.variables.insert(
                name.clone(),
                Variable {
                    name: basic_name,
                    kind,
                },
            );
        }
        self.line(format!("{}={value}", self.variables[name].name));
        self.assigned.insert(name.clone());
        Ok(())
    }

    fn finish(self) -> Result<String, Error> {
        let mut labels = HashMap::new();
        let mut number = 10;
        for instruction in &self.instructions {
            match instruction {
                Instruction::Label(id) => {
                    labels.insert(*id, number);
                }
                _ => {
                    number += 10;
                }
            }
        }
        if number - 10 > 65_529 {
            return Err(Error::new(
                1,
                1,
                "generated program exceeds GW-BASIC's line-number range",
            ));
        }
        let mut output = String::new();
        number = 10;
        for instruction in self.instructions {
            let text = match instruction {
                Instruction::Label(_) => continue,
                Instruction::Line(text) => text,
                Instruction::Goto(id) => format!("GOTO {}", labels[&id]),
                Instruction::Branch(expr, id) => format!("IF {expr} THEN {}", labels[&id]),
            };
            let line = format!("{number} {text}");
            if line.len() > 255 {
                return Err(Error::new(
                    1,
                    1,
                    format!(
                        "generated BASIC line {number} exceeds 255 bytes; simplify the expression"
                    ),
                ));
            }
            output.push_str(&line);
            output.push('\n');
            number += 10;
        }
        Ok(output)
    }
}

fn basic_string(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".into();
    }
    let mut parts = Vec::new();
    let mut text = String::new();
    for byte in value.bytes() {
        if byte == b'"' || !(32..=126).contains(&byte) {
            if !text.is_empty() {
                parts.push(format!("\"{text}\""));
                text.clear();
            }
            parts.push(format!("CHR$({byte})"));
        } else {
            text.push(char::from(byte));
        }
    }
    if !text.is_empty() {
        parts.push(format!("\"{text}\""));
    }
    if parts.len() == 1 {
        parts.remove(0)
    } else {
        format!("({})", parts.join("+"))
    }
}
