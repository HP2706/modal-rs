//! Minimal Python pickle protocol 2 encoder and protocol 0-5 decoder.
//!
//! Supports the basic types used by Modal Queue operations:
//! None, bool, int, float, string, bytes, list, tuple, dict.

use crate::error::ModalError;
use std::collections::HashMap;

// Pickle opcodes
const PROTO: u8 = 0x80;
const STOP: u8 = b'.';
const NONE: u8 = b'N';
const NEWTRUE: u8 = 0x88;
const NEWFALSE: u8 = 0x89;
const BININT1: u8 = b'K';
const BININT2: u8 = b'M';
const BININT: u8 = b'J';
const LONG1: u8 = 0x8a;
const BINFLOAT: u8 = b'G';
const BINUNICODE: u8 = b'X';
const SHORT_BINUNICODE: u8 = 0x8c;
const SHORT_BINBYTES: u8 = b'C';
const BINBYTES: u8 = b'B';
const BINBYTES8: u8 = 0x8e;
const BINUNICODE8: u8 = 0x8d;
const SHORT_BINSTRING: u8 = b'U';
const BINSTRING: u8 = b'T';
const EMPTY_LIST: u8 = b']';
const EMPTY_DICT: u8 = b'}';
const EMPTY_TUPLE: u8 = b')';
const MARK: u8 = b'(';
const APPEND: u8 = b'a';
const APPENDS: u8 = b'e';
const SETITEM: u8 = b's';
const SETITEMS: u8 = b'u';
const LIST: u8 = b'l';
const DICT: u8 = b'd';
const TUPLE: u8 = b't';
const TUPLE1: u8 = 0x85;
const TUPLE2: u8 = 0x86;
const TUPLE3: u8 = 0x87;
const BINPUT: u8 = b'q';
const LONG_BINPUT: u8 = b'r';
const BINGET: u8 = b'h';
const LONG_BINGET: u8 = b'j';
const PUT: u8 = b'p';
const GET: u8 = b'g';
const MEMOIZE: u8 = 0x94;
const FRAME: u8 = 0x95;
const INT: u8 = b'I';
const FLOAT: u8 = b'F';
const STRING: u8 = b'S';
const UNICODE: u8 = b'V';
const POP: u8 = b'0';
const DUP: u8 = b'2';
const POP_MARK: u8 = b'1';
const GLOBAL: u8 = b'c';
const REDUCE: u8 = b'R';
const BUILD: u8 = b'b';
const NEWOBJ: u8 = 0x81;
const STACK_GLOBAL: u8 = 0x93;

/// A value that can be serialized/deserialized via pickle protocol.
#[derive(Debug, Clone, PartialEq)]
pub enum PickleValue {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<PickleValue>),
    Tuple(Vec<PickleValue>),
    Dict(Vec<(PickleValue, PickleValue)>),
}

impl From<bool> for PickleValue {
    fn from(v: bool) -> Self {
        PickleValue::Bool(v)
    }
}

impl From<i32> for PickleValue {
    fn from(v: i32) -> Self {
        PickleValue::Int(v as i64)
    }
}

impl From<i64> for PickleValue {
    fn from(v: i64) -> Self {
        PickleValue::Int(v)
    }
}

impl From<u32> for PickleValue {
    fn from(v: u32) -> Self {
        PickleValue::Int(v as i64)
    }
}

impl From<f64> for PickleValue {
    fn from(v: f64) -> Self {
        PickleValue::Float(v)
    }
}

impl From<&str> for PickleValue {
    fn from(v: &str) -> Self {
        PickleValue::String(v.to_string())
    }
}

impl From<String> for PickleValue {
    fn from(v: String) -> Self {
        PickleValue::String(v)
    }
}

impl From<Vec<u8>> for PickleValue {
    fn from(v: Vec<u8>) -> Self {
        PickleValue::Bytes(v)
    }
}

impl From<Vec<PickleValue>> for PickleValue {
    fn from(v: Vec<PickleValue>) -> Self {
        PickleValue::List(v)
    }
}

// ── Encoder (protocol 2) ──────────────────────────────────────────────

/// Serialize a PickleValue to pickle protocol 2 bytes.
pub fn pickle_serialize(value: &PickleValue) -> Result<Vec<u8>, ModalError> {
    let mut buf = Vec::new();
    buf.push(PROTO);
    buf.push(2); // protocol version
    encode_value(&mut buf, value)?;
    buf.push(STOP);
    Ok(buf)
}

fn encode_value(buf: &mut Vec<u8>, value: &PickleValue) -> Result<(), ModalError> {
    match value {
        PickleValue::None => buf.push(NONE),
        PickleValue::Bool(true) => buf.push(NEWTRUE),
        PickleValue::Bool(false) => buf.push(NEWFALSE),
        PickleValue::Int(v) => encode_int(buf, *v),
        PickleValue::Float(v) => {
            buf.push(BINFLOAT);
            buf.extend_from_slice(&v.to_be_bytes());
        }
        PickleValue::String(s) => {
            let bytes = s.as_bytes();
            buf.push(BINUNICODE);
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
        PickleValue::Bytes(b) => {
            if b.len() < 256 {
                buf.push(SHORT_BINBYTES);
                buf.push(b.len() as u8);
            } else {
                buf.push(BINBYTES);
                buf.extend_from_slice(&(b.len() as u32).to_le_bytes());
            }
            buf.extend_from_slice(b);
        }
        PickleValue::List(items) => {
            buf.push(EMPTY_LIST);
            if !items.is_empty() {
                buf.push(MARK);
                for item in items {
                    encode_value(buf, item)?;
                }
                buf.push(APPENDS);
            }
        }
        PickleValue::Tuple(items) => match items.len() {
            0 => buf.push(EMPTY_TUPLE),
            1 => {
                encode_value(buf, &items[0])?;
                buf.push(TUPLE1);
            }
            2 => {
                encode_value(buf, &items[0])?;
                encode_value(buf, &items[1])?;
                buf.push(TUPLE2);
            }
            3 => {
                encode_value(buf, &items[0])?;
                encode_value(buf, &items[1])?;
                encode_value(buf, &items[2])?;
                buf.push(TUPLE3);
            }
            _ => {
                buf.push(MARK);
                for item in items {
                    encode_value(buf, item)?;
                }
                buf.push(TUPLE);
            }
        },
        PickleValue::Dict(pairs) => {
            buf.push(EMPTY_DICT);
            if !pairs.is_empty() {
                buf.push(MARK);
                for (k, v) in pairs {
                    encode_value(buf, k)?;
                    encode_value(buf, v)?;
                }
                buf.push(SETITEMS);
            }
        }
    }
    Ok(())
}

fn encode_int(buf: &mut Vec<u8>, v: i64) {
    if v >= 0 && v <= 0xFF {
        buf.push(BININT1);
        buf.push(v as u8);
    } else if v >= 0 && v <= 0xFFFF {
        buf.push(BININT2);
        buf.extend_from_slice(&(v as u16).to_le_bytes());
    } else if v >= i32::MIN as i64 && v <= i32::MAX as i64 {
        buf.push(BININT);
        buf.extend_from_slice(&(v as i32).to_le_bytes());
    } else {
        // Use LONG1 for values outside i32 range
        buf.push(LONG1);
        let bytes = long_to_bytes(v);
        buf.push(bytes.len() as u8);
        buf.extend_from_slice(&bytes);
    }
}

/// Convert i64 to pickle long format (signed, little-endian, minimal bytes).
fn long_to_bytes(v: i64) -> Vec<u8> {
    if v == 0 {
        return vec![];
    }
    let raw = v.to_le_bytes();
    // Find minimal representation
    let mut len = 8;
    if v >= 0 {
        while len > 1 && raw[len - 1] == 0 {
            len -= 1;
        }
        // Need extra 0 byte if high bit set (to keep sign positive)
        if raw[len - 1] & 0x80 != 0 {
            let mut result = raw[..len].to_vec();
            result.push(0);
            return result;
        }
    } else {
        while len > 1 && raw[len - 1] == 0xFF {
            len -= 1;
        }
        // Need extra 0xFF byte if high bit clear (to keep sign negative)
        if raw[len - 1] & 0x80 == 0 {
            let mut result = raw[..len].to_vec();
            result.push(0xFF);
            return result;
        }
    }
    raw[..len].to_vec()
}

// ── Decoder (protocol 0-5) ────────────────────────────────────────────

/// Deserialize pickle bytes into a PickleValue.
pub fn pickle_deserialize(data: &[u8]) -> Result<PickleValue, ModalError> {
    let mut decoder = Decoder::new(data);
    decoder.decode()
}

struct Decoder<'a> {
    data: &'a [u8],
    pos: usize,
    stack: Vec<StackItem>,
    memo: HashMap<u32, PickleValue>,
}

#[derive(Debug)]
enum StackItem {
    Value(PickleValue),
    Mark,
}

impl<'a> Decoder<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            stack: Vec::new(),
            memo: HashMap::new(),
        }
    }

    fn err(&self, msg: &str) -> ModalError {
        ModalError::Serialization(format!("pickle decode error at {}: {}", self.pos, msg))
    }

    fn read_byte(&mut self) -> Result<u8, ModalError> {
        if self.pos >= self.data.len() {
            return Err(self.err("unexpected end of data"));
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], ModalError> {
        if self.pos + n > self.data.len() {
            return Err(self.err("unexpected end of data"));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn read_u16_le(&mut self) -> Result<u16, ModalError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_i32_le(&mut self) -> Result<i32, ModalError> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u32_le(&mut self) -> Result<u32, ModalError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64_le(&mut self) -> Result<u64, ModalError> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_f64_be(&mut self) -> Result<f64, ModalError> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_line(&mut self) -> Result<&'a [u8], ModalError> {
        let start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos] != b'\n' {
            self.pos += 1;
        }
        if self.pos >= self.data.len() {
            return Err(self.err("unterminated line"));
        }
        let line = &self.data[start..self.pos];
        self.pos += 1; // skip \n
        Ok(line)
    }

    fn push(&mut self, v: PickleValue) {
        self.stack.push(StackItem::Value(v));
    }

    fn pop_value(&mut self) -> Result<PickleValue, ModalError> {
        match self.stack.pop() {
            Some(StackItem::Value(v)) => Ok(v),
            Some(StackItem::Mark) => Err(self.err("unexpected mark on stack")),
            None => Err(self.err("stack underflow")),
        }
    }

    /// Pop items from stack down to the most recent MARK, returning them in order.
    fn pop_to_mark(&mut self) -> Result<Vec<PickleValue>, ModalError> {
        let mut items = Vec::new();
        loop {
            match self.stack.pop() {
                Some(StackItem::Mark) => {
                    items.reverse();
                    return Ok(items);
                }
                Some(StackItem::Value(v)) => items.push(v),
                None => return Err(self.err("MARK not found on stack")),
            }
        }
    }

    fn decode(&mut self) -> Result<PickleValue, ModalError> {
        loop {
            let opcode = self.read_byte()?;
            match opcode {
                PROTO => {
                    let _version = self.read_byte()?;
                }
                FRAME => {
                    // Protocol 4+ frame: 8-byte frame size, just skip it
                    let _frame_size = self.read_u64_le()?;
                }
                STOP => {
                    return self.pop_value();
                }
                NONE => self.push(PickleValue::None),
                NEWTRUE => self.push(PickleValue::Bool(true)),
                NEWFALSE => self.push(PickleValue::Bool(false)),
                BININT1 => {
                    let v = self.read_byte()? as i64;
                    self.push(PickleValue::Int(v));
                }
                BININT2 => {
                    let v = self.read_u16_le()? as i64;
                    self.push(PickleValue::Int(v));
                }
                BININT => {
                    let v = self.read_i32_le()? as i64;
                    self.push(PickleValue::Int(v));
                }
                LONG1 => {
                    let n = self.read_byte()? as usize;
                    let bytes = self.read_bytes(n)?;
                    let v = bytes_to_long(bytes);
                    self.push(PickleValue::Int(v));
                }
                BINFLOAT => {
                    let v = self.read_f64_be()?;
                    self.push(PickleValue::Float(v));
                }
                BINUNICODE => {
                    let len = self.read_u32_le()? as usize;
                    let bytes = self.read_bytes(len)?;
                    let s = String::from_utf8(bytes.to_vec())
                        .map_err(|e| self.err(&format!("invalid UTF-8: {}", e)))?;
                    self.push(PickleValue::String(s));
                }
                SHORT_BINUNICODE => {
                    let len = self.read_byte()? as usize;
                    let bytes = self.read_bytes(len)?;
                    let s = String::from_utf8(bytes.to_vec())
                        .map_err(|e| self.err(&format!("invalid UTF-8: {}", e)))?;
                    self.push(PickleValue::String(s));
                }
                BINUNICODE8 => {
                    let len = self.read_u64_le()? as usize;
                    let bytes = self.read_bytes(len)?;
                    let s = String::from_utf8(bytes.to_vec())
                        .map_err(|e| self.err(&format!("invalid UTF-8: {}", e)))?;
                    self.push(PickleValue::String(s));
                }
                SHORT_BINBYTES => {
                    let len = self.read_byte()? as usize;
                    let bytes = self.read_bytes(len)?;
                    self.push(PickleValue::Bytes(bytes.to_vec()));
                }
                BINBYTES => {
                    let len = self.read_u32_le()? as usize;
                    let bytes = self.read_bytes(len)?;
                    self.push(PickleValue::Bytes(bytes.to_vec()));
                }
                BINBYTES8 => {
                    let len = self.read_u64_le()? as usize;
                    let bytes = self.read_bytes(len)?;
                    self.push(PickleValue::Bytes(bytes.to_vec()));
                }
                SHORT_BINSTRING => {
                    // Protocol 1 string (bytes in Python 2, str in Py2)
                    let len = self.read_byte()? as usize;
                    let bytes = self.read_bytes(len)?;
                    self.push(PickleValue::Bytes(bytes.to_vec()));
                }
                BINSTRING => {
                    let len = self.read_i32_le()? as usize;
                    let bytes = self.read_bytes(len)?;
                    self.push(PickleValue::Bytes(bytes.to_vec()));
                }
                UNICODE => {
                    let line = self.read_line()?;
                    let s = String::from_utf8(line.to_vec())
                        .map_err(|e| self.err(&format!("invalid UTF-8: {}", e)))?;
                    self.push(PickleValue::String(s));
                }
                STRING => {
                    let line = self.read_line()?;
                    // Remove surrounding quotes
                    let s = std::str::from_utf8(line)
                        .map_err(|e| self.err(&format!("invalid UTF-8: {}", e)))?;
                    let trimmed = s.trim_matches('\'').trim_matches('"');
                    self.push(PickleValue::Bytes(trimmed.as_bytes().to_vec()));
                }
                INT => {
                    let line = self.read_line()?;
                    let s = std::str::from_utf8(line)
                        .map_err(|e| self.err(&format!("invalid UTF-8: {}", e)))?;
                    let s = s.trim();
                    if s == "00" {
                        self.push(PickleValue::Bool(false));
                    } else if s == "01" {
                        self.push(PickleValue::Bool(true));
                    } else {
                        let v: i64 = s
                            .parse()
                            .map_err(|e| self.err(&format!("invalid int: {}", e)))?;
                        self.push(PickleValue::Int(v));
                    }
                }
                FLOAT => {
                    let line = self.read_line()?;
                    let s = std::str::from_utf8(line)
                        .map_err(|e| self.err(&format!("invalid UTF-8: {}", e)))?;
                    let v: f64 = s
                        .trim()
                        .parse()
                        .map_err(|e| self.err(&format!("invalid float: {}", e)))?;
                    self.push(PickleValue::Float(v));
                }
                EMPTY_LIST => self.push(PickleValue::List(vec![])),
                EMPTY_DICT => self.push(PickleValue::Dict(vec![])),
                EMPTY_TUPLE => self.push(PickleValue::Tuple(vec![])),
                MARK => self.stack.push(StackItem::Mark),
                LIST => {
                    let items = self.pop_to_mark()?;
                    self.push(PickleValue::List(items));
                }
                DICT => {
                    let items = self.pop_to_mark()?;
                    if items.len() % 2 != 0 {
                        return Err(self.err("odd number of items for dict"));
                    }
                    let pairs = items
                        .chunks(2)
                        .map(|c| (c[0].clone(), c[1].clone()))
                        .collect();
                    self.push(PickleValue::Dict(pairs));
                }
                TUPLE => {
                    let items = self.pop_to_mark()?;
                    self.push(PickleValue::Tuple(items));
                }
                TUPLE1 => {
                    let a = self.pop_value()?;
                    self.push(PickleValue::Tuple(vec![a]));
                }
                TUPLE2 => {
                    let b = self.pop_value()?;
                    let a = self.pop_value()?;
                    self.push(PickleValue::Tuple(vec![a, b]));
                }
                TUPLE3 => {
                    let c = self.pop_value()?;
                    let b = self.pop_value()?;
                    let a = self.pop_value()?;
                    self.push(PickleValue::Tuple(vec![a, b, c]));
                }
                APPEND => {
                    let item = self.pop_value()?;
                    match self.stack.last_mut() {
                        Some(StackItem::Value(PickleValue::List(list))) => list.push(item),
                        _ => return Err(self.err("APPEND on non-list")),
                    }
                }
                APPENDS => {
                    let items = self.pop_to_mark()?;
                    match self.stack.last_mut() {
                        Some(StackItem::Value(PickleValue::List(list))) => list.extend(items),
                        _ => return Err(self.err("APPENDS on non-list")),
                    }
                }
                SETITEM => {
                    let val = self.pop_value()?;
                    let key = self.pop_value()?;
                    match self.stack.last_mut() {
                        Some(StackItem::Value(PickleValue::Dict(pairs))) => {
                            pairs.push((key, val));
                        }
                        _ => return Err(self.err("SETITEM on non-dict")),
                    }
                }
                SETITEMS => {
                    let items = self.pop_to_mark()?;
                    if items.len() % 2 != 0 {
                        return Err(self.err("odd number of items for SETITEMS"));
                    }
                    let new_pairs: Vec<(PickleValue, PickleValue)> = items
                        .chunks(2)
                        .map(|c| (c[0].clone(), c[1].clone()))
                        .collect();
                    match self.stack.last_mut() {
                        Some(StackItem::Value(PickleValue::Dict(pairs))) => {
                            pairs.extend(new_pairs);
                        }
                        _ => return Err(self.err("SETITEMS on non-dict")),
                    }
                }
                BINPUT => {
                    let idx = self.read_byte()? as u32;
                    if let Some(StackItem::Value(v)) = self.stack.last() {
                        self.memo.insert(idx, v.clone());
                    }
                }
                LONG_BINPUT => {
                    let idx = self.read_u32_le()?;
                    if let Some(StackItem::Value(v)) = self.stack.last() {
                        self.memo.insert(idx, v.clone());
                    }
                }
                PUT => {
                    let line = self.read_line()?;
                    let s = std::str::from_utf8(line)
                        .map_err(|e| self.err(&format!("invalid memo index: {}", e)))?;
                    let idx: u32 = s
                        .trim()
                        .parse()
                        .map_err(|e| self.err(&format!("invalid memo index: {}", e)))?;
                    if let Some(StackItem::Value(v)) = self.stack.last() {
                        self.memo.insert(idx, v.clone());
                    }
                }
                BINGET => {
                    let idx = self.read_byte()? as u32;
                    let v = self
                        .memo
                        .get(&idx)
                        .ok_or_else(|| self.err(&format!("memo key {} not found", idx)))?
                        .clone();
                    self.push(v);
                }
                LONG_BINGET => {
                    let idx = self.read_u32_le()?;
                    let v = self
                        .memo
                        .get(&idx)
                        .ok_or_else(|| self.err(&format!("memo key {} not found", idx)))?
                        .clone();
                    self.push(v);
                }
                GET => {
                    let line = self.read_line()?;
                    let s = std::str::from_utf8(line)
                        .map_err(|e| self.err(&format!("invalid memo index: {}", e)))?;
                    let idx: u32 = s
                        .trim()
                        .parse()
                        .map_err(|e| self.err(&format!("invalid memo index: {}", e)))?;
                    let v = self
                        .memo
                        .get(&idx)
                        .ok_or_else(|| self.err(&format!("memo key {} not found", idx)))?
                        .clone();
                    self.push(v);
                }
                MEMOIZE => {
                    let idx = self.memo.len() as u32;
                    if let Some(StackItem::Value(v)) = self.stack.last() {
                        self.memo.insert(idx, v.clone());
                    }
                }
                POP => {
                    self.stack.pop();
                }
                DUP => {
                    if let Some(StackItem::Value(v)) = self.stack.last() {
                        let v = v.clone();
                        self.push(v);
                    }
                }
                POP_MARK => {
                    let _ = self.pop_to_mark()?;
                }
                GLOBAL => {
                    // Read module\nname\n
                    let _module = self.read_line()?;
                    let _name = self.read_line()?;
                    // Push a placeholder — we don't support arbitrary globals
                    self.push(PickleValue::None);
                }
                STACK_GLOBAL => {
                    // Pop name and module from stack
                    let _name = self.pop_value()?;
                    let _module = self.pop_value()?;
                    self.push(PickleValue::None);
                }
                REDUCE => {
                    let _args = self.pop_value()?;
                    let _callable = self.pop_value()?;
                    // We can't actually call anything — push None
                    self.push(PickleValue::None);
                }
                NEWOBJ => {
                    let _args = self.pop_value()?;
                    let _cls = self.pop_value()?;
                    self.push(PickleValue::None);
                }
                BUILD => {
                    let _state = self.pop_value()?;
                    // BUILD modifies TOS in place; we ignore it
                }
                _ => {
                    return Err(self.err(&format!("unsupported opcode 0x{:02x}", opcode)));
                }
            }
        }
    }
}

/// Convert pickle long bytes (signed, little-endian) to i64.
fn bytes_to_long(bytes: &[u8]) -> i64 {
    if bytes.is_empty() {
        return 0;
    }
    // Sign-extend to 8 bytes
    let negative = bytes[bytes.len() - 1] & 0x80 != 0;
    let fill = if negative { 0xFF } else { 0x00 };
    let mut buf = [fill; 8];
    let len = bytes.len().min(8);
    buf[..len].copy_from_slice(&bytes[..len]);
    i64::from_le_bytes(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_none() {
        let v = PickleValue::None;
        let encoded = pickle_serialize(&v).unwrap();
        let decoded = pickle_deserialize(&encoded).unwrap();
        assert_eq!(decoded, v);
    }

    #[test]
    fn test_roundtrip_bool() {
        for b in [true, false] {
            let v = PickleValue::Bool(b);
            let encoded = pickle_serialize(&v).unwrap();
            let decoded = pickle_deserialize(&encoded).unwrap();
            assert_eq!(decoded, v);
        }
    }

    #[test]
    fn test_roundtrip_int_small() {
        for i in [0i64, 1, 127, 255] {
            let v = PickleValue::Int(i);
            let encoded = pickle_serialize(&v).unwrap();
            let decoded = pickle_deserialize(&encoded).unwrap();
            assert_eq!(decoded, v);
        }
    }

    #[test]
    fn test_roundtrip_int_medium() {
        for i in [256i64, 1000, 65535] {
            let v = PickleValue::Int(i);
            let encoded = pickle_serialize(&v).unwrap();
            let decoded = pickle_deserialize(&encoded).unwrap();
            assert_eq!(decoded, v);
        }
    }

    #[test]
    fn test_roundtrip_int_large() {
        for i in [65536i64, -1, -128, i32::MIN as i64, i32::MAX as i64] {
            let v = PickleValue::Int(i);
            let encoded = pickle_serialize(&v).unwrap();
            let decoded = pickle_deserialize(&encoded).unwrap();
            assert_eq!(decoded, v);
        }
    }

    #[test]
    fn test_roundtrip_int_very_large() {
        for i in [i64::MAX, i64::MIN, i32::MAX as i64 + 1, i32::MIN as i64 - 1] {
            let v = PickleValue::Int(i);
            let encoded = pickle_serialize(&v).unwrap();
            let decoded = pickle_deserialize(&encoded).unwrap();
            assert_eq!(decoded, v);
        }
    }

    #[test]
    fn test_roundtrip_float() {
        for f in [0.0, 1.5, -3.14, f64::INFINITY, f64::NEG_INFINITY] {
            let v = PickleValue::Float(f);
            let encoded = pickle_serialize(&v).unwrap();
            let decoded = pickle_deserialize(&encoded).unwrap();
            assert_eq!(decoded, v);
        }
    }

    #[test]
    fn test_roundtrip_float_nan() {
        let v = PickleValue::Float(f64::NAN);
        let encoded = pickle_serialize(&v).unwrap();
        let decoded = pickle_deserialize(&encoded).unwrap();
        match decoded {
            PickleValue::Float(f) => assert!(f.is_nan()),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn test_roundtrip_string() {
        for s in ["", "hello", "hello world! 🦀", "a".repeat(300).as_str()] {
            let v = PickleValue::String(s.to_string());
            let encoded = pickle_serialize(&v).unwrap();
            let decoded = pickle_deserialize(&encoded).unwrap();
            assert_eq!(decoded, v);
        }
    }

    #[test]
    fn test_roundtrip_bytes() {
        let v = PickleValue::Bytes(vec![0, 1, 2, 255]);
        let encoded = pickle_serialize(&v).unwrap();
        let decoded = pickle_deserialize(&encoded).unwrap();
        assert_eq!(decoded, v);
    }

    #[test]
    fn test_roundtrip_list() {
        let v = PickleValue::List(vec![
            PickleValue::Int(1),
            PickleValue::String("two".to_string()),
            PickleValue::Bool(true),
        ]);
        let encoded = pickle_serialize(&v).unwrap();
        let decoded = pickle_deserialize(&encoded).unwrap();
        assert_eq!(decoded, v);
    }

    #[test]
    fn test_roundtrip_empty_list() {
        let v = PickleValue::List(vec![]);
        let encoded = pickle_serialize(&v).unwrap();
        let decoded = pickle_deserialize(&encoded).unwrap();
        assert_eq!(decoded, v);
    }

    #[test]
    fn test_roundtrip_tuple() {
        // Test all tuple size paths: 0, 1, 2, 3, 4+
        for n in [0, 1, 2, 3, 5] {
            let items: Vec<PickleValue> = (0..n).map(|i| PickleValue::Int(i)).collect();
            let v = PickleValue::Tuple(items);
            let encoded = pickle_serialize(&v).unwrap();
            let decoded = pickle_deserialize(&encoded).unwrap();
            assert_eq!(decoded, v);
        }
    }

    #[test]
    fn test_roundtrip_dict() {
        let v = PickleValue::Dict(vec![
            (
                PickleValue::String("key1".to_string()),
                PickleValue::Int(42),
            ),
            (
                PickleValue::String("key2".to_string()),
                PickleValue::Bool(false),
            ),
        ]);
        let encoded = pickle_serialize(&v).unwrap();
        let decoded = pickle_deserialize(&encoded).unwrap();
        assert_eq!(decoded, v);
    }

    #[test]
    fn test_roundtrip_nested() {
        let v = PickleValue::List(vec![
            PickleValue::Dict(vec![(
                PickleValue::String("nested".to_string()),
                PickleValue::List(vec![PickleValue::Int(1), PickleValue::Int(2)]),
            )]),
            PickleValue::None,
        ]);
        let encoded = pickle_serialize(&v).unwrap();
        let decoded = pickle_deserialize(&encoded).unwrap();
        assert_eq!(decoded, v);
    }

    #[test]
    fn test_protocol_header() {
        let encoded = pickle_serialize(&PickleValue::None).unwrap();
        assert_eq!(encoded[0], PROTO);
        assert_eq!(encoded[1], 2); // protocol version
        assert_eq!(*encoded.last().unwrap(), STOP);
    }

    #[test]
    fn test_from_conversions() {
        assert_eq!(PickleValue::from(42i32), PickleValue::Int(42));
        assert_eq!(PickleValue::from(42i64), PickleValue::Int(42));
        assert_eq!(PickleValue::from(42u32), PickleValue::Int(42));
        assert_eq!(PickleValue::from(3.14f64), PickleValue::Float(3.14));
        assert_eq!(PickleValue::from(true), PickleValue::Bool(true));
        assert_eq!(
            PickleValue::from("hello"),
            PickleValue::String("hello".to_string())
        );
        assert_eq!(
            PickleValue::from("hello".to_string()),
            PickleValue::String("hello".to_string())
        );
    }

    #[test]
    fn test_decode_empty_data_errors() {
        let result = pickle_deserialize(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_invalid_opcode() {
        let data = [PROTO, 2, 0xFF, STOP];
        let result = pickle_deserialize(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_long_to_bytes_zero() {
        assert_eq!(long_to_bytes(0), Vec::<u8>::new());
    }

    #[test]
    fn test_long_to_bytes_positive() {
        // 128 needs [0x80, 0x00] to avoid looking negative
        let bytes = long_to_bytes(128);
        assert_eq!(bytes_to_long(&bytes), 128);
    }

    #[test]
    fn test_long_to_bytes_negative() {
        let bytes = long_to_bytes(-1);
        assert_eq!(bytes_to_long(&bytes), -1);

        let bytes = long_to_bytes(-128);
        assert_eq!(bytes_to_long(&bytes), -128);
    }

    #[test]
    fn test_bytes_long_empty() {
        assert_eq!(bytes_to_long(&[]), 0);
    }
}
