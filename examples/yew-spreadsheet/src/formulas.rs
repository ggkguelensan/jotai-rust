//! A bounded arithmetic grammar. Formulas never execute host-language code.
use crate::coordinates::Address;
use std::fmt;

pub const MAX_STEPS: usize = 2048;
pub const MAX_DEPTH: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Ref,
    Cycle,
    DivideByZero,
    Value,
    Number,
    Formula,
    Limit,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ref => "#REF!",
            Self::Cycle => "#CYCLE!",
            Self::DivideByZero => "#DIV/0!",
            Self::Value => "#VALUE!",
            Self::Number => "#NUM!",
            Self::Formula => "#FORMULA!",
            Self::Limit => "#LIMIT!",
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Blank,
    Number(f64),
    Text(String),
    Error(Error),
}
impl Value {
    pub fn display(&self) -> String {
        match self {
            Self::Blank => String::new(),
            Self::Number(n) => {
                let text = format!("{n:.2}");
                text.trim_end_matches('0').trim_end_matches('.').to_string()
            }
            Self::Text(text) => text.clone(),
            Self::Error(error) => error.to_string(),
        }
    }
    fn number(self) -> Result<f64, Error> {
        match self {
            Self::Blank => Ok(0.0),
            Self::Number(n) => Ok(n),
            Self::Error(e) => Err(e),
            _ => Err(Error::Value),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Literal(Value),
    Reference(Address),
    Range(Address, Address),
    Negative(Box<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
    Sum(Vec<Expr>),
}
#[derive(Debug, PartialEq)]
enum Token {
    Number(f64),
    Word(String),
    Symbol(char),
}

pub fn parse(input: &str) -> Result<Expr, Error> {
    let trimmed = input.trim();
    if !trimmed.starts_with('=') {
        return Ok(Expr::Literal(if trimmed.is_empty() {
            Value::Blank
        } else if let Ok(n) = trimmed.parse::<f64>()
            && n.is_finite()
        {
            Value::Number(n)
        } else {
            Value::Text(input.to_string())
        }));
    }
    if input.len() > 512 {
        return Err(Error::Formula);
    }
    let source = trimmed[1..].to_ascii_uppercase();
    let bytes = source.as_bytes();
    let (mut offset, mut tokens) = (0, Vec::new());
    while offset < bytes.len() {
        let start = offset;
        let c = bytes[offset];
        if c.is_ascii_whitespace() {
            offset += 1;
            continue;
        }
        if c.is_ascii_digit() || c == b'.' {
            while offset < bytes.len() && (bytes[offset].is_ascii_digit() || bytes[offset] == b'.')
            {
                offset += 1;
            }
            if bytes.get(offset) == Some(&b'E') {
                offset += 1;
                if matches!(bytes.get(offset), Some(b'+' | b'-')) {
                    offset += 1;
                }
                while bytes.get(offset).is_some_and(u8::is_ascii_digit) {
                    offset += 1;
                }
            }
            let number: f64 = source[start..offset].parse().map_err(|_| Error::Formula)?;
            if !number.is_finite() {
                return Err(Error::Number);
            }
            tokens.push(Token::Number(number));
        } else if c.is_ascii_alphabetic() {
            while bytes.get(offset).is_some_and(u8::is_ascii_alphanumeric) {
                offset += 1;
            }
            tokens.push(Token::Word(source[start..offset].to_string()));
        } else if b"+-*/():,".contains(&c) {
            offset += 1;
            tokens.push(Token::Symbol(c as char));
        } else {
            return Err(Error::Formula);
        }
    }
    let mut parser = Parser {
        tokens,
        cursor: 0,
        depth: 0,
    };
    let expression = parser.expression()?;
    if parser.cursor != parser.tokens.len() {
        return Err(Error::Formula);
    }
    Ok(expression)
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    depth: usize,
}
impl Parser {
    fn symbol(&mut self, c: char) -> bool {
        if self.tokens.get(self.cursor) == Some(&Token::Symbol(c)) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: char) -> Result<(), Error> {
        if self.symbol(c) {
            Ok(())
        } else {
            Err(Error::Formula)
        }
    }
    fn primary(&mut self) -> Result<Expr, Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Error::Limit);
        }
        let result = self.primary_inner();
        self.depth -= 1;
        result
    }
    fn primary_inner(&mut self) -> Result<Expr, Error> {
        if self.symbol('+') {
            return self.primary();
        }
        if self.symbol('-') {
            return Ok(Expr::Negative(Box::new(self.primary()?)));
        }
        if self.symbol('(') {
            let value = self.expression()?;
            self.expect(')')?;
            return Ok(value);
        }
        let token = self.tokens.get(self.cursor).ok_or(Error::Formula)?;
        self.cursor += 1;
        match token {
            Token::Number(n) => Ok(Expr::Literal(Value::Number(*n))),
            Token::Word(name) if name == "SUM" => {
                self.expect('(')?;
                let mut values = vec![];
                if !self.symbol(')') {
                    loop {
                        values.push(self.expression()?);
                        if !self.symbol(',') {
                            break;
                        }
                    }
                    self.expect(')')?;
                }
                Ok(Expr::Sum(values))
            }
            Token::Word(name) => {
                if !name.bytes().any(|c| c.is_ascii_digit()) {
                    return Err(Error::Formula);
                }
                let from = name.parse().map_err(|_| Error::Ref)?;
                if self.symbol(':') {
                    let Some(Token::Word(name)) = self.tokens.get(self.cursor) else {
                        return Err(Error::Formula);
                    };
                    let to = name.parse().map_err(|_| Error::Ref)?;
                    self.cursor += 1;
                    Ok(Expr::Range(from, to))
                } else {
                    Ok(Expr::Reference(from))
                }
            }
            _ => Err(Error::Formula),
        }
    }
    fn product(&mut self) -> Result<Expr, Error> {
        let mut left = self.primary()?;
        loop {
            let operator = if self.symbol('*') {
                '*'
            } else if self.symbol('/') {
                '/'
            } else {
                break;
            };
            left = Expr::Binary(operator, Box::new(left), Box::new(self.primary()?));
        }
        Ok(left)
    }
    fn expression(&mut self) -> Result<Expr, Error> {
        let mut left = self.product()?;
        loop {
            let operator = if self.symbol('+') {
                '+'
            } else if self.symbol('-') {
                '-'
            } else {
                break;
            };
            left = Expr::Binary(operator, Box::new(left), Box::new(self.product()?));
        }
        Ok(left)
    }
}

fn range(from: Address, to: Address) -> Result<Vec<Address>, Error> {
    let rows = from
        .row
        .abs_diff(to.row)
        .checked_add(1)
        .ok_or(Error::Limit)?;
    let columns = from
        .column
        .abs_diff(to.column)
        .checked_add(1)
        .ok_or(Error::Limit)?;
    if rows.checked_mul(columns).ok_or(Error::Limit)? > MAX_STEPS {
        return Err(Error::Limit);
    }
    Ok((from.row.min(to.row)..=from.row.max(to.row))
        .flat_map(|row| {
            (from.column.min(to.column)..=from.column.max(to.column))
                .map(move |column| Address::new(row, column))
        })
        .collect())
}

pub fn references(expression: &Expr) -> Result<Vec<Address>, Error> {
    let mut refs = Vec::new();
    fn collect(expr: &Expr, refs: &mut Vec<Address>) -> Result<(), Error> {
        match expr {
            Expr::Reference(address) => refs.push(*address),
            Expr::Range(from, to) => refs.extend(range(*from, *to)?),
            Expr::Negative(expr) => collect(expr, refs)?,
            Expr::Binary(_, left, right) => {
                collect(left, refs)?;
                collect(right, refs)?;
            }
            Expr::Sum(items) => {
                for item in items {
                    collect(item, refs)?;
                }
            }
            Expr::Literal(_) => {}
        }
        if refs.len() > MAX_STEPS {
            return Err(Error::Limit);
        }
        Ok(())
    }
    collect(expression, &mut refs)?;
    Ok(refs)
}

pub fn evaluate(expression: &Expr, mut read: impl FnMut(Address) -> Value) -> Value {
    fn finite(n: f64) -> Result<Value, Error> {
        if n.is_finite() {
            Ok(Value::Number(n))
        } else {
            Err(Error::Number)
        }
    }
    fn tick(steps: &mut usize) -> Result<(), Error> {
        *steps += 1;
        if *steps > MAX_STEPS {
            Err(Error::Limit)
        } else {
            Ok(())
        }
    }
    fn eval(
        expr: &Expr,
        read: &mut impl FnMut(Address) -> Value,
        steps: &mut usize,
    ) -> Result<Value, Error> {
        tick(steps)?;
        match expr {
            Expr::Literal(value) => Ok(value.clone()),
            Expr::Reference(address) => match read(*address) {
                Value::Error(e) => Err(e),
                value => Ok(value),
            },
            Expr::Range(_, _) => Err(Error::Value),
            Expr::Negative(value) => finite(-eval(value, read, steps)?.number()?),
            Expr::Binary(op, left, right) => {
                let left = eval(left, read, steps)?.number()?;
                let right = eval(right, read, steps)?.number()?;
                finite(match op {
                    '+' => left + right,
                    '-' => left - right,
                    '*' => left * right,
                    '/' if right == 0.0 => return Err(Error::DivideByZero),
                    '/' => left / right,
                    _ => return Err(Error::Formula),
                })
            }
            Expr::Sum(items) => {
                let mut sum = 0.0;
                for item in items {
                    let values = if let Expr::Range(from, to) = item {
                        let mut values = vec![];
                        for address in range(*from, *to)? {
                            tick(steps)?;
                            values.push(read(address));
                        }
                        values
                    } else {
                        vec![eval(item, read, steps)?]
                    };
                    for value in values {
                        match value {
                            Value::Number(n) => sum += n,
                            Value::Error(e) => return Err(e),
                            _ => {}
                        }
                    }
                }
                finite(sum)
            }
        }
    }
    eval(expression, &mut read, &mut 0).unwrap_or_else(Value::Error)
}
