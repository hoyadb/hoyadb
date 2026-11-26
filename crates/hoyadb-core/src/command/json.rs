//! API-0006 section 1 / TC-0032 C05: bounded, dependency-free command values.
use crate::storage::FiniteFloat;
use std::collections::BTreeMap;

pub(super) const MAX_BYTES: usize = 1_048_576;

const MAX_DEPTH: usize = 64;

const MAX_NODES: usize = 100_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    Null,
    Bool(bool),
    String(String),
    Integer(i64),
    Float(FiniteFloat),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonError {
    InvalidUtf8,
    InvalidSyntax,
    InvalidNumber,
    DuplicateKey,
    LimitExceeded,
}

impl Json {
    pub fn parse(input: &str) -> Result<Self, JsonError> {
        if input.len() > MAX_BYTES {
            return Err(JsonError::LimitExceeded);
        }
        let mut parser = Parser {
            input,
            pos: 0,
            nodes: 0,
        };
        let value = parser.value(0)?;
        parser.whitespace();
        if parser.pos != input.len() {
            return Err(JsonError::InvalidSyntax);
        }
        Ok(value)
    }

    pub fn from_bytes(input: &[u8]) -> Result<Self, JsonError> {
        if input.len() > MAX_BYTES {
            return Err(JsonError::LimitExceeded);
        }
        Self::parse(std::str::from_utf8(input).map_err(|_| JsonError::InvalidUtf8)?)
    }

    pub fn canonical(&self) -> Result<String, JsonError> {
        let mut encoder = Encoder {
            output: String::new(),
            nodes: 0,
        };
        encoder.value(self, 0)?;
        Ok(encoder.output)
    }
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
    nodes: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\r' | b'\n' | b'\t')) {
            self.pos += 1;
        }
    }

    fn take(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), JsonError> {
        self.take(byte)
            .then_some(())
            .ok_or(JsonError::InvalidSyntax)
    }

    fn value(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(JsonError::LimitExceeded);
        }
        self.whitespace();
        match self.peek() {
            Some(b'n') => self.literal("null", Json::Null),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'"') => self.string().map(Json::String),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b'[' | b'{') if depth >= MAX_DEPTH => Err(JsonError::LimitExceeded),
            Some(b'[') => self.array(depth + 1),
            Some(b'{') => self.object(depth + 1),
            _ => Err(JsonError::InvalidSyntax),
        }
    }

    fn literal(&mut self, text: &str, value: Json) -> Result<Json, JsonError> {
        if !self.input[self.pos..].starts_with(text) {
            return Err(JsonError::InvalidSyntax);
        }
        self.pos += text.len();
        Ok(value)
    }

    fn array(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.pos += 1;
        let mut values = Vec::new();
        self.whitespace();
        if self.take(b']') {
            return Ok(Json::Array(values));
        }
        loop {
            values.push(self.value(depth)?);
            self.whitespace();
            if self.take(b']') {
                return Ok(Json::Array(values));
            }
            self.expect(b',')?;
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.pos += 1;
        let mut values = BTreeMap::new();
        self.whitespace();
        if self.take(b'}') {
            return Ok(Json::Object(values));
        }
        loop {
            self.whitespace();
            let key = self.string()?;
            if values.contains_key(&key) {
                return Err(JsonError::DuplicateKey);
            }
            self.whitespace();
            self.expect(b':')?;
            values.insert(key, self.value(depth)?);
            self.whitespace();
            if self.take(b'}') {
                return Ok(Json::Object(values));
            }
            self.expect(b',')?;
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut output = String::new();
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(output);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let escaped = self.peek().ok_or(JsonError::InvalidSyntax)?;
                    self.pos += 1;
                    output.push(match escaped {
                        b'"' | b'\\' | b'/' => char::from(escaped),
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode()?,
                        _ => return Err(JsonError::InvalidSyntax),
                    });
                }
                Some(0..=31) | None => return Err(JsonError::InvalidSyntax),
                Some(_) => {
                    // Input is validated UTF-8 and positions remain character boundaries.
                    let character = self.input[self.pos..]
                        .chars()
                        .next()
                        .ok_or(JsonError::InvalidSyntax)?;
                    self.pos += character.len_utf8();
                    output.push(character);
                }
            }
        }
    }

    fn hex_quad(&mut self) -> Result<u32, JsonError> {
        let mut value = 0;
        for _ in 0..4 {
            let digit = match self.peek() {
                Some(b'0'..=b'9') => u32::from(self.input.as_bytes()[self.pos] - b'0'),
                Some(b'a'..=b'f') => u32::from(self.input.as_bytes()[self.pos] - b'a') + 10,
                Some(b'A'..=b'F') => u32::from(self.input.as_bytes()[self.pos] - b'A') + 10,
                _ => return Err(JsonError::InvalidSyntax),
            };
            self.pos += 1;
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn unicode(&mut self) -> Result<char, JsonError> {
        let first = self.hex_quad()?;
        let codepoint = if (0xd800..=0xdbff).contains(&first) {
            self.expect(b'\\')?;
            self.expect(b'u')?;
            let second = self.hex_quad()?;
            if !(0xdc00..=0xdfff).contains(&second) {
                return Err(JsonError::InvalidSyntax);
            }
            0x10000 + ((first - 0xd800) << 10) + second - 0xdc00
        } else {
            first
        };
        char::from_u32(codepoint).ok_or(JsonError::InvalidSyntax)
    }

    fn digits(&mut self) -> Result<(), JsonError> {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        (self.pos > start)
            .then_some(())
            .ok_or(JsonError::InvalidNumber)
    }

    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.pos;
        self.take(b'-');
        if !self.take(b'0') {
            self.digits()?;
        }
        let mut float = false;
        if self.take(b'.') {
            float = true;
            self.digits()?;
        }
        if self.take(b'e') || self.take(b'E') {
            float = true;
            if !self.take(b'+') {
                self.take(b'-');
            }
            self.digits()?;
        }
        let token = &self.input[start..self.pos];
        if float {
            token
                .parse::<f64>()
                .ok()
                .and_then(FiniteFloat::new)
                .map(Json::Float)
                .ok_or(JsonError::InvalidNumber)
        } else {
            token
                .parse::<i64>()
                .map(Json::Integer)
                .map_err(|_| JsonError::InvalidNumber)
        }
    }
}

struct Encoder {
    output: String,
    nodes: usize,
}

impl Encoder {
    fn append(&mut self, text: &str) -> Result<(), JsonError> {
        if text.len() > MAX_BYTES - self.output.len() {
            return Err(JsonError::LimitExceeded);
        }
        self.output.push_str(text);
        Ok(())
    }

    fn string(&mut self, text: &str) -> Result<(), JsonError> {
        self.append("\"")?;
        for character in text.chars() {
            match character {
                '"' => self.append("\\\"")?,
                '\\' => self.append("\\\\")?,
                '\u{0}'..='\u{1f}' => {
                    const HEX: &[u8] = b"0123456789abcdef";
                    let codepoint = character as usize;
                    let escaped = [
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[codepoint >> 4],
                        HEX[codepoint & 15],
                    ];
                    self.append(
                        std::str::from_utf8(&escaped).map_err(|_| JsonError::InvalidUtf8)?,
                    )?;
                }
                _ => self.append(character.encode_utf8(&mut [0; 4]))?,
            }
        }
        self.append("\"")
    }

    fn value(&mut self, value: &Json, depth: usize) -> Result<(), JsonError> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(JsonError::LimitExceeded);
        }
        match value {
            Json::Null => self.append("null"),
            Json::Bool(true) => self.append("true"),
            Json::Bool(false) => self.append("false"),
            Json::String(text) => self.string(text),
            Json::Integer(value) => self.append(&value.to_string()),
            Json::Float(value) => {
                let text = value.get().to_string();
                self.append(&text)?;
                if !text.contains(['.', 'e', 'E']) {
                    self.append(".0")?;
                }
                Ok(())
            }
            Json::Array(_) | Json::Object(_) if depth >= MAX_DEPTH => Err(JsonError::LimitExceeded),
            Json::Array(values) => {
                self.append("[")?;
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        self.append(",")?;
                    }
                    self.value(value, depth + 1)?;
                }
                self.append("]")
            }
            Json::Object(values) => {
                self.append("{")?;
                for (index, (key, value)) in values.iter().enumerate() {
                    if index > 0 {
                        self.append(",")?;
                    }
                    self.string(key)?;
                    self.append(":")?;
                    self.value(value, depth + 1)?;
                }
                self.append("}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float(value: f64) -> Json {
        Json::Float(FiniteFloat::new(value).unwrap())
    }

    fn nested(depth: usize) -> String {
        format!("{}null{}", "[".repeat(depth), "]".repeat(depth))
    }

    fn many_nodes(count: usize) -> String {
        format!("[{}]", vec!["0"; count - 1].join(","))
    }

    #[test]
    fn c05_canonical_values_sort_keys_and_preserve_unicode_and_controls() {
        let input = " \r\n{\"z\": [null, true, false], \"é\": \"東京😀\", \"a\": \"\\\"\\\\\\/\\b\\f\\n\\r\\t\\u0000\\u001f\"} \t";
        let canonical = "{\"a\":\"\\\"\\\\/\\u0008\\u000c\\u000a\\u000d\\u0009\\u0000\\u001f\",\"z\":[null,true,false],\"é\":\"東京😀\"}";
        let value = Json::parse(input).unwrap();
        assert_eq!(value.canonical().unwrap(), canonical);
        assert_eq!(Json::parse(canonical).unwrap(), value);
        assert_eq!(
            Json::parse("\"\\uD83D\\uDE00\"").unwrap(),
            Json::String("😀".into())
        );
        assert_eq!(
            Json::from_bytes("\"café\"".as_bytes()).unwrap(),
            Json::String("café".into())
        );
        for value in [
            Json::Null,
            Json::Bool(false),
            Json::Bool(true),
            Json::Array(Vec::new()),
            Json::Object(BTreeMap::new()),
        ] {
            assert_eq!(Json::parse(&value.canonical().unwrap()).unwrap(), value);
        }
    }

    #[test]
    fn c05_exact_integers_and_all_finite_float_extremes_roundtrip() {
        for value in [i64::MIN, -1, 0, 1, i64::MAX] {
            let json = Json::Integer(value);
            assert_eq!(Json::parse(&json.canonical().unwrap()).unwrap(), json);
            assert_eq!(Json::parse(&value.to_string()).unwrap(), json);
        }
        for value in [
            0.0,
            -0.0,
            1.0,
            1.25,
            -12.0,
            f64::MAX,
            f64::MIN,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
            1e308,
        ] {
            let json = float(value);
            assert_eq!(Json::parse(&json.canonical().unwrap()).unwrap(), json);
        }
        assert_eq!(float(42.0).canonical().unwrap(), "42.0");
        assert_eq!(float(-0.0).canonical().unwrap(), "0.0");
        assert_eq!(Json::parse("1e308").unwrap(), float(1e308));
        assert_eq!(Json::parse("1e-9999").unwrap(), float(0.0));
        assert_eq!(Json::parse("-0").unwrap(), Json::Integer(0));
        for input in [
            "9223372036854775808",
            "-9223372036854775809",
            "1e309",
            "-1e309",
        ] {
            assert_eq!(Json::parse(input), Err(JsonError::InvalidNumber), "{input}");
        }
    }

    #[test]
    fn c05_rejects_invalid_grammar_unicode_duplicates_and_nonfinite_tokens() {
        for input in [
            "",
            "null true",
            "\u{a0}null",
            "[1,]",
            "{\"a\":1,}",
            "{1:2}",
            "[",
            "\"x",
            "\"\n\"",
            "\"\\x\"",
            "\"\\uZZZZ\"",
            "\"\\uD800\"",
            "\"\\uDC00\"",
            "\"\\uDC00\\uD800\"",
            "\"\\uD800\\u0041\"",
            "NaN",
            "Infinity",
            "+1",
            ".1",
            "01",
            "-01",
            "1.",
            "1e",
            "1e+",
            "--1",
        ] {
            assert!(Json::parse(input).is_err(), "accepted {input:?}");
        }
        for input in [
            "{\"a\":1,\"a\":2}",
            "{\"a\":1,\"\\u0061\":2}",
            "{\"😀\":1,\"\\ud83d\\ude00\":2}",
        ] {
            assert_eq!(Json::parse(input), Err(JsonError::DuplicateKey));
        }
        assert_eq!(
            Json::from_bytes(&[b'"', 0xff, b'"']),
            Err(JsonError::InvalidUtf8)
        );
        assert_eq!(
            Json::from_bytes(&[b'"', 0xc0, 0xaf, b'"']),
            Err(JsonError::InvalidUtf8)
        );
    }

    #[test]
    fn c05_depth_is_bounded_for_input_and_caller_constructed_trees() {
        let valid = Json::parse(&nested(MAX_DEPTH)).unwrap();
        assert_eq!(valid.canonical().unwrap(), nested(MAX_DEPTH));
        assert_eq!(
            Json::parse(&nested(MAX_DEPTH + 1)),
            Err(JsonError::LimitExceeded)
        );
        assert_eq!(
            Json::Array(vec![valid]).canonical(),
            Err(JsonError::LimitExceeded)
        );
        let mut value = Json::Null;
        for _ in 0..MAX_DEPTH {
            value = Json::Object(BTreeMap::from([("a".into(), value)]));
        }
        assert!(value.canonical().is_ok());
        assert_eq!(
            Json::Object(BTreeMap::from([("a".into(), value)])).canonical(),
            Err(JsonError::LimitExceeded)
        );
    }

    #[test]
    fn c05_node_limit_counts_containers_and_values_but_not_keys() {
        let valid = Json::parse(&many_nodes(MAX_NODES)).unwrap();
        assert!(valid.canonical().unwrap() == many_nodes(MAX_NODES));
        assert_eq!(
            Json::parse(&many_nodes(MAX_NODES + 1)),
            Err(JsonError::LimitExceeded)
        );
        assert_eq!(
            Json::Array(vec![Json::Null; MAX_NODES]).canonical(),
            Err(JsonError::LimitExceeded)
        );
        let object = Json::Object(
            (0..MAX_NODES - 1)
                .map(|i| (i.to_string(), Json::Integer(0)))
                .collect(),
        );
        let encoded = object.canonical().unwrap();
        assert!(Json::parse(&encoded).unwrap() == object);
    }

    #[test]
    fn c05_byte_limit_checks_input_output_and_escape_expansion() {
        let valid = format!("\"{}\"", "x".repeat(MAX_BYTES - 2));
        let value = Json::parse(&valid).unwrap();
        assert!(value.canonical().unwrap() == valid);
        let unicode = Json::String("é".repeat((MAX_BYTES - 2) / 2));
        let encoded = unicode.canonical().unwrap();
        assert_eq!(encoded.len(), MAX_BYTES);
        assert!(Json::parse(&encoded).unwrap() == unicode);
        assert_eq!(
            Json::String("é".repeat(MAX_BYTES / 2)).canonical(),
            Err(JsonError::LimitExceeded)
        );
        assert_eq!(
            Json::parse(&format!(" {valid}")),
            Err(JsonError::LimitExceeded)
        );
        assert_eq!(
            Json::String("x".repeat(MAX_BYTES - 1)).canonical(),
            Err(JsonError::LimitExceeded)
        );
        let escaped = Json::String("\0".repeat((MAX_BYTES - 2) / 6));
        assert_eq!(escaped.canonical().unwrap().len(), MAX_BYTES - 2);
        assert_eq!(
            Json::String("\0".repeat((MAX_BYTES - 2) / 6 + 1)).canonical(),
            Err(JsonError::LimitExceeded)
        );
        assert_eq!(
            Json::from_bytes(&vec![b' '; MAX_BYTES + 1]),
            Err(JsonError::LimitExceeded)
        );
    }
}
