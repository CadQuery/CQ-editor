//! The script's parameters: top-level assignments of a number literal, such
//! as `width = 40`. The Parameters panel shows them as spin boxes and
//! sliders and changes the literal in the script, which stays the only
//! source of truth.
//!
//! Only statements that start a top-level line are recognized: lines inside
//! a function, a block, a bracketed expression, a triple-quoted string or a
//! backslash continuation are skipped, as are expressions, chained or
//! multiple assignments and other kinds of literals. A trailing comment may
//! give the slider's range: `# [10, 100]`, `# [0, 5, 0.5]` (with a step) or
//! `# 10..100`.

use std::ops::{Range, RangeInclusive};

#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub name: String,
    /// 1-based line of the assignment.
    pub line: usize,
    /// Byte range of the literal in the text, including its sign.
    pub literal: Range<usize>,
    pub value: f64,
    /// Written as an integer; it stays one.
    pub integer: bool,
    /// Digits after the point in the literal (0 for integers).
    pub decimals: usize,
    /// Written with an exponent, such as `1e-3`.
    pub exponent: bool,
    /// From the trailing comment.
    pub range: Option<RangeInclusive<f64>>,
    pub step: Option<f64>,
}

impl Parameter {
    /// The literal to write for `value`, in the style of the current one.
    pub fn literal_for(&self, value: f64) -> String {
        let value = match &self.range {
            Some(range) => value.clamp(*range.start(), *range.end()),
            None => value,
        };
        if self.integer {
            return format!("{}", value.round() as i64);
        }
        if self.exponent {
            return format!("{value:e}");
        }
        let decimals = self.precision();
        let text = format!("{value:.decimals$}");
        if text.starts_with('-') && text[1..].bytes().all(|b| b == b'0' || b == b'.') {
            text[1..].to_owned()
        } else {
            text
        }
    }

    /// Decimals a float keeps: those of the literal and of the step.
    pub fn precision(&self) -> usize {
        if self.integer {
            return 0;
        }
        let step = self.step.map_or(0, decimals_of);
        self.decimals.max(step).max(1)
    }

    /// Change per pixel of a drag: a whole number of the smallest unit
    /// written, about a 200th of the value.
    pub fn drag_speed(&self) -> f64 {
        if let Some(step) = self.step {
            return step;
        }
        let unit = if self.integer {
            1.0
        } else {
            10f64.powi(-(self.precision() as i32))
        };
        unit * (self.value.abs() / 200.0 / unit).round().max(1.0)
    }
}

/// The parameters of `text`, in line order.
pub fn parameters(text: &str) -> Vec<Parameter> {
    let mut found = Vec::new();
    let mut scanner = Scanner::default();
    let mut offset = 0;
    for (index, line) in text.split('\n').enumerate() {
        if scanner.at_statement_start()
            && let Some(parameter) = parse_line(line, offset, index + 1)
        {
            found.push(parameter);
        }
        scanner.scan(line);
        offset += line.len() + 1;
    }
    found
}

/// Where each line starts: inside a string, brackets or a continuation.
#[derive(Default)]
struct Scanner {
    /// The quote character of an open triple-quoted string.
    triple: Option<u8>,
    depth: usize,
    continued: bool,
}

impl Scanner {
    fn at_statement_start(&self) -> bool {
        self.triple.is_none() && self.depth == 0 && !self.continued
    }

    fn scan(&mut self, line: &str) {
        let bytes = line.as_bytes();
        let mut i = 0;
        let mut quote: Option<u8> = None;
        self.continued = false;
        while i < bytes.len() {
            let byte = bytes[i];
            if let Some(q) = self.triple {
                if byte == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i..].starts_with(&[q, q, q]) {
                    self.triple = None;
                    i += 3;
                    continue;
                }
                i += 1;
                continue;
            }
            if let Some(q) = quote {
                if byte == b'\\' {
                    i += 2;
                    continue;
                }
                if byte == q {
                    quote = None;
                }
                i += 1;
                continue;
            }
            match byte {
                b'#' => break,
                b'"' | b'\'' => {
                    if bytes[i..].starts_with(&[byte, byte, byte]) {
                        self.triple = Some(byte);
                        i += 3;
                        continue;
                    }
                    quote = Some(byte);
                }
                b'(' | b'[' | b'{' => self.depth += 1,
                b')' | b']' | b'}' => self.depth = self.depth.saturating_sub(1),
                b'\\' if i + 1 == bytes.len() => self.continued = true,
                _ => {}
            }
            i += 1;
        }
    }
}

/// `name = number`, `name: int = number` or `name: float = number`, with
/// an optional comment, starting at column 0.
fn parse_line(line: &str, offset: usize, number: usize) -> Option<Parameter> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    let mut cursor = Cursor { text: line, at: 0 };
    let name = cursor.identifier()?;
    if name.starts_with("__") {
        return None;
    }
    cursor.spaces();
    if cursor.eat(":") {
        cursor.spaces();
        let annotation = cursor.identifier()?;
        if annotation != "int" && annotation != "float" {
            return None;
        }
        cursor.spaces();
    }
    if !cursor.eat("=") || cursor.peek() == Some(b'=') {
        return None;
    }
    cursor.spaces();
    let start = cursor.at;
    let literal = cursor.number()?;
    let end = cursor.at;
    cursor.spaces();
    let comment = match cursor.rest() {
        "" => "",
        rest => rest.strip_prefix('#')?,
    };
    let (range, step) = range_hint(comment).unzip();
    Some(Parameter {
        name: name.to_owned(),
        line: number,
        literal: offset + start..offset + end,
        value: literal.value,
        integer: literal.integer,
        decimals: literal.decimals,
        exponent: literal.exponent,
        range,
        step: step.flatten(),
    })
}

struct Literal {
    value: f64,
    integer: bool,
    decimals: usize,
    exponent: bool,
}

struct Cursor<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    fn eat(&mut self, token: &str) -> bool {
        if self.rest().starts_with(token) {
            self.at += token.len();
            true
        } else {
            false
        }
    }

    fn spaces(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.at += 1;
        }
    }

    fn take_while(&mut self, accept: impl Fn(u8) -> bool) -> &'a str {
        let start = self.at;
        while self.peek().is_some_and(&accept) {
            self.at += 1;
        }
        &self.text[start..self.at]
    }

    fn identifier(&mut self) -> Option<&'a str> {
        if !self
            .peek()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        {
            return None;
        }
        Some(self.take_while(|b| b.is_ascii_alphanumeric() || b == b'_'))
    }

    /// A decimal int or float literal with an optional sign: `-12`,
    /// `1_000`, `2.5`, `.5`, `5.`, `1e-3`.
    fn number(&mut self) -> Option<Literal> {
        let start = self.at;
        if matches!(self.peek(), Some(b'-' | b'+')) {
            self.at += 1;
            self.spaces();
        }
        let digits = |b: u8| b.is_ascii_digit() || b == b'_';
        let whole = self.take_while(digits);
        let mut fraction = None;
        if self.eat(".") {
            fraction = Some(self.take_while(digits));
        }
        if whole.is_empty() && fraction.is_none_or(str::is_empty) {
            return None;
        }
        // Not 0x10, 0o7, 1j or the start of a name: those are not plain.
        if whole.len() > 1 && whole.starts_with('0') && fraction.is_none() {
            return None;
        }
        let mut exponent = false;
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'-' | b'+')) {
                self.at += 1;
            }
            if self.take_while(|b| b.is_ascii_digit()).is_empty() {
                return None;
            }
            exponent = true;
        }
        if self
            .peek()
            .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
        {
            return None;
        }
        // "- 5" is valid Python too; underscores only group digits.
        let plain: String = self.text[start..self.at]
            .chars()
            .filter(|c| !matches!(c, '_' | ' ' | '\t'))
            .collect();
        let value: f64 = plain.parse().ok()?;
        value.is_finite().then(|| Literal {
            value,
            integer: fraction.is_none() && !exponent,
            decimals: fraction.map_or(0, |f| f.chars().filter(|c| *c != '_').count()),
            exponent,
        })
    }
}

/// `[min, max]`, `[min, max, step]` or `min..max` anywhere in a comment.
fn range_hint(comment: &str) -> Option<(RangeInclusive<f64>, Option<f64>)> {
    if let Some(open) = comment.find('[')
        && let Some(close) = comment[open..].find(']')
    {
        let numbers: Option<Vec<f64>> = comment[open + 1..open + close]
            .split(',')
            .map(|part| part.trim().parse::<f64>().ok())
            .collect();
        if let Some(numbers) = numbers {
            match numbers.as_slice() {
                [min, max] if min < max => return Some((*min..=*max, None)),
                [min, max, step] if min < max && *step > 0.0 => {
                    return Some((*min..=*max, Some(*step)));
                }
                _ => {}
            }
        }
    }
    let (left, right) = comment.split_once("..")?;
    let min = left.split_whitespace().last()?.parse::<f64>().ok()?;
    let max = right.split_whitespace().next()?.parse::<f64>().ok()?;
    (min < max).then_some((min..=max, None))
}

fn decimals_of(value: f64) -> usize {
    let text = format!("{value}");
    text.split_once('.')
        .map_or(0, |(_, fraction)| fraction.len())
}

#[cfg(test)]
#[path = "../tests/rust/app/test_parameters.rs"]
mod tests;
