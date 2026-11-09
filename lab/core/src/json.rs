//! A small JSON reader for recipes, written by hand: serde_json alone took the size spike's
//! WebAssembly module from 27,899 to 67,805 bytes gzipped (eb49c96, `spike/wasm-size/sizes.json`).
//! Numbers are kept as their text, so the recipe converts each one exactly once, the same way
//! on every platform.

pub enum Json<'a> {
    Null,
    Bool(bool),
    Num(&'a str),
    Str(String),
    Arr(Vec<Json<'a>>),
    /// Keys in document order; a repeated key is an error.
    Obj(Vec<(String, Json<'a>)>),
}

pub struct JsonError {
    pub msg: &'static str,
    pub line: u32,
    pub col: u32,
}

pub fn parse(src: &str) -> Result<Json<'_>, JsonError> {
    let mut p = Parser { s: src.as_bytes(), src, i: 0, depth: 0 };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(p.err("unexpected text after the value"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    src: &'a str,
    i: usize,
    depth: u32,
}

impl<'a> Parser<'a> {
    fn err(&self, msg: &'static str) -> JsonError {
        let before = &self.s[..self.i.min(self.s.len())];
        let line = 1 + before.iter().filter(|&&b| b == b'\n').count() as u32;
        let col = 1 + before.iter().rev().take_while(|&&b| b != b'\n').count() as u32;
        JsonError { msg, line, col }
    }
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.i += 1;
        }
    }
    fn eat(&mut self, lit: &[u8]) -> bool {
        if self.s[self.i..].starts_with(lit) {
            self.i += lit.len();
            true
        } else {
            false
        }
    }
    fn value(&mut self) -> Result<Json<'a>, JsonError> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ if self.eat(b"true") => Ok(Json::Bool(true)),
            _ if self.eat(b"false") => Ok(Json::Bool(false)),
            _ if self.eat(b"null") => Ok(Json::Null),
            None => Err(self.err("unexpected end of the recipe")),
            _ => Err(self.err("expected a value")),
        }
    }
    fn nest(&mut self) -> Result<(), JsonError> {
        self.depth += 1;
        if self.depth > 32 { Err(self.err("nested too deeply")) } else { Ok(()) }
    }
    fn object(&mut self) -> Result<Json<'a>, JsonError> {
        self.nest()?;
        self.i += 1;
        let mut out: Vec<(String, Json<'a>)> = Vec::new();
        self.ws();
        if self.eat(b"}") {
            self.depth -= 1;
            return Ok(Json::Obj(out));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected a key in quotes"));
            }
            let at = self.i;
            let k = self.string()?;
            if out.iter().any(|(e, _)| *e == k) {
                self.i = at;
                return Err(self.err("repeated key"));
            }
            self.ws();
            if !self.eat(b":") {
                return Err(self.err("expected ':'"));
            }
            self.ws();
            let v = self.value()?;
            out.push((k, v));
            self.ws();
            if self.eat(b",") {
                continue;
            }
            if self.eat(b"}") {
                self.depth -= 1;
                return Ok(Json::Obj(out));
            }
            return Err(self.err("expected ',' or '}'"));
        }
    }
    fn array(&mut self) -> Result<Json<'a>, JsonError> {
        self.nest()?;
        self.i += 1;
        let mut out = Vec::new();
        self.ws();
        if self.eat(b"]") {
            self.depth -= 1;
            return Ok(Json::Arr(out));
        }
        loop {
            self.ws();
            out.push(self.value()?);
            self.ws();
            if self.eat(b",") {
                continue;
            }
            if self.eat(b"]") {
                self.depth -= 1;
                return Ok(Json::Arr(out));
            }
            return Err(self.err("expected ',' or ']'"));
        }
    }
    fn string(&mut self) -> Result<String, JsonError> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.i += 1;
            }
            out.push_str(self.src.get(start..self.i).unwrap_or(""));
            match self.peek() {
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.i += 1;
                    let c = match self.peek() {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'/') => '/',
                        Some(b'n') => '\n',
                        Some(b't') => '\t',
                        Some(b'r') => '\r',
                        Some(b'b') => '\u{8}',
                        Some(b'f') => '\u{c}',
                        Some(b'u') => {
                            let hex = self.s.get(self.i + 1..self.i + 5).ok_or(self.err("bad \\u escape"))?;
                            let mut n = 0u32;
                            for &h in hex {
                                n = n * 16 + (h as char).to_digit(16).ok_or(self.err("bad \\u escape"))?;
                            }
                            self.i += 4;
                            char::from_u32(n).ok_or(self.err("\\u escape outside the basic plane"))?
                        }
                        _ => return Err(self.err("bad escape")),
                    };
                    self.i += 1;
                    out.push(c);
                }
                Some(_) => return Err(self.err("control character in a string")),
                None => return Err(self.err("unterminated string")),
            }
        }
    }
    fn number(&mut self) -> Result<Json<'a>, JsonError> {
        let start = self.i;
        self.eat(b"-");
        let digits = |p: &mut Self| {
            let s = p.i;
            while let Some(b'0'..=b'9') = p.peek() {
                p.i += 1;
            }
            p.i - s
        };
        let int_digits = digits(self);
        if int_digits == 0 || (int_digits > 1 && self.s[self.i - int_digits] == b'0') {
            return Err(self.err("bad number"));
        }
        if self.eat(b".") && digits(self) == 0 {
            return Err(self.err("bad number"));
        }
        if let Some(b'e' | b'E') = self.peek() {
            return Err(self.err("exponents are not used in recipes; write the decimal"));
        }
        Ok(Json::Num(self.src.get(start..self.i).unwrap_or("0")))
    }
}
