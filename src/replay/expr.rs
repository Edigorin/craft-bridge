//! Expression handling for the replay plan: one bounded parser producing an
//! AST, a dimensional evaluator, and a translator to SolidWorks equation
//! syntax.
//!
//! Semantics follow SolidCraft's expression rules as pinned in
//! docs/mapping.md: numeric literals are *poly* (their dimension comes from
//! context — bare `2` in `2 * width` doubles a length rather than being two
//! millimetres), an explicit unit fixes a literal's dimension immediately,
//! parameters carry their own unit, and a bare result is read in the caller's
//! context default. Anything outside the subset (functions, implicit
//! multiplication, unknown names at evaluation) is an error with a reason:
//! never guessed, never injected.
//!
//! Hostile input: expressions come from artifacts. Length, token count and
//! nesting depth are capped before parsing; identifiers are validated against
//! a strict grammar; division by zero and non-finite results are rejected.

use std::fmt;

/// Hard caps for one expression.
pub const MAX_EXPR_CHARS: usize = 400;
pub const MAX_EXPR_TOKENS: usize = 128;
pub const MAX_EXPR_DEPTH: usize = 16;
pub const MAX_IDENT_CHARS: usize = 64;

/// The dimension of a quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dim {
    Count,
    Length,
    Angle,
}

/// A dimensioned value in SI: metres for lengths, radians for angles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity {
    pub value: f64,
    pub dim: Dim,
}

/// What a bare numeric literal (and a bare result) means in the caller's
/// context: lengths read as millimetres for feature literals, as the design
/// length unit for non-literal expressions, angles as radians, counts as
/// counts.
#[derive(Debug, Clone, Copy)]
pub struct Context {
    /// SI metres per bare length unit (0.001 for millimetres).
    pub bare_length: f64,
    /// SI radians per bare angle unit (1 for radians, pi/180 for degrees).
    pub bare_angle: f64,
    /// The dimension a bare result takes.
    pub bare_dim: Dim,
    /// The unit text a bare length literal is suffixed with on translation,
    /// so the equation means the same thing whatever the SolidWorks part's
    /// document units are.
    pub bare_text: &'static str,
}

impl Context {
    pub fn millimetres() -> Self {
        Context {
            bare_length: 0.001,
            bare_angle: 1.0,
            bare_dim: Dim::Length,
            bare_text: "mm",
        }
    }

    /// The context of a parameter with its own unit: a bare number reads in
    /// that unit (SolidCraft's parameter rule).
    pub fn for_unit(unit: &str, design_length: f64, design_text: &'static str) -> Self {
        match unit {
            "deg" => Context {
                bare_length: design_length,
                bare_angle: std::f64::consts::PI / 180.0,
                bare_dim: Dim::Angle,
                bare_text: "deg",
            },
            "rad" => Context {
                bare_length: design_length,
                bare_angle: 1.0,
                bare_dim: Dim::Angle,
                bare_text: "rad",
            },
            "" | "text" => Context {
                bare_length: design_length,
                bare_angle: 1.0,
                bare_dim: Dim::Length,
                bare_text: design_text,
            },
            "mm" | "cm" | "m" | "in" | "ft" => Context {
                bare_length: unit_factor(unit).map(|(f, _)| f).unwrap_or(design_length),
                bare_angle: 1.0,
                bare_dim: Dim::Length,
                bare_text: match unit {
                    "cm" => "cm",
                    "m" => "m",
                    "in" => "in",
                    "ft" => "ft",
                    _ => "mm",
                },
            },
            _ => Context {
                bare_length: design_length,
                bare_angle: 1.0,
                bare_dim: Dim::Length,
                bare_text: design_text,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprError {
    TooLong(usize),
    TooManyTokens(usize),
    TooDeep(usize),
    BadNumber(String),
    BadIdentifier(String),
    Unexpected(char),
    UnexpectedEnd,
    Unsupported(&'static str),
    DimensionMismatch(String),
    DivideByZero,
    NonFinite,
    UnknownName(String),
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprError::TooLong(n) => {
                write!(f, "expression is {n} characters (limit {MAX_EXPR_CHARS})")
            }
            ExprError::TooManyTokens(n) => {
                write!(f, "expression has {n} tokens (limit {MAX_EXPR_TOKENS})")
            }
            ExprError::TooDeep(n) => {
                write!(f, "expression nests {n} deep (limit {MAX_EXPR_DEPTH})")
            }
            ExprError::BadNumber(s) => write!(f, "bad number {s:?}"),
            ExprError::BadIdentifier(s) => write!(f, "bad identifier {s:?}"),
            ExprError::Unexpected(c) => write!(f, "unexpected character {c:?}"),
            ExprError::UnexpectedEnd => write!(f, "expression ends unexpectedly"),
            ExprError::Unsupported(what) => write!(f, "unsupported in expressions: {what}"),
            ExprError::DimensionMismatch(s) => write!(f, "dimension mismatch: {s}"),
            ExprError::DivideByZero => write!(f, "division by zero"),
            ExprError::NonFinite => write!(f, "result is not finite"),
            ExprError::UnknownName(s) => write!(f, "unknown name {s:?}"),
        }
    }
}

/// The parsed expression. Literals keep their unit text so translation can
/// preserve meaning.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// A number with an optional unit suffix as written ("40 mm", "2", "3 deg").
    Number {
        text: String,
        unit: Option<String>,
    },
    Name(String),
    Unary(Box<Node>),
    Add(Box<Node>, Box<Node>),
    Sub(Box<Node>, Box<Node>),
    Mul(Box<Node>, Box<Node>),
    Div(Box<Node>, Box<Node>),
    Pow(Box<Node>, Box<Node>),
}

/// Collect every name an expression references (evaluation-time binding
/// checks use this).
pub fn names(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Number { .. } => {}
        Node::Name(n) => out.push(n.clone()),
        Node::Unary(a) => names(a, out),
        Node::Add(a, b) | Node::Sub(a, b) | Node::Mul(a, b) | Node::Div(a, b) | Node::Pow(a, b) => {
            names(a, out);
            names(b, out);
        }
    }
}

/// The unit suffixes the subset accepts, with their SI factors and dims.
fn unit_factor(unit: &str) -> Option<(f64, Dim)> {
    match unit {
        "mm" => Some((0.001, Dim::Length)),
        "cm" => Some((0.01, Dim::Length)),
        "m" => Some((1.0, Dim::Length)),
        "in" => Some((0.0254, Dim::Length)),
        "ft" => Some((0.3048, Dim::Length)),
        "deg" => Some((std::f64::consts::PI / 180.0, Dim::Angle)),
        "rad" => Some((1.0, Dim::Angle)),
        _ => None,
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_more(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.'
}

/// Parse a bounded expression. Errors are values; nothing panics.
pub fn parse(src: &str) -> Result<Node, ExprError> {
    if src.len() > MAX_EXPR_CHARS {
        return Err(ExprError::TooLong(src.len()));
    }
    let mut p = Parser {
        chars: src.chars().collect(),
        pos: 0,
        tokens: 0,
    };
    let node = p.expr(0)?;
    p.skip_ws();
    if p.pos < p.chars.len() {
        return Err(ExprError::Unexpected(p.chars[p.pos]));
    }
    Ok(node)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    tokens: usize,
}

impl Parser {
    fn skip_ws(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_ws();
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn count_token(&mut self) -> Result<(), ExprError> {
        self.tokens += 1;
        if self.tokens > MAX_EXPR_TOKENS {
            return Err(ExprError::TooManyTokens(self.tokens));
        }
        Ok(())
    }

    fn expr(&mut self, depth: usize) -> Result<Node, ExprError> {
        if depth > MAX_EXPR_DEPTH {
            return Err(ExprError::TooDeep(depth));
        }
        let mut left = self.term(depth)?;
        loop {
            match self.peek() {
                Some('+') => {
                    self.count_token()?;
                    self.bump();
                    let right = self.term(depth)?;
                    left = Node::Add(Box::new(left), Box::new(right));
                }
                Some('-') => {
                    self.count_token()?;
                    self.bump();
                    let right = self.term(depth)?;
                    left = Node::Sub(Box::new(left), Box::new(right));
                }
                _ => return Ok(left),
            }
        }
    }

    fn term(&mut self, depth: usize) -> Result<Node, ExprError> {
        let mut left = self.power(depth)?;
        loop {
            match self.peek() {
                Some('*') => {
                    self.count_token()?;
                    self.bump();
                    let right = self.power(depth)?;
                    left = Node::Mul(Box::new(left), Box::new(right));
                }
                Some('/') => {
                    self.count_token()?;
                    self.bump();
                    let right = self.power(depth)?;
                    left = Node::Div(Box::new(left), Box::new(right));
                }
                // Implicit multiplication ("2 width") is not in the subset.
                Some(c) if is_ident_start(c) => {
                    return Err(ExprError::Unsupported("implicit multiplication"))
                }
                _ => return Ok(left),
            }
        }
    }

    fn power(&mut self, depth: usize) -> Result<Node, ExprError> {
        let base = self.atom(depth)?;
        if self.peek() == Some('^') {
            self.count_token()?;
            self.bump();
            let exp = self.atom(depth)?;
            Ok(Node::Pow(Box::new(base), Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    fn atom(&mut self, depth: usize) -> Result<Node, ExprError> {
        self.count_token()?;
        match self.peek() {
            None => Err(ExprError::UnexpectedEnd),
            Some('(') => {
                self.bump();
                let inner = self.expr(depth + 1)?;
                if self.peek() != Some(')') {
                    return Err(ExprError::UnexpectedEnd);
                }
                self.bump();
                Ok(inner)
            }
            Some('-') | Some('+') => {
                let sign = self.chars[self.pos];
                self.bump();
                let inner = self.atom(depth + 1)?;
                if sign == '-' {
                    Ok(Node::Unary(Box::new(inner)))
                } else {
                    Ok(inner)
                }
            }
            Some(c) if c.is_ascii_digit() || c == '.' => self.number(),
            Some(c) if is_ident_start(c) => self.name(),
            Some(c) => Err(ExprError::Unexpected(c)),
        }
    }

    fn number(&mut self) -> Result<Node, ExprError> {
        let start = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_digit() || self.chars[self.pos] == '.')
        {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        let value: f64 = text
            .parse()
            .map_err(|_| ExprError::BadNumber(text.clone()))?;
        if !value.is_finite() {
            return Err(ExprError::NonFinite);
        }
        // Optional unit suffix directly after the number: "40 mm", "3deg".
        let save = self.pos;
        self.skip_ws();
        let unit_start = self.pos;
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_alphabetic() {
            self.pos += 1;
        }
        let unit: String = self.chars[unit_start..self.pos].iter().collect();
        let unit = if unit.is_empty() {
            self.pos = save;
            None
        } else {
            if unit_factor(&unit).is_none() {
                self.pos = save;
                return Err(ExprError::Unsupported("unit suffix"));
            }
            Some(unit)
        };
        Ok(Node::Number { text, unit })
    }

    fn name(&mut self) -> Result<Node, ExprError> {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.chars.len() && is_ident_more(self.chars[self.pos]) {
            self.pos += 1;
        }
        let name: String = self.chars[start..self.pos].iter().collect();
        if name.len() > MAX_IDENT_CHARS {
            return Err(ExprError::BadIdentifier(name));
        }
        // A name directly after a number would be implicit multiplication.
        match self.peek() {
            Some(c) if c.is_ascii_digit() => Err(ExprError::Unsupported("implicit multiplication")),
            _ => Ok(Node::Name(name)),
        }
    }
}

/// The inferred dimension of a subtree: Poly means "not yet fixed" and takes
/// its dimension from context (SolidCraft's bare-number rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Poly,
    Fixed(Dim),
}

fn unify(a: Shape, b: Shape, what: &str) -> Result<Dim, ExprError> {
    match (a, b) {
        (Shape::Poly, Shape::Poly) => Err(ExprError::DimensionMismatch(what.to_string())),
        (Shape::Poly, Shape::Fixed(d)) | (Shape::Fixed(d), Shape::Poly) => Ok(d),
        (Shape::Fixed(x), Shape::Fixed(y)) if x == y => Ok(x),
        (Shape::Fixed(x), Shape::Fixed(y)) => Err(ExprError::DimensionMismatch(format!(
            "{what}: {x:?} vs {y:?}"
        ))),
    }
}

/// Evaluate against a name environment. The environment resolves parameter
/// names to their already-computed SI quantities (recipe order is dependency
/// order). Unknown names are errors: nothing is guessed.
pub fn eval(
    node: &Node,
    ctx: &Context,
    names: &dyn Fn(&str) -> Option<Quantity>,
) -> Result<Quantity, ExprError> {
    let (value, shape) = eval_inner(node, names, 0)?;
    let dim = match shape {
        Shape::Poly => ctx.bare_dim,
        Shape::Fixed(d) => d,
    };
    let value = match (shape, dim) {
        (Shape::Poly, Dim::Length) => value * ctx.bare_length,
        (Shape::Poly, Dim::Angle) => value * ctx.bare_angle,
        _ => value,
    };
    if !value.is_finite() {
        return Err(ExprError::NonFinite);
    }
    Ok(Quantity { value, dim })
}

fn eval_inner(
    node: &Node,
    names: &dyn Fn(&str) -> Option<Quantity>,
    depth: usize,
) -> Result<(f64, Shape), ExprError> {
    if depth > MAX_EXPR_DEPTH {
        return Err(ExprError::TooDeep(depth));
    }
    match node {
        Node::Number { text, unit } => {
            let value: f64 = text
                .parse()
                .map_err(|_| ExprError::BadNumber(text.clone()))?;
            match unit {
                None => Ok((value, Shape::Poly)),
                Some(u) => {
                    let (factor, dim) =
                        unit_factor(u).ok_or(ExprError::Unsupported("unit suffix"))?;
                    Ok((value * factor, Shape::Fixed(dim)))
                }
            }
        }
        Node::Name(name) => match names(name) {
            Some(q) => Ok((q.value, Shape::Fixed(q.dim))),
            None => Err(ExprError::UnknownName(name.clone())),
        },
        Node::Unary(inner) => {
            let (v, s) = eval_inner(inner, names, depth + 1)?;
            Ok((-v, s))
        }
        Node::Add(a, b) | Node::Sub(a, b) => {
            let (va, sa) = eval_inner(a, names, depth + 1)?;
            let (vb, sb) = eval_inner(b, names, depth + 1)?;
            let shape = match (sa, sb) {
                (Shape::Poly, Shape::Poly) => Shape::Poly,
                _ => Shape::Fixed(unify(sa, sb, "addition")?),
            };
            Ok((va + vb, shape))
        }
        Node::Mul(a, b) => {
            let (va, sa) = eval_inner(a, names, depth + 1)?;
            let (vb, sb) = eval_inner(b, names, depth + 1)?;
            // A bare operand stays dimensionless: 2 * width doubles a length.
            match (sa, sb) {
                (Shape::Poly, Shape::Poly) => Ok((va * vb, Shape::Poly)),
                (Shape::Poly, Shape::Fixed(d)) | (Shape::Fixed(d), Shape::Poly) => {
                    Ok((va * vb, Shape::Fixed(d)))
                }
                (Shape::Fixed(_), Shape::Fixed(_)) => Err(ExprError::DimensionMismatch(
                    "multiplication of two dimensioned values".to_string(),
                )),
            }
        }
        Node::Div(a, b) => {
            let (va, sa) = eval_inner(a, names, depth + 1)?;
            let (vb, sb) = eval_inner(b, names, depth + 1)?;
            if vb == 0.0 {
                return Err(ExprError::DivideByZero);
            }
            match (sa, sb) {
                (Shape::Poly, Shape::Fixed(_)) => Err(ExprError::DimensionMismatch(
                    "dividing a bare number by a dimensioned value".to_string(),
                )),
                (Shape::Fixed(_), Shape::Poly) => Ok((va / vb, sa)),
                (Shape::Fixed(x), Shape::Fixed(y)) if x == y => {
                    Ok((va / vb, Shape::Fixed(Dim::Count)))
                }
                (Shape::Fixed(_), Shape::Fixed(_)) => Err(ExprError::DimensionMismatch(
                    "division of different dimensions".to_string(),
                )),
                (Shape::Poly, Shape::Poly) => Ok((va / vb, Shape::Poly)),
            }
        }
        Node::Pow(a, b) => {
            let (va, sa) = eval_inner(a, names, depth + 1)?;
            let (vb, sb) = eval_inner(b, names, depth + 1)?;
            if sb != Shape::Poly {
                return Err(ExprError::DimensionMismatch(
                    "exponent must be a bare number".to_string(),
                ));
            }
            if vb.fract().abs() > f64::EPSILON {
                return Err(ExprError::Unsupported("fractional powers"));
            }
            if sa != Shape::Poly && vb != 0.0 {
                return Err(ExprError::DimensionMismatch(
                    "powers of dimensioned values".to_string(),
                ));
            }
            Ok((va.powf(vb), sa))
        }
    }
}

/// Translate to a SolidWorks equation right-hand side, preserving meaning:
/// literals carry explicit units where their dimension is fixed, bare numbers
/// that stand for lengths get the context unit suffix (a bare `20` in a
/// SolidWorks equation follows the part's document units, so the suffix is
/// what keeps the meaning), bare dimensionless operands stay bare, and
/// identifiers become quoted global-variable names. The result is equation
/// *syntax*; the emitter escapes it for the macro separately.
pub fn translate(node: &Node, ctx: &Context) -> String {
    // The root: a bare number that means a length gets the unit suffix here.
    if let Node::Number { text, unit: None } = node {
        if ctx.bare_dim != Dim::Count {
            return format!("{text}{}", ctx.bare_text);
        }
    }
    render(node, ctx)
}

fn render(node: &Node, ctx: &Context) -> String {
    fn bare_number(node: &Node) -> bool {
        matches!(node, Node::Number { unit: None, .. })
    }
    fn side(node: &Node, ctx: &Context, suffix: bool) -> String {
        let text = render(node, ctx);
        if suffix && bare_number(node) {
            format!("{text}{}", ctx.bare_text)
        } else {
            text
        }
    }
    match node {
        Node::Number { text, unit } => match unit {
            Some(u) => format!("{text}{u}"),
            None => text.clone(),
        },
        Node::Name(name) => format!("\"{}\"", name.replace('"', "")),
        Node::Unary(inner) => format!("-{}", render(inner, ctx)),
        Node::Add(a, b) | Node::Sub(a, b) => {
            let op = if matches!(node, Node::Add(..)) {
                "+"
            } else {
                "-"
            };
            format!(
                "({} {} {})",
                side(a, ctx, !bare_number(b)),
                op,
                side(b, ctx, !bare_number(a))
            )
        }
        Node::Mul(a, b) => format!("{} * {}", render(a, ctx), render(b, ctx)),
        Node::Div(a, b) => format!("{} / {}", render(a, ctx), render(b, ctx)),
        Node::Pow(a, b) => format!("({} ^ {})", render(a, ctx), render(b, ctx)),
    }
}

/// How a name resolves for translation purposes.
pub enum NameRef {
    /// A user parameter: quoted global-variable name.
    Global,
    /// A model parameter: the runtime name of a registered dimension.
    Slot(usize),
    Unknown,
}

/// Translate with runtime name resolution: segments of literal equation
/// syntax and dimension slots. Meaning-preserving like `translate`; a name
/// that resolves to nothing is an error, never a guess.
pub fn translate_with(
    node: &Node,
    ctx: &Context,
    resolve: &dyn Fn(&str) -> NameRef,
) -> Result<Vec<(String, Option<usize>)>, String> {
    fn bare_number(node: &Node) -> bool {
        matches!(node, Node::Number { unit: None, .. })
    }
    fn render(
        node: &Node,
        ctx: &Context,
        resolve: &dyn Fn(&str) -> NameRef,
        suffix_bare: bool,
    ) -> Result<Vec<(String, Option<usize>)>, String> {
        Ok(match node {
            Node::Number { text, unit } => {
                let text = match unit {
                    Some(u) => format!("{text}{u}"),
                    None if suffix_bare && ctx.bare_dim != Dim::Count => {
                        format!("{text}{}", ctx.bare_text)
                    }
                    None => text.clone(),
                };
                vec![(text, None)]
            }
            Node::Name(name) => match resolve(name) {
                NameRef::Global => vec![(format!("\"{}\"", name.replace('"', "")), None)],
                NameRef::Slot(slot) => vec![(String::new(), Some(slot))],
                NameRef::Unknown => return Err(format!("unresolvable name {name:?}")),
            },
            Node::Unary(a) => {
                let mut v = vec![("-".to_string(), None)];
                v.extend(render(a, ctx, resolve, false)?);
                v
            }
            Node::Add(a, b) | Node::Sub(a, b) => {
                let op = if matches!(node, Node::Add(..)) {
                    "+"
                } else {
                    "-"
                };
                let mut v = vec![("(".to_string(), None)];
                v.extend(render(a, ctx, resolve, !bare_number(b))?);
                v.push((format!(" {op} "), None));
                v.extend(render(b, ctx, resolve, !bare_number(a))?);
                v.push((")".to_string(), None));
                v
            }
            Node::Mul(a, b) => {
                let mut v = render(a, ctx, resolve, false)?;
                v.push((" * ".to_string(), None));
                v.extend(render(b, ctx, resolve, false)?);
                v
            }
            Node::Div(a, b) => {
                let mut v = render(a, ctx, resolve, false)?;
                v.push((" / ".to_string(), None));
                v.extend(render(b, ctx, resolve, false)?);
                v
            }
            Node::Pow(a, b) => {
                let mut v = vec![("(".to_string(), None)];
                v.extend(render(a, ctx, resolve, false)?);
                v.push((" ^ ".to_string(), None));
                v.extend(render(b, ctx, resolve, false)?);
                v.push((")".to_string(), None));
                v
            }
        })
    }
    // The root: a bare number that means a length gets the suffix.
    render(
        node,
        ctx,
        resolve,
        matches!(node, Node::Number { unit: None, .. }),
    )
}
