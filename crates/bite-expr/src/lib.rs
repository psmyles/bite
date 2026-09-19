//! A deterministic, bounded expression language. Compiled expressions own their
//! AST; evaluation performs no parsing and has no ambient I/O or application state.
use bite_schema::ArgSpec;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
pub mod definition;

pub const MAX_SOURCE: usize = 4096;
pub const MAX_DEPTH: usize = 64;
pub const MAX_STRING: usize = 65536;
pub type Context = BTreeMap<String, Value>;
pub type TypeContext = BTreeMap<String, Type>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Vector(Vec<f64>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Null,
    Bool,
    Number,
    String,
    Vector,
    Numeric,
    Value,
}
impl Value {
    pub fn kind(&self) -> Type {
        match self {
            Self::Null => Type::Null,
            Self::Bool(_) => Type::Bool,
            Self::Int(_) | Self::Float(_) => Type::Number,
            Self::String(_) => Type::String,
            Self::Vector(_) => Type::Vector,
        }
    }
    pub fn truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(b) => *b,
            Self::Int(n) => *n != 0,
            Self::Float(n) => *n != 0.0 && !n.is_nan(),
            Self::String(s) => !s.is_empty(),
            Self::Vector(v) => v.iter().any(|n| *n != 0.0),
        }
    }
    pub fn scalar(&self) -> Result<f64, String> {
        match self {
            Self::Int(n) => Ok(*n as f64),
            Self::Float(n) => Ok(*n),
            Self::Vector(v) => Ok(v.iter().map(|n| n * n).sum::<f64>().sqrt()),
            _ => Err("expected a numeric value".into()),
        }
    }
    pub fn text(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(b) => b.to_string(),
            Self::Int(n) => n.to_string(),
            Self::Float(n) => number_text(*n),
            Self::String(s) => s.clone(),
            Self::Vector(v) => v
                .iter()
                .map(|n| number_text(*n))
                .collect::<Vec<_>>()
                .join(","),
        }
    }
}
fn number_text(n: f64) -> String {
    if n == 0.0 {
        "0".into()
    } else if n == f64::INFINITY {
        "Infinity".into()
    } else if n == f64::NEG_INFINITY {
        "-Infinity".into()
    } else {
        n.to_string()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub start: usize,
    pub end: usize,
    pub message: String,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at bytes {}..{}", self.message, self.start, self.end)
    }
}
impl std::error::Error for Error {}
fn err(start: usize, end: usize, message: impl Into<String>) -> Error {
    Error {
        start,
        end,
        message: message.into(),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    Number(Value),
    String(String),
    Ident(String),
    Op(String),
    Left,
    Right,
    Comma,
    Dot,
    End,
}
#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
}
fn lex(source: &str) -> Result<Vec<Token>, Error> {
    if source.len() > MAX_SOURCE {
        return Err(err(0, source.len(), "expression exceeds 4096 bytes"));
    }
    let mut tokens = Vec::new();
    let mut i = 0;
    let b = source.as_bytes();
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let kind = match b[i] {
            b'0'..=b'9' => {
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                if i < b.len() && b[i] == b'.' {
                    i += 1;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                if i < b.len() && matches!(b[i], b'e' | b'E') {
                    i += 1;
                    if i < b.len() && matches!(b[i], b'+' | b'-') {
                        i += 1;
                    }
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let text = &source[start..i];
                let value = if let Ok(n) = text.parse::<i64>() {
                    Value::Int(n)
                } else {
                    let n = text
                        .parse::<f64>()
                        .map_err(|_| err(start, i, "invalid number"))?;
                    if !n.is_finite() {
                        return Err(err(start, i, "numeric literal is not finite"));
                    }
                    Value::Float(n)
                };
                TokenKind::Number(value)
            }
            b'\'' | b'"' => {
                let quote = b[i];
                i += 1;
                let mut s = String::new();
                let mut closed = false;
                while i < b.len() {
                    if b[i] == quote {
                        i += 1;
                        closed = true;
                        break;
                    }
                    if b[i] == b'\\' {
                        i += 1;
                        if i == b.len() {
                            break;
                        }
                        let c = match b[i] {
                            b'n' => '\n',
                            b'r' => '\r',
                            b't' => '\t',
                            b'\\' => '\\',
                            b'\'' => '\'',
                            b'"' => '"',
                            _ => return Err(err(i - 1, i + 1, "unsupported escape")),
                        };
                        s.push(c);
                        i += 1;
                    } else {
                        let c = source[i..].chars().next().unwrap();
                        s.push(c);
                        i += c.len_utf8();
                    }
                }
                if !closed {
                    return Err(err(start, i, "unterminated string"));
                }
                TokenKind::String(s)
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                i += 1;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                TokenKind::Ident(source[start..i].into())
            }
            b'(' => {
                i += 1;
                TokenKind::Left
            }
            b')' => {
                i += 1;
                TokenKind::Right
            }
            b',' => {
                i += 1;
                TokenKind::Comma
            }
            b'.' => {
                i += 1;
                TokenKind::Dot
            }
            b'+' | b'-' | b'*' | b'/' | b'%' | b'!' | b'<' | b'>' | b'=' | b'&' | b'|' => {
                i += 1;
                if i < b.len()
                    && ((matches!(b[start], b'!' | b'<' | b'>' | b'=') && b[i] == b'=')
                        || (matches!(b[start], b'&' | b'|') && b[i] == b[start]))
                {
                    i += 1;
                }
                let op = &source[start..i];
                if ["=", "&", "|"].contains(&op) {
                    return Err(err(
                        start,
                        i,
                        "assignment and bitwise operators are not supported",
                    ));
                }
                TokenKind::Op(op.into())
            }
            _ => return Err(err(start, start + 1, "unexpected character")),
        };
        tokens.push(Token {
            kind,
            start,
            end: i,
        });
    }
    tokens.push(Token {
        kind: TokenKind::End,
        start: i,
        end: i,
    });
    Ok(tokens)
}
#[derive(Debug, Clone)]
enum NodeKind {
    Literal(Value),
    Variable(String),
    Unary(String, Box<Node>),
    Binary(String, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}
#[derive(Debug, Clone)]
struct Node {
    kind: NodeKind,
    start: usize,
    end: usize,
}
impl Node {
    fn error(&self, message: impl Into<String>) -> Error {
        err(self.start, self.end, message)
    }
}
struct Parser {
    tokens: Vec<Token>,
    i: usize,
}
impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.i]
    }
    fn next(&mut self) -> Token {
        let t = self.tokens[self.i].clone();
        self.i += 1;
        t
    }
    fn parse(&mut self, minimum: u8, depth: usize) -> Result<Node, Error> {
        if depth > MAX_DEPTH {
            return Err(err(
                self.peek().start,
                self.peek().end,
                "expression nesting exceeds 64",
            ));
        }
        let t = self.next();
        let mut left = match t.kind {
            TokenKind::Number(v) => Node {
                kind: NodeKind::Literal(v),
                start: t.start,
                end: t.end,
            },
            TokenKind::String(s) => Node {
                kind: NodeKind::Literal(Value::String(s)),
                start: t.start,
                end: t.end,
            },
            TokenKind::Ident(mut name) => {
                if [
                    "let", "const", "var", "for", "while", "function", "return", "import",
                    "require",
                ]
                .contains(&name.as_str())
                {
                    return Err(err(
                        t.start,
                        t.end,
                        "statements and scripting are not supported",
                    ));
                }
                let mut end = t.end;
                while self.peek().kind == TokenKind::Dot {
                    self.next();
                    let member = self.next();
                    if let TokenKind::Ident(s) = member.kind {
                        name.push('.');
                        name.push_str(&s);
                        end = member.end;
                    } else {
                        return Err(err(member.start, member.end, "expected member name"));
                    }
                }
                if self.peek().kind == TokenKind::Left {
                    self.next();
                    let mut args = Vec::new();
                    if self.peek().kind != TokenKind::Right {
                        loop {
                            args.push(self.parse(0, depth + 1)?);
                            if self.peek().kind != TokenKind::Comma {
                                break;
                            }
                            self.next();
                        }
                    }
                    let close = self.next();
                    if close.kind != TokenKind::Right {
                        return Err(err(close.start, close.end, "expected ')'"));
                    }
                    Node {
                        kind: NodeKind::Call(name, args),
                        start: t.start,
                        end: close.end,
                    }
                } else {
                    let kind = match name.as_str() {
                        "true" => NodeKind::Literal(Value::Bool(true)),
                        "false" => NodeKind::Literal(Value::Bool(false)),
                        "null" => NodeKind::Literal(Value::Null),
                        _ => NodeKind::Variable(name),
                    };
                    Node {
                        kind,
                        start: t.start,
                        end,
                    }
                }
            }
            TokenKind::Op(op) if ["!", "-", "+"].contains(&op.as_str()) => {
                let child = self.parse(7, depth + 1)?;
                Node {
                    start: t.start,
                    end: child.end,
                    kind: NodeKind::Unary(op, Box::new(child)),
                }
            }
            TokenKind::Left => {
                let mut inner = self.parse(0, depth + 1)?;
                let close = self.next();
                if close.kind != TokenKind::Right {
                    return Err(err(close.start, close.end, "expected ')'"));
                }
                inner.start = t.start;
                inner.end = close.end;
                inner
            }
            _ => return Err(err(t.start, t.end, "expected expression")),
        };
        while let TokenKind::Op(op) = &self.peek().kind {
            let precedence = match op.as_str() {
                "||" => 1,
                "&&" => 2,
                "==" | "!=" => 3,
                "<" | "<=" | ">" | ">=" => 4,
                "+" | "-" => 5,
                "*" | "/" | "%" => 6,
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            let op = op.clone();
            self.next();
            let right = self.parse(precedence + 1, depth + 1)?;
            left = Node {
                start: left.start,
                end: right.end,
                kind: NodeKind::Binary(op, Box::new(left), Box::new(right)),
            };
        }
        Ok(left)
    }
}
#[derive(Debug, Clone)]
pub struct Expression {
    ast: Node,
    pub result_type: Type,
    pub metadata: BTreeSet<String>,
}
impl Expression {
    pub fn compile(source: &str, types: &TypeContext) -> Result<Self, Error> {
        let mut parser = Parser {
            tokens: lex(source)?,
            i: 0,
        };
        let ast = parser.parse(0, 0)?;
        if parser.peek().kind != TokenKind::End {
            return Err(err(
                parser.peek().start,
                parser.peek().end,
                "unexpected token after expression",
            ));
        }
        let mut metadata = BTreeSet::new();
        let result_type = check(&ast, types, &mut metadata, 0)?;
        Ok(Self {
            ast,
            result_type,
            metadata,
        })
    }
    pub fn evaluate(&self, context: &Context) -> Result<Value, Error> {
        evaluate(&self.ast, context)
    }
}
fn numeric(t: Type) -> bool {
    matches!(t, Type::Number | Type::Vector | Type::Numeric | Type::Value)
}
fn merged(a: Type, b: Type) -> Type {
    if a == b {
        a
    } else if numeric(a) && numeric(b) {
        Type::Numeric
    } else {
        Type::Value
    }
}
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<_> = b.chars().collect();
    let mut row: Vec<_> = (0..=b.len()).collect();
    for (i, ac) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, bc) in b.iter().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (row[j] + 1)
                .min(old + 1)
                .min(previous + usize::from(ac != *bc));
            previous = old;
        }
    }
    row[b.len()]
}
fn arity(name: &str) -> Option<(usize, usize)> {
    Some(match name {
        "abs" | "floor" | "ceil" | "round" | "sqrt" | "sin" | "cos" | "tan" | "str" | "int"
        | "float" | "bool" | "length" | "normalize" | "rgba" => (1, 1),
        "pow" | "dot" => (2, 2),
        "min" | "max" => (2, 16),
        "clamp" | "lerp" | "if" => (3, 3),
        "format" => (1, 16),
        "vec2" => (2, 2),
        "vec3" => (3, 3),
        "vec4" => (4, 4),
        // Required by the shipped text_filter compute node.
        "lower" | "strip_extension" | "dirname" | "power_of_two" | "rational" | "parse_int" => {
            (1, 1)
        }
        "starts_with" | "ends_with" | "contains" => (2, 2),
        _ => return None,
    })
}
fn check(
    n: &Node,
    types: &TypeContext,
    metadata: &mut BTreeSet<String>,
    depth: usize,
) -> Result<Type, Error> {
    if depth > MAX_DEPTH {
        return Err(n.error("AST depth exceeds 64"));
    }
    match &n.kind {
        NodeKind::Literal(v) => Ok(v.kind()),
        NodeKind::Variable(name) => {
            if let Some(t) = types.get(name) {
                if name.starts_with("image.") {
                    metadata.insert(name.clone());
                }
                return Ok(*t);
            }
            if let Some((root, member)) = name.split_once('.') {
                if types
                    .get(root)
                    .is_some_and(|t| matches!(t, Type::Vector | Type::Numeric | Type::Value))
                    && ["x", "y", "z", "w"].contains(&member)
                {
                    return Ok(Type::Number);
                }
                return Err(n.error(format!("invalid member access {name:?}")));
            }
            let suggestion = types
                .keys()
                .filter(|k| distance(name, k) <= 2)
                .min_by_key(|k| distance(name, k));
            Err(n.error(format!(
                "unknown identifier {name:?}{}",
                suggestion
                    .map(|s| format!("; did you mean {s:?}?"))
                    .unwrap_or_default()
            )))
        }
        NodeKind::Unary(op, child) => {
            let t = check(child, types, metadata, depth + 1)?;
            if op == "!" {
                Ok(Type::Bool)
            } else if numeric(t) {
                Ok(t)
            } else {
                Err(n.error("unary arithmetic requires numeric operand"))
            }
        }
        NodeKind::Binary(op, a, b) => {
            let a = check(a, types, metadata, depth + 1)?;
            let b = check(b, types, metadata, depth + 1)?;
            match op.as_str() {
                "&&" | "||" | "==" | "!=" => Ok(Type::Bool),
                "+" if a == Type::String && b == Type::String => Ok(Type::String),
                _ if numeric(a) && numeric(b) => {
                    if ["<", "<=", ">", ">="].contains(&op.as_str()) {
                        Ok(Type::Bool)
                    } else {
                        Ok(merged(a, b))
                    }
                }
                _ => Err(n.error("incompatible operand types")),
            }
        }
        NodeKind::Call(name, args) => {
            let Some((min, max)) = arity(name) else {
                return Err(n.error(format!("unknown function {name:?}")));
            };
            if args.len() < min || args.len() > max {
                return Err(n.error(format!(
                    "{name} expects {min}..{max} arguments, got {}",
                    args.len()
                )));
            }
            let ts: Vec<Type> = args
                .iter()
                .map(|a| check(a, types, metadata, depth + 1))
                .collect::<Result<_, _>>()?;
            match name.as_str() {
                "if" => Ok(merged(ts[1], ts[2])),
                "str" => Ok(Type::String),
                "bool" => Ok(Type::Bool),
                "int" | "float" => Ok(Type::Number),
                "power_of_two" if numeric(ts[0]) => Ok(Type::Bool),
                "strip_extension" | "dirname" | "rational" | "parse_int" => {
                    if ts[0] == Type::String {
                        Ok(if name == "rational" || name == "parse_int" {
                            Type::Number
                        } else {
                            Type::String
                        })
                    } else {
                        Err(n.error("metadata conversion requires a string"))
                    }
                }
                "format" => {
                    if ts[0] == Type::String {
                        Ok(Type::String)
                    } else {
                        Err(n.error("format requires a string template"))
                    }
                }
                "rgba" => {
                    if matches!(ts[0], Type::Vector | Type::String | Type::Value) {
                        Ok(Type::String)
                    } else {
                        Err(n.error("rgba requires a color string or vector"))
                    }
                }
                "lower" | "starts_with" | "ends_with" | "contains" => {
                    if ts.iter().all(|t| *t == Type::String || *t == Type::Value) {
                        Ok(if name == "lower" {
                            Type::String
                        } else {
                            Type::Bool
                        })
                    } else {
                        Err(n.error("text functions require strings"))
                    }
                }
                _ if ts.iter().all(|t| numeric(*t)) => Ok(match name.as_str() {
                    "vec2" | "vec3" | "vec4" => Type::Vector,
                    "length" | "dot" => Type::Number,
                    _ => ts.into_iter().reduce(merged).unwrap(),
                }),
                _ => Err(n.error(format!("{name} requires numeric arguments"))),
            }
        }
    }
}
fn broadcast(
    a: &Value,
    b: &Value,
    identity: f64,
    f: impl Fn(f64, f64) -> f64,
) -> Result<Value, String> {
    Ok(match (a, b) {
        (Value::Vector(a), Value::Vector(b)) => Value::Vector(
            a.iter()
                .enumerate()
                .map(|(i, x)| f(*x, b.get(i).copied().unwrap_or(identity)))
                .collect(),
        ),
        (Value::Vector(a), b) => {
            let b = b.scalar()?;
            Value::Vector(a.iter().map(|x| f(*x, b)).collect())
        }
        (a, Value::Vector(b)) => {
            let a = a.scalar()?;
            Value::Vector(b.iter().map(|x| f(a, *x)).collect())
        }
        _ => Value::Float(f(a.scalar()?, b.scalar()?)),
    })
}
fn unary(v: &Value, f: impl Fn(f64) -> f64) -> Result<Value, String> {
    Ok(match v {
        Value::Vector(v) => Value::Vector(v.iter().map(|n| f(*n)).collect()),
        _ => Value::Float(f(v.scalar()?)),
    })
}
fn evaluate(n: &Node, ctx: &Context) -> Result<Value, Error> {
    let result = match &n.kind {
        NodeKind::Literal(v) => Ok(v.clone()),
        NodeKind::Variable(name) => ctx
            .get(name)
            .cloned()
            .or_else(|| {
                name.split_once('.').and_then(|(root, member)| {
                    let i = ["x", "y", "z", "w"].iter().position(|m| *m == member)?;
                    match ctx.get(root)? {
                        Value::Vector(v) => Some(Value::Float(*v.get(i).unwrap_or(&0.0))),
                        Value::Int(v) => Some(Value::Float(if i == 0 { *v as f64 } else { 0.0 })),
                        Value::Float(v) => Some(Value::Float(if i == 0 { *v } else { 0.0 })),
                        _ => None,
                    }
                })
            })
            .ok_or_else(|| n.error(format!("missing context value {name:?}"))),
        NodeKind::Unary(op, child) => {
            let v = evaluate(child, ctx)?;
            if op == "!" {
                Ok(Value::Bool(!v.truthy()))
            } else {
                unary(&v, |x| if op == "-" { -x } else { x }).map_err(|e| n.error(e))
            }
        }
        NodeKind::Binary(op, left, right) => {
            let a = evaluate(left, ctx)?;
            if op == "&&" && !a.truthy() {
                return Ok(Value::Bool(false));
            }
            if op == "||" && a.truthy() {
                return Ok(Value::Bool(true));
            }
            let b = evaluate(right, ctx)?;
            match op.as_str() {
                "&&" | "||" => Ok(Value::Bool(b.truthy())),
                "==" | "!=" => {
                    let equal = if numeric(a.kind()) && numeric(b.kind()) {
                        a.scalar().unwrap() == b.scalar().unwrap()
                    } else {
                        a == b
                    };
                    Ok(Value::Bool(if op == "==" { equal } else { !equal }))
                }
                "<" | "<=" | ">" | ">=" => {
                    let a = a.scalar().map_err(|e| n.error(e))?;
                    let b = b.scalar().map_err(|e| n.error(e))?;
                    Ok(Value::Bool(match op.as_str() {
                        "<" => a < b,
                        "<=" => a <= b,
                        ">" => a > b,
                        _ => a >= b,
                    }))
                }
                "+" if matches!((&a, &b), (Value::String(_), Value::String(_))) => {
                    Ok(Value::String(a.text() + &b.text()))
                }
                _ => broadcast(&a, &b, if op == "*" { 1.0 } else { 0.0 }, |a, b| {
                    match op.as_str() {
                        "+" => a + b,
                        "-" => a - b,
                        "*" => a * b,
                        "/" => {
                            if b == 0.0 {
                                0.0
                            } else {
                                a / b
                            }
                        }
                        "%" => {
                            if b == 0.0 {
                                0.0
                            } else {
                                a % b
                            }
                        }
                        _ => unreachable!(),
                    }
                })
                .map_err(|e| n.error(e)),
            }
        }
        NodeKind::Call(name, args) => {
            if name == "if" {
                return evaluate(
                    &args[if evaluate(&args[0], ctx)?.truthy() {
                        1
                    } else {
                        2
                    }],
                    ctx,
                );
            }
            let args: Vec<_> = args
                .iter()
                .map(|a| evaluate(a, ctx))
                .collect::<Result<_, _>>()?;
            call(name, &args).map_err(|e| n.error(e))
        }
    }?;
    if matches!(&result,Value::String(s) if s.len() > MAX_STRING) {
        return Err(n.error("string result exceeds 65536 bytes"));
    }
    if matches!(&result,Value::Vector(v) if v.is_empty() || v.len() > 4) {
        return Err(n.error("vectors must contain 1..4 components"));
    }
    Ok(result)
}
fn call(name: &str, a: &[Value]) -> Result<Value, String> {
    Ok(match name {
        "str" => Value::String(a[0].text()),
        "bool" => Value::Bool(a[0].truthy()),
        "strip_extension" => {
            let s = a[0].text();
            Value::String(match s.rfind('.') {
                Some(i) if i + 1 < s.len() => s[..i].into(),
                _ => s,
            })
        }
        "dirname" => {
            let s = a[0].text();
            Value::String(match s.rfind(['/', '\\']) {
                Some(0) => s[..1].into(),
                Some(i) => s[..i].into(),
                None => ".".into(),
            })
        }
        "power_of_two" => {
            let n = a[0].scalar()?;
            let bits = n as u32;
            Value::Bool(n > 0.0 && (bits & bits.wrapping_sub(1)) == 0)
        }
        "rational" | "parse_int" => {
            let s = a[0].text();
            let s = s.trim_start();
            let parse_prefix = |s: &str| {
                let mut last = 0.0;
                for (i, _) in s
                    .char_indices()
                    .skip(1)
                    .chain(std::iter::once((s.len(), ' ')))
                {
                    if let Ok(n) = s[..i].parse::<f64>() {
                        last = n;
                    }
                }
                last
            };
            let v = if name == "rational" {
                if let Some((a, b)) = s.split_once('/') {
                    match (a.parse::<u64>(), b.parse::<u64>()) {
                        (Ok(a), Ok(b)) => {
                            if b == 0 {
                                0.0
                            } else {
                                a as f64 / b as f64
                            }
                        }
                        _ => parse_prefix(s),
                    }
                } else {
                    parse_prefix(s)
                }
            } else {
                let end = s
                    .char_indices()
                    .find(|(i, c)| !c.is_ascii_digit() && !(*i == 0 && (*c == '+' || *c == '-')))
                    .map_or(s.len(), |(i, _)| i);
                s[..end].parse::<f64>().unwrap_or(0.0)
            };
            Value::Float(v)
        }
        "float" | "int" => {
            let n = match &a[0] {
                Value::Null => 0.0,
                Value::Bool(b) => {
                    if *b {
                        1.0
                    } else {
                        0.0
                    }
                }
                Value::String(s) => {
                    if s.trim().is_empty() {
                        0.0
                    } else {
                        s.trim()
                            .parse()
                            .map_err(|_| "cannot convert string to number")?
                    }
                }
                v => v.scalar()?,
            };
            if name == "int" {
                if !n.is_finite() || n < i64::MIN as f64 || n >= -(i64::MIN as f64) {
                    return Err("integer conversion out of range".into());
                }
                Value::Int(n.trunc() as i64)
            } else {
                Value::Float(n)
            }
        }
        "format" => {
            let Value::String(template) = &a[0] else {
                return Err("format requires a string".into());
            };
            let parts: Vec<_> = template.split("{}").collect();
            if parts.len() != a.len() {
                return Err("format placeholder count does not match arguments".into());
            }
            let mut s = parts[0].to_owned();
            for (part, value) in parts[1..].iter().zip(&a[1..]) {
                s.push_str(&value.text());
                s.push_str(part);
                if s.len() > MAX_STRING {
                    return Err("formatted string exceeds limit".into());
                }
            }
            Value::String(s)
        }
        "vec2" | "vec3" | "vec4" => {
            Value::Vector(a.iter().map(|v| v.scalar()).collect::<Result<_, _>>()?)
        }
        "rgba" => match &a[0] {
            Value::String(s) => Value::String(s.clone()),
            Value::Vector(v) if v.len() == 4 => Value::String(format!(
                "rgba({},{},{},{})",
                number_text((v[0].clamp(0.0, 1.0) * 255.0).round()),
                number_text((v[1].clamp(0.0, 1.0) * 255.0).round()),
                number_text((v[2].clamp(0.0, 1.0) * 255.0).round()),
                number_text(v[3].clamp(0.0, 1.0))
            )),
            _ => return Err("rgba requires four components".into()),
        },
        "length" => Value::Float(a[0].scalar()?.abs()),
        "normalize" => {
            let length = a[0].scalar()?.abs();
            unary(&a[0], |n| if length == 0.0 { 0.0 } else { n / length })?
        }
        "dot" => {
            let av = if let Value::Vector(v) = &a[0] {
                v.clone()
            } else {
                vec![a[0].scalar()?]
            };
            let bv = if let Value::Vector(v) = &a[1] {
                v.clone()
            } else {
                vec![a[1].scalar()?]
            };
            Value::Float(av.iter().zip(bv.iter()).map(|(a, b)| a * b).sum())
        }
        "pow" => {
            let exp = a[1].scalar()?;
            unary(&a[0], |n| n.powf(exp))?
        }
        "lerp" => {
            let t = a[2].scalar()?;
            match (&a[0], &a[1]) {
                (Value::Vector(av), Value::Vector(bv)) => Value::Vector(
                    av.iter()
                        .enumerate()
                        .map(|(i, x)| x + (bv.get(i).unwrap_or(x) - x) * t)
                        .collect(),
                ),
                _ => broadcast(&a[0], &a[1], 0.0, |x, y| x + (y - x) * t)?,
            }
        }
        "min" | "max" => {
            let mut v = a[0].clone();
            for b in &a[1..] {
                v = broadcast(
                    &v,
                    b,
                    0.0,
                    |x, y| if name == "min" { x.min(y) } else { x.max(y) },
                )?;
            }
            v
        }
        "clamp" => {
            let lo = a[1].scalar()?;
            let hi = a[2].scalar()?;
            if lo > hi || lo.is_nan() || hi.is_nan() {
                return Err("clamp minimum exceeds maximum or is NaN".into());
            }
            unary(&a[0], |n| n.clamp(lo, hi))?
        }
        "lower" => match &a[0] {
            Value::String(s) => Value::String(s.to_lowercase()),
            _ => return Err("lower requires string".into()),
        },
        "starts_with" | "ends_with" | "contains" => {
            let (Value::String(s), Value::String(p)) = (&a[0], &a[1]) else {
                return Err("text functions require strings".into());
            };
            Value::Bool(match name {
                "starts_with" => s.starts_with(p),
                "ends_with" => s.ends_with(p),
                _ => s.contains(p),
            })
        }
        _ => unary(&a[0], |n| match name {
            "abs" => n.abs(),
            "floor" => n.floor(),
            "ceil" => n.ceil(),
            "round" => (n + 0.5).floor(),
            "sqrt" => n.sqrt(),
            "sin" => n.sin(),
            "cos" => n.cos(),
            "tan" => n.tan(),
            _ => unreachable!(),
        })?,
    })
}

#[derive(Debug, Clone)]
pub enum CompiledArg {
    Literal(String),
    Expression(Expression),
    Conditional(Expression, Vec<CompiledArg>),
    Switch(
        Expression,
        BTreeMap<String, Vec<CompiledArg>>,
        Vec<CompiledArg>,
    ),
}
impl CompiledArg {
    pub fn compile_all(args: &[ArgSpec], types: &TypeContext) -> Result<Vec<Self>, Error> {
        Self::compile_depth(args, types, 0)
    }
    fn compile_depth(
        args: &[ArgSpec],
        types: &TypeContext,
        depth: usize,
    ) -> Result<Vec<Self>, Error> {
        if depth > 32 {
            return Err(err(0, 0, "argument nesting exceeds 32"));
        }
        args.iter()
            .map(|a| {
                Ok(match a {
                    ArgSpec::Literal(s) => Self::Literal(s.clone()),
                    ArgSpec::Expression { expr } => {
                        Self::Expression(Expression::compile(expr, types)?)
                    }
                    ArgSpec::Conditional { when, args } => Self::Conditional(
                        Expression::compile(when, types)?,
                        Self::compile_depth(args, types, depth + 1)?,
                    ),
                    ArgSpec::Switch {
                        switch,
                        cases,
                        default,
                    } => Self::Switch(
                        Expression::compile(switch, types)?,
                        cases
                            .iter()
                            .map(|(key, args)| {
                                Ok((key.clone(), Self::compile_depth(args, types, depth + 1)?))
                            })
                            .collect::<Result<_, Error>>()?,
                        Self::compile_depth(default, types, depth + 1)?,
                    ),
                })
            })
            .collect()
    }
    pub fn resolve_all(args: &[Self], ctx: &Context) -> Result<Vec<String>, Error> {
        let mut result = Vec::new();
        for a in args {
            match a {
                Self::Literal(s) => result.push(s.clone()),
                Self::Expression(e) => result.push(e.evaluate(ctx)?.text()),
                Self::Conditional(e, args) => {
                    if e.evaluate(ctx)?.truthy() {
                        result.extend(Self::resolve_all(args, ctx)?);
                    }
                }
                Self::Switch(e, cases, default) => result.extend(Self::resolve_all(
                    cases.get(&e.evaluate(ctx)?.text()).unwrap_or(default),
                    ctx,
                )?),
            }
        }
        Ok(result)
    }
    pub fn metadata(&self) -> BTreeSet<String> {
        match self {
            Self::Literal(_) => BTreeSet::new(),
            Self::Expression(e) => e.metadata.clone(),
            Self::Conditional(e, args) => e
                .metadata
                .iter()
                .cloned()
                .chain(args.iter().flat_map(Self::metadata))
                .collect(),
            Self::Switch(e, cases, default) => e
                .metadata
                .iter()
                .cloned()
                .chain(
                    cases
                        .values()
                        .flatten()
                        .chain(default)
                        .flat_map(Self::metadata),
                )
                .collect(),
        }
    }
}
