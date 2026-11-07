//! Formula text <-> expression tree: the parser, the printer and reference rewriting.

use crate::model::*;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ref1 {
    pub row: u32,
    pub col: u32,
    pub abs_r: bool,
    pub abs_c: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RefExpr {
    pub sheet: Option<Arc<str>>,
    pub a: Ref1,
    /// Second corner for ranges.
    pub b: Option<Ref1>,
}

impl RefExpr {
    pub fn rect(&self) -> Rect {
        match self.b {
            Some(b) => Rect::new(self.a.row, self.a.col, b.row, b.col),
            None => Rect::new(self.a.row, self.a.col, self.a.row, self.a.col),
        }
    }
    pub fn from_rect(sheet: Option<Arc<str>>, r: Rect) -> Self {
        RefExpr {
            sheet,
            a: Ref1 { row: r.r1, col: r.c1, abs_r: false, abs_c: false },
            b: if r.is_single() {
                None
            } else {
                Some(Ref1 { row: r.r2, col: r.c2, abs_r: false, abs_c: false })
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    fn text(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Pow => "^",
            BinOp::Concat => "&",
            BinOp::Eq => "=",
            BinOp::Ne => "<>",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    Str(Arc<str>),
    Bool(bool),
    Err(XlErr),
    Ref(RefExpr),
    Name(Arc<str>),
    Neg(Box<Expr>),
    Plus(Box<Expr>),
    Percent(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Paren(Box<Expr>),
    Call(Arc<str>, Vec<Expr>),
    Array(Vec<Vec<Expr>>),
    Missing,
    /// `A1#`: the whole spill range anchored at the cell.
    Spill(Box<Expr>),
    /// `@expr`: implicit intersection.
    Implicit(Box<Expr>),
    /// `Sheet1:Sheet3!A1:B2`.
    Ref3D(Arc<str>, Arc<str>, RefExpr),
    /// `Table1[Column]`, `Table1[#All]`, `Table1[[#Headers],[Col]]`, `[@Col]`.
    Struct(Arc<str>, Arc<str>),
    /// `a:b` where a side is not a plain reference, such as `A1:INDEX(...)`.
    RangeOp(Box<Expr>, Box<Expr>),
}

pub fn parse(src: &str) -> Result<Expr, String> {
    let s = src.trim();
    let s = s.strip_prefix('=').unwrap_or(s);
    let chars: Vec<char> = s.chars().collect();
    let mut p = P { s: &chars, i: 0 };
    p.ws();
    let e = p.compare()?;
    p.ws();
    if p.i < chars.len() {
        return Err(format!("Unexpected '{}'", chars[p.i]));
    }
    Ok(e)
}

struct P<'a> {
    s: &'a [char],
    i: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    fn at(&self, o: usize) -> Option<char> {
        self.s.get(self.i + o).copied()
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: char) -> bool {
        self.ws();
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn compare(&mut self) -> Result<Expr, String> {
        let mut l = self.concat()?;
        loop {
            self.ws();
            let op = match (self.peek(), self.at(1)) {
                (Some('<'), Some('=')) => (BinOp::Le, 2),
                (Some('<'), Some('>')) => (BinOp::Ne, 2),
                (Some('>'), Some('=')) => (BinOp::Ge, 2),
                (Some('<'), _) => (BinOp::Lt, 1),
                (Some('>'), _) => (BinOp::Gt, 1),
                (Some('='), _) => (BinOp::Eq, 1),
                _ => return Ok(l),
            };
            self.i += op.1;
            let r = self.concat()?;
            l = Expr::Bin(op.0, Box::new(l), Box::new(r));
        }
    }

    fn concat(&mut self) -> Result<Expr, String> {
        let mut l = self.additive()?;
        loop {
            self.ws();
            if self.peek() == Some('&') {
                self.i += 1;
                let r = self.additive()?;
                l = Expr::Bin(BinOp::Concat, Box::new(l), Box::new(r));
            } else {
                return Ok(l);
            }
        }
    }

    fn additive(&mut self) -> Result<Expr, String> {
        let mut l = self.term()?;
        loop {
            self.ws();
            let op = match self.peek() {
                Some('+') => BinOp::Add,
                Some('-') => BinOp::Sub,
                _ => return Ok(l),
            };
            self.i += 1;
            let r = self.term()?;
            l = Expr::Bin(op, Box::new(l), Box::new(r));
        }
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut l = self.power()?;
        loop {
            self.ws();
            let op = match self.peek() {
                Some('*') => BinOp::Mul,
                Some('/') => BinOp::Div,
                _ => return Ok(l),
            };
            self.i += 1;
            let r = self.power()?;
            l = Expr::Bin(op, Box::new(l), Box::new(r));
        }
    }

    fn power(&mut self) -> Result<Expr, String> {
        let mut l = self.unary()?;
        loop {
            self.ws();
            if self.peek() == Some('^') {
                self.i += 1;
                let r = self.unary()?;
                l = Expr::Bin(BinOp::Pow, Box::new(l), Box::new(r));
            } else {
                return Ok(l);
            }
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        self.ws();
        match self.peek() {
            Some('-') => {
                self.i += 1;
                Ok(Expr::Neg(Box::new(self.unary()?)))
            }
            Some('+') => {
                self.i += 1;
                Ok(Expr::Plus(Box::new(self.unary()?)))
            }
            Some('@') => {
                self.i += 1;
                Ok(Expr::Implicit(Box::new(self.unary()?)))
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.primary()?;
        loop {
            match self.peek() {
                Some('%') => {
                    self.i += 1;
                    e = Expr::Percent(Box::new(e));
                }
                Some('#') if matches!(e, Expr::Ref(_) | Expr::Name(_)) => {
                    self.i += 1;
                    e = Expr::Spill(Box::new(e));
                }
                Some(':') => {
                    self.i += 1;
                    let r = self.primary()?;
                    e = Expr::RangeOp(Box::new(e), Box::new(r));
                }
                _ => {
                    let save = self.i;
                    self.ws();
                    if self.peek() == Some('%') {
                        continue;
                    }
                    self.i = save;
                    return Ok(e);
                }
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, String> {
        self.ws();
        let c = self.peek().ok_or_else(|| "Formula is incomplete".to_string())?;
        match c {
            '(' => {
                self.i += 1;
                let e = self.compare()?;
                if !self.eat(')') {
                    return Err("Missing closing parenthesis".into());
                }
                Ok(Expr::Paren(Box::new(e)))
            }
            '"' => self.string(),
            '{' => self.array(),
            '#' => self.error_lit(),
            '\'' => self.quoted_sheet(),
            '[' => {
                let spec = self.bracket()?;
                Ok(Expr::Struct("".into(), spec.into()))
            }
            c if c.is_ascii_digit() || c == '.' => self.number_or_row(),
            c if c.is_alphabetic() || c == '_' || c == '$' || c == '\\' => self.word(),
            _ => Err(format!("Unexpected '{c}'")),
        }
    }

    fn string(&mut self) -> Result<Expr, String> {
        self.i += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err("Missing closing quote".into()),
                Some('"') => {
                    self.i += 1;
                    if self.peek() == Some('"') {
                        out.push('"');
                        self.i += 1;
                    } else {
                        return Ok(Expr::Str(out.into()));
                    }
                }
                Some(c) => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn array(&mut self) -> Result<Expr, String> {
        self.i += 1;
        let mut rows = vec![];
        let mut cur = vec![];
        loop {
            self.ws();
            let e = self.unary()?;
            cur.push(e);
            self.ws();
            match self.peek() {
                Some(',') => self.i += 1,
                Some(';') => {
                    self.i += 1;
                    rows.push(std::mem::take(&mut cur));
                }
                Some('}') => {
                    self.i += 1;
                    rows.push(cur);
                    return Ok(Expr::Array(rows));
                }
                _ => return Err("Bad array constant".into()),
            }
        }
    }

    fn error_lit(&mut self) -> Result<Expr, String> {
        let rest: String = self.s[self.i..].iter().take(8).collect::<String>().to_ascii_uppercase();
        for t in ["#DIV/0!", "#VALUE!", "#NULL!", "#NAME?", "#REF!", "#NUM!", "#N/A", "#SPILL!", "#CALC!"] {
            if rest.starts_with(t) {
                self.i += t.chars().count();
                if let Some(e) = XlErr::parse(t) {
                    return Ok(Expr::Err(e));
                }
            }
        }
        Err("Unknown error literal".into())
    }

    fn quoted_sheet(&mut self) -> Result<Expr, String> {
        self.i += 1;
        let mut name = String::new();
        loop {
            match self.peek() {
                None => return Err("Missing closing quote on sheet name".into()),
                Some('\'') => {
                    self.i += 1;
                    if self.peek() == Some('\'') {
                        name.push('\'');
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                Some(c) => {
                    name.push(c);
                    self.i += 1;
                }
            }
        }
        if self.peek() != Some('!') {
            return Err("Expected '!' after sheet name".into());
        }
        self.i += 1;
        if let Some((a, b)) = name.split_once(':') {
            let r = self.reference(Some("".into()))?;
            if let Expr::Ref(mut r) = r {
                r.sheet = None;
                return Ok(Expr::Ref3D(a.into(), b.into(), r));
            }
            return Err("Bad 3D reference".into());
        }
        self.reference(Some(name.into()))
    }

    /// Parses what follows `Sheet!`.
    fn reference(&mut self, sheet: Option<Arc<str>>) -> Result<Expr, String> {
        self.ws();
        if self.s[self.i..].iter().take(5).collect::<String>().eq_ignore_ascii_case("#REF!") {
            self.i += 5;
            return Ok(Expr::Err(XlErr::Ref));
        }
        let start = self.i;
        let word = self.take_word();
        if let Some(e) = self.try_ref_word(&word, sheet.clone()) {
            return Ok(e);
        }
        // Row-only reference such as Sheet2!1:3
        self.i = start;
        let digits = self.take_digits();
        if !digits.is_empty() && self.peek() == Some(':') {
            self.i += 1;
            let d2 = self.take_digits();
            if let (Ok(a), Ok(b)) = (digits.parse::<u32>(), d2.parse::<u32>()) {
                return Ok(whole_rows(sheet, a, false, b, false));
            }
        }
        if !word.is_empty() {
            self.i = start + word.chars().count();
            // A name scoped to a sheet: keep only the name.
            return Ok(Expr::Name(word.to_ascii_uppercase().into()));
        }
        Err("Bad reference".into())
    }

    fn take_word(&mut self) -> String {
        let st = self.i;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '.' || c == '$' || c == '\\' || c == '?' {
                self.i += 1;
            } else {
                break;
            }
        }
        self.s[st..self.i].iter().collect()
    }

    fn take_digits(&mut self) -> String {
        let st = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
        }
        self.s[st..self.i].iter().collect()
    }

    /// `word` was just consumed. Returns a reference if the word (and a `:second`) reads as one.
    fn try_ref_word(&mut self, word: &str, sheet: Option<Arc<str>>) -> Option<Expr> {
        if let Some(a) = split_cell(word) {
            if self.peek() == Some(':') {
                let save = self.i;
                self.i += 1;
                let w2 = self.take_word();
                if let Some(b) = split_cell(&w2) {
                    return Some(Expr::Ref(RefExpr { sheet, a, b: Some(b) }));
                }
                self.i = save;
            }
            return Some(Expr::Ref(RefExpr { sheet, a, b: None }));
        }
        // Whole columns: A:C or $A:$C
        if let Some((ca, aa)) = split_col(word) {
            if self.peek() == Some(':') {
                let save = self.i;
                self.i += 1;
                let w2 = self.take_word();
                if let Some((cb, ab)) = split_col(&w2) {
                    return Some(Expr::Ref(RefExpr {
                        sheet,
                        a: Ref1 { row: 0, col: ca, abs_r: false, abs_c: aa },
                        b: Some(Ref1 { row: MAX_ROWS - 1, col: cb, abs_r: false, abs_c: ab }),
                    }));
                }
                self.i = save;
            }
        }
        None
    }

    fn number_or_row(&mut self) -> Result<Expr, String> {
        let st = self.i;
        let digits = self.take_digits();
        // Whole rows like 3:7
        if self.peek() == Some(':') && !digits.is_empty() {
            let save = self.i;
            self.i += 1;
            let abs = self.peek() == Some('$');
            if abs {
                self.i += 1;
            }
            let d2 = self.take_digits();
            if !d2.is_empty() {
                if let (Ok(a), Ok(b)) = (digits.parse::<u32>(), d2.parse::<u32>()) {
                    return Ok(whole_rows(None, a, false, b, abs));
                }
            }
            self.i = save;
        }
        if self.peek() == Some('.') {
            self.i += 1;
            self.take_digits();
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            let save = self.i;
            self.i += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.i += 1;
            }
            if self.take_digits().is_empty() {
                self.i = save;
            }
        }
        let txt: String = self.s[st..self.i].iter().collect();
        txt.parse::<f64>().map(Expr::Num).map_err(|_| format!("Bad number '{txt}'"))
    }

    fn bracket(&mut self) -> Result<String, String> {
        // Reads a balanced [...] and returns the inside.
        self.i += 1;
        let st = self.i;
        let mut depth = 1;
        while let Some(c) = self.peek() {
            self.i += 1;
            if c == '[' {
                depth += 1;
            } else if c == ']' {
                depth -= 1;
                if depth == 0 {
                    return Ok(self.s[st..self.i - 1].iter().collect());
                }
            }
        }
        Err("Missing ']'".into())
    }

    fn word(&mut self) -> Result<Expr, String> {
        // `$3:$7` whole rows
        if self.peek() == Some('$') && matches!(self.at(1), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
            let d = self.take_digits();
            if self.peek() == Some(':') {
                self.i += 1;
                let abs = self.peek() == Some('$');
                if abs {
                    self.i += 1;
                }
                let d2 = self.take_digits();
                if let (Ok(a), Ok(b)) = (d.parse::<u32>(), d2.parse::<u32>()) {
                    return Ok(whole_rows(None, a, true, b, abs));
                }
            }
            return Err("Bad reference".into());
        }
        let word = self.take_word();
        if word.is_empty() {
            return Err("Unexpected character".into());
        }
        match self.peek() {
            Some('(') => {
                self.i += 1;
                return self.call(&word);
            }
            Some('!') => {
                self.i += 1;
                return self.reference(Some(word.into()));
            }
            Some('[') => {
                let spec = self.bracket()?;
                return Ok(Expr::Struct(word.into(), spec.into()));
            }
            Some(':') => {
                // Sheet1:Sheet3!A1
                let save = self.i;
                self.i += 1;
                let w2 = self.take_word();
                if self.peek() == Some('!') && !w2.is_empty() {
                    self.i += 1;
                    if let Expr::Ref(r) = self.reference(None)? {
                        return Ok(Expr::Ref3D(word.into(), w2.into(), r));
                    }
                    return Err("Bad 3D reference".into());
                }
                self.i = save;
            }
            _ => {}
        }
        if let Some(e) = self.try_ref_word(&word, None) {
            return Ok(e);
        }
        let up = word.to_ascii_uppercase();
        match up.as_str() {
            "TRUE" => Ok(Expr::Bool(true)),
            "FALSE" => Ok(Expr::Bool(false)),
            _ => Ok(Expr::Name(up.into())),
        }
    }

    fn call(&mut self, name: &str) -> Result<Expr, String> {
        let up = name.to_ascii_uppercase();
        let up = up.strip_prefix("_XLFN._XLWS.").or(up.strip_prefix("_XLFN.")).or(up.strip_prefix("_XLWS.")).unwrap_or(&up).to_string();
        let mut args = vec![];
        self.ws();
        if self.peek() == Some(')') {
            self.i += 1;
            return Ok(Expr::Call(up.into(), args));
        }
        loop {
            self.ws();
            if matches!(self.peek(), Some(',')) {
                args.push(Expr::Missing);
                self.i += 1;
                continue;
            }
            if self.peek() == Some(')') {
                args.push(Expr::Missing);
                self.i += 1;
                return Ok(Expr::Call(up.into(), args));
            }
            args.push(self.compare()?);
            self.ws();
            match self.peek() {
                Some(',') => {
                    self.i += 1;
                }
                Some(')') => {
                    self.i += 1;
                    return Ok(Expr::Call(up.into(), args));
                }
                None => return Err("Missing closing parenthesis".into()),
                Some(c) => return Err(format!("Unexpected '{c}'")),
            }
        }
    }
}

fn whole_rows(sheet: Option<Arc<str>>, a: u32, aa: bool, b: u32, ab: bool) -> Expr {
    Expr::Ref(RefExpr {
        sheet,
        a: Ref1 { row: a.saturating_sub(1), col: 0, abs_r: aa, abs_c: false },
        b: Some(Ref1 { row: b.saturating_sub(1), col: MAX_COLS - 1, abs_r: ab, abs_c: false }),
    })
}

fn split_col(w: &str) -> Option<(u32, bool)> {
    let abs = w.starts_with('$');
    let t = w.strip_prefix('$').unwrap_or(w);
    parse_col(t).map(|c| (c, abs))
}

fn split_cell(w: &str) -> Option<Ref1> {
    let b = w.as_bytes();
    let mut i = 0;
    let abs_c = b.first() == Some(&b'$');
    if abs_c {
        i += 1;
    }
    let st = i;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    let col = parse_col(w.get(st..i)?)?;
    let abs_r = b.get(i) == Some(&b'$');
    if abs_r {
        i += 1;
    }
    let digits = w.get(i..)?;
    if digits.is_empty() || !digits.bytes().all(|x| x.is_ascii_digit()) {
        return None;
    }
    let row: u32 = digits.parse().ok()?;
    if row == 0 || row > MAX_ROWS {
        return None;
    }
    Some(Ref1 { row: row - 1, col, abs_r, abs_c })
}

pub fn fmt_num(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        let s = format!("{}", x);
        if s.len() > 22 {
            format!("{:e}", x).replace('e', "E")
        } else {
            s
        }
    }
}

pub fn quote_sheet(name: &str) -> String {
    let plain = !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        && !name.chars().next().unwrap().is_ascii_digit()
        && split_cell(name).is_none();
    if plain {
        name.to_string()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

fn r1_str(r: &Ref1) -> String {
    format!(
        "{}{}{}{}",
        if r.abs_c { "$" } else { "" },
        col_name(r.col),
        if r.abs_r { "$" } else { "" },
        r.row + 1
    )
}

pub fn ref_str(r: &RefExpr) -> String {
    let mut s = String::new();
    if let Some(sh) = &r.sheet {
        s.push_str(&quote_sheet(sh));
        s.push('!');
    }
    match &r.b {
        None => s.push_str(&r1_str(&r.a)),
        Some(b) => {
            if r.a.row == 0 && b.row == MAX_ROWS - 1 {
                s.push_str(&format!(
                    "{}{}:{}{}",
                    if r.a.abs_c { "$" } else { "" },
                    col_name(r.a.col),
                    if b.abs_c { "$" } else { "" },
                    col_name(b.col)
                ));
            } else if r.a.col == 0 && b.col == MAX_COLS - 1 {
                s.push_str(&format!(
                    "{}{}:{}{}",
                    if r.a.abs_r { "$" } else { "" },
                    r.a.row + 1,
                    if b.abs_r { "$" } else { "" },
                    b.row + 1
                ));
            } else {
                s.push_str(&r1_str(&r.a));
                s.push(':');
                s.push_str(&r1_str(b));
            }
        }
    }
    s
}

pub fn print(e: &Expr) -> String {
    let mut s = String::new();
    pr(e, &mut s);
    s
}

fn pr(e: &Expr, s: &mut String) {
    match e {
        Expr::Num(x) => s.push_str(&fmt_num(*x)),
        Expr::Str(t) => {
            s.push('"');
            s.push_str(&t.replace('"', "\"\""));
            s.push('"');
        }
        Expr::Bool(b) => s.push_str(if *b { "TRUE" } else { "FALSE" }),
        Expr::Err(x) => s.push_str(x.text()),
        Expr::Ref(r) => s.push_str(&ref_str(r)),
        Expr::Name(n) => s.push_str(n),
        Expr::Neg(x) => {
            s.push('-');
            pr(x, s);
        }
        Expr::Plus(x) => {
            s.push('+');
            pr(x, s);
        }
        Expr::Percent(x) => {
            pr(x, s);
            s.push('%');
        }
        Expr::Bin(op, a, b) => {
            pr(a, s);
            s.push_str(op.text());
            pr(b, s);
        }
        Expr::Paren(x) => {
            s.push('(');
            pr(x, s);
            s.push(')');
        }
        Expr::Call(n, args) => {
            s.push_str(n);
            s.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                pr(a, s);
            }
            s.push(')');
        }
        Expr::Array(rows) => {
            s.push('{');
            for (i, r) in rows.iter().enumerate() {
                if i > 0 {
                    s.push(';');
                }
                for (j, v) in r.iter().enumerate() {
                    if j > 0 {
                        s.push(',');
                    }
                    pr(v, s);
                }
            }
            s.push('}');
        }
        Expr::Missing => {}
        Expr::Spill(x) => {
            pr(x, s);
            s.push('#');
        }
        Expr::Implicit(x) => {
            s.push('@');
            pr(x, s);
        }
        Expr::Ref3D(a, b, r) => {
            s.push_str(&quote_sheet(&format!("{a}:{b}")).replace("''", "'"));
            s.push('!');
            s.push_str(&ref_str(r));
        }
        Expr::Struct(t, spec) => {
            s.push_str(t);
            s.push('[');
            s.push_str(spec);
            s.push(']');
        }
        Expr::RangeOp(a, b) => {
            pr(a, s);
            s.push(':');
            pr(b, s);
        }
    }
}

/// Visits every reference, mutably. The callback may replace the reference with an error.
pub fn map_refs(e: &mut Expr, f: &mut dyn FnMut(&mut RefExpr) -> bool) {
    match e {
        Expr::Ref(r) => {
            if !f(r) {
                *e = Expr::Err(XlErr::Ref);
            }
        }
        Expr::Ref3D(_, _, r) => {
            if !f(r) {
                *e = Expr::Err(XlErr::Ref);
            }
        }
        Expr::Neg(x) | Expr::Plus(x) | Expr::Percent(x) | Expr::Paren(x) | Expr::Spill(x) | Expr::Implicit(x) => {
            map_refs(x, f)
        }
        Expr::Bin(_, a, b) | Expr::RangeOp(a, b) => {
            map_refs(a, f);
            map_refs(b, f);
        }
        Expr::Call(_, args) => {
            for a in args {
                map_refs(a, f);
            }
        }
        Expr::Array(rows) => {
            for r in rows {
                for a in r {
                    map_refs(a, f);
                }
            }
        }
        _ => {}
    }
}

/// Moves relative parts of a reference, as when copying a formula. Returns false if it falls off the sheet.
pub fn shift_ref(r: &mut RefExpr, dr: i64, dc: i64) -> bool {
    let whole_c = matches!(r.b, Some(b) if r.a.row == 0 && b.row == MAX_ROWS - 1);
    let whole_r = matches!(r.b, Some(b) if r.a.col == 0 && b.col == MAX_COLS - 1);
    let mv = |x: &mut Ref1| -> bool {
        if !x.abs_r && !whole_c {
            let n = x.row as i64 + dr;
            if n < 0 || n >= MAX_ROWS as i64 {
                return false;
            }
            x.row = n as u32;
        }
        if !x.abs_c && !whole_r {
            let n = x.col as i64 + dc;
            if n < 0 || n >= MAX_COLS as i64 {
                return false;
            }
            x.col = n as u32;
        }
        true
    };
    if !mv(&mut r.a) {
        return false;
    }
    if let Some(b) = &mut r.b {
        if !mv(b) {
            return false;
        }
    }
    true
}

pub fn shifted(e: &Expr, dr: i64, dc: i64) -> Expr {
    let mut x = e.clone();
    map_refs(&mut x, &mut |r| shift_ref(r, dr, dc));
    x
}

/// Rewrites a reference after `n` rows were inserted before `at`.
pub fn ins_rows(r: &mut RefExpr, at: u32, n: u32) {
    match &mut r.b {
        None => {
            if r.a.row >= at {
                r.a.row = (r.a.row + n).min(MAX_ROWS - 1);
            }
        }
        Some(b) => {
            if r.a.row == 0 && b.row == MAX_ROWS - 1 {
                return;
            }
            if r.a.row >= at {
                r.a.row = (r.a.row + n).min(MAX_ROWS - 1);
            }
            if b.row >= at {
                b.row = (b.row + n).min(MAX_ROWS - 1);
            }
        }
    }
}

pub fn ins_cols(r: &mut RefExpr, at: u32, n: u32) {
    match &mut r.b {
        None => {
            if r.a.col >= at {
                r.a.col = (r.a.col + n).min(MAX_COLS - 1);
            }
        }
        Some(b) => {
            if r.a.col == 0 && b.col == MAX_COLS - 1 {
                return;
            }
            if r.a.col >= at {
                r.a.col = (r.a.col + n).min(MAX_COLS - 1);
            }
            if b.col >= at {
                b.col = (b.col + n).min(MAX_COLS - 1);
            }
        }
    }
}

/// Maps one coordinate through a deletion of `n` items at `at`. Returns None if it was removed.
fn del1(x: u32, at: u32, n: u32) -> Option<u32> {
    if x < at {
        Some(x)
    } else if x >= at + n {
        Some(x - n)
    } else {
        None
    }
}

/// Maps a span through a deletion. None if the entire span was removed.
fn del_span(a: u32, b: u32, at: u32, n: u32) -> Option<(u32, u32)> {
    match (del1(a, at, n), del1(b, at, n)) {
        (Some(x), Some(y)) => Some((x, y)),
        (None, Some(y)) => {
            if b >= at + n {
                Some((at, y))
            } else {
                None
            }
        }
        (Some(x), None) => {
            if a < at {
                Some((x, at - 1))
            } else {
                None
            }
        }
        (None, None) => None,
    }
}

pub fn del_rows(r: &mut RefExpr, at: u32, n: u32) -> bool {
    match &mut r.b {
        None => match del1(r.a.row, at, n) {
            Some(x) => {
                r.a.row = x;
                true
            }
            None => false,
        },
        Some(b) => {
            if r.a.row == 0 && b.row == MAX_ROWS - 1 {
                return true;
            }
            match del_span(r.a.row, b.row, at, n) {
                Some((x, y)) => {
                    r.a.row = x;
                    b.row = y;
                    true
                }
                None => false,
            }
        }
    }
}

pub fn del_cols(r: &mut RefExpr, at: u32, n: u32) -> bool {
    match &mut r.b {
        None => match del1(r.a.col, at, n) {
            Some(x) => {
                r.a.col = x;
                true
            }
            None => false,
        },
        Some(b) => {
            if r.a.col == 0 && b.col == MAX_COLS - 1 {
                return true;
            }
            match del_span(r.a.col, b.col, at, n) {
                Some((x, y)) => {
                    r.a.col = x;
                    b.col = y;
                    true
                }
                None => false,
            }
        }
    }
}

/// Maps a rectangle through a row/column insertion.
pub fn rect_ins(x: Rect, rows: bool, at: u32, n: u32) -> Rect {
    let mut x = x;
    if rows {
        if x.r1 >= at {
            x.r1 = (x.r1 + n).min(MAX_ROWS - 1);
        }
        if x.r2 >= at {
            x.r2 = (x.r2 + n).min(MAX_ROWS - 1);
        }
    } else {
        if x.c1 >= at {
            x.c1 = (x.c1 + n).min(MAX_COLS - 1);
        }
        if x.c2 >= at {
            x.c2 = (x.c2 + n).min(MAX_COLS - 1);
        }
    }
    x
}

pub fn rect_del(x: Rect, rows: bool, at: u32, n: u32) -> Option<Rect> {
    let mut x = x;
    if rows {
        if x.whole_cols() {
            return Some(x);
        }
        let (a, b) = del_span(x.r1, x.r2, at, n)?;
        x.r1 = a;
        x.r2 = b;
    } else {
        if x.whole_rows() {
            return Some(x);
        }
        let (a, b) = del_span(x.c1, x.c2, at, n)?;
        x.c1 = a;
        x.c2 = b;
    }
    Some(x)
}

/// Finds the character spans of cell references in formula text, for colouring and F4.
/// Returns (start, end, text) in char indexes.
pub fn find_refs(text: &str) -> Vec<(usize, usize, String)> {
    let ch: Vec<char> = text.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < ch.len() {
        let c = ch[i];
        if c == '"' {
            i += 1;
            while i < ch.len() && ch[i] != '"' {
                i += 1;
            }
            i += 1;
            continue;
        }
        if c == '\'' {
            let st = i;
            i += 1;
            while i < ch.len() && ch[i] != '\'' {
                i += 1;
            }
            i += 1;
            if i < ch.len() && ch[i] == '!' {
                i += 1;
                let e = word_end(&ch, i);
                let e = range_tail(&ch, e);
                out.push((st, e, ch[st..e].iter().collect()));
                i = e;
            }
            continue;
        }
        if c.is_alphanumeric() || c == '$' || c == '_' {
            let st = i;
            let mut e = word_end(&ch, i);
            let w: String = ch[st..e].iter().collect();
            if e < ch.len() && ch[e] == '(' {
                i = e;
                continue;
            }
            if e < ch.len() && ch[e] == '!' {
                let e2 = word_end(&ch, e + 1);
                let e2 = range_tail(&ch, e2);
                out.push((st, e2, ch[st..e2].iter().collect()));
                i = e2;
                continue;
            }
            let colon_next = e < ch.len() && ch[e] == ':';
            let is_ref = split_cell(&w).is_some() || (colon_next && (split_col(&w).is_some() || w.trim_start_matches('$').parse::<u32>().is_ok()));
            if is_ref {
                e = range_tail(&ch, e);
                out.push((st, e, ch[st..e].iter().collect()));
            }
            i = e.max(st + 1);
            continue;
        }
        i += 1;
    }
    out
}

fn word_end(ch: &[char], mut i: usize) -> usize {
    while i < ch.len() && (ch[i].is_alphanumeric() || ch[i] == '$' || ch[i] == '_' || ch[i] == '.') {
        i += 1;
    }
    i
}

fn range_tail(ch: &[char], e: usize) -> usize {
    if e < ch.len() && ch[e] == ':' {
        word_end(ch, e + 1)
    } else {
        e
    }
}

/// F4: cycles a reference through $A$1, A$1, $A1, A1.
pub fn cycle_abs(reference: &str) -> String {
    let (sheet, body) = match reference.rfind('!') {
        Some(i) => (&reference[..=i], &reference[i + 1..]),
        None => ("", reference),
    };
    let parts: Vec<String> = body
        .split(':')
        .map(|p| {
            let t = p.replace('$', "");
            let split = t.find(|c: char| c.is_ascii_digit()).unwrap_or(t.len());
            let (c, r) = t.split_at(split);
            let abs_c = p.starts_with('$');
            let abs_r = p[1.min(p.len())..].contains('$');
            let (nc, nr) = match (abs_c, abs_r) {
                (false, false) => (true, true),
                (true, true) => (false, true),
                (false, true) => (true, false),
                (true, false) => (false, false),
            };
            format!(
                "{}{}{}{}",
                if nc && !c.is_empty() { "$" } else { "" },
                c,
                if nr && !r.is_empty() { "$" } else { "" },
                r
            )
        })
        .collect();
    format!("{sheet}{}", parts.join(":"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for f in [
            "1+2*3",
            "SUM(A1:B2)",
            "IF(A1>0,\"yes\",\"no\")",
            "Sheet2!$A$1+'My Sheet'!B2",
            "-A1^2%",
            "SUM(A:A,1:1)",
            "{1,2;3,4}",
            "LOG10(100)",
            "A1#",
            "IFERROR(VLOOKUP(A1,$D$2:$F$10,2,FALSE),\"\")",
        ] {
            let e = parse(f).unwrap();
            assert_eq!(print(&e), f);
        }
    }

    #[test]
    fn shift() {
        let e = parse("A1+$B2+C$3+$D$4").unwrap();
        assert_eq!(print(&shifted(&e, 2, 1)), "B3+$B4+D$3+$D$4");
    }

    #[test]
    fn delete_row_in_range() {
        let mut r = RefExpr::from_rect(None, Rect::new(2, 0, 5, 0));
        assert!(del_rows(&mut r, 0, 3));
        assert_eq!(r.rect(), Rect::new(0, 0, 2, 0));
        let mut r = RefExpr::from_rect(None, Rect::new(2, 0, 2, 0));
        assert!(!del_rows(&mut r, 2, 1));
    }

    #[test]
    fn refs_found() {
        let v = find_refs("SUM(A1:B2)+Sheet2!C3+\"D4\"");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].2, "A1:B2");
        assert_eq!(v[1].2, "Sheet2!C3");
    }
}
