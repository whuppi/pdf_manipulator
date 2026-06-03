//! Binary wire format codec — parse requests and encode responses.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Mirrors `binary_codec.dart` exactly. Same format, same type codes,
//! same byte order (little-endian). A request encoded by Dart is parsed
//! here; a response encoded here is decoded by Dart.
//!
//! Format:
//!   Request:  [op_len:u8] [op:utf8] [num_fields:u16le] [fields...]
//!   Response: [status:u8] [num_fields:u16le] [fields...]
//!             status 0 = error: [msg_len:u32le] [msg:utf8]
//!             status 1 = ok:    [fields...]
//!   Field:    [key_len:u8] [key:utf8] [type:u8] [value...]
//!
//! Type codes:
//!   0=null  1=i32  2=i64  3=f64  4=bool  5=string  6=bytes
//!   7=int_list  8=float_list  9=string_list  10=map_list

// ═══════════════════════════════════════════════════════════════════
// Request parser
// ═══════════════════════════════════════════════════════════════════

/// A parsed binary request with an operation name and typed fields.
pub struct Request<'a> {
    op: &'a str,
    fields: Vec<(&'a str, FieldValue<'a>)>,
}

/// A typed field value decoded from a binary request.
pub enum FieldValue<'a> {
    /// Null / absent value (type code 0).
    Null,
    /// 32-bit signed integer (type code 1).
    I32(i32),
    /// 64-bit signed integer (type code 2).
    I64(i64),
    /// 64-bit floating point (type code 3).
    F64(f64),
    /// Boolean (type code 4).
    Bool(bool),
    /// UTF-8 string slice (type code 5).
    Str(&'a str),
    /// Raw byte slice (type code 6).
    Bytes(&'a [u8]),
    /// List of i32 values (type code 7).
    IntList(Vec<i32>),
    /// List of f64 values (type code 8).
    F64List(Vec<f64>),
    /// List of string slices (type code 9).
    StringList(Vec<&'a str>),
    /// List of key-value maps (type code 10).
    MapList(Vec<Vec<(&'a str, FieldValue<'a>)>>),
}

impl<'a> Request<'a> {
    /// Parse a binary request from raw bytes.
    pub fn parse(data: &'a [u8]) -> Result<Self, &'static str> {
        let mut r = Reader::new(data);
        let op_len = r.u8()? as usize;
        let op = r.utf8(op_len)?;
        let field_count = r.u16()? as usize;
        let mut fields = Vec::with_capacity(field_count);
        for _ in 0..field_count {
            let key_len = r.u8()? as usize;
            let key = r.utf8(key_len)?;
            let value = read_value(&mut r)?;
            fields.push((key, value));
        }
        Ok(Request { op, fields })
    }

    /// Return the operation name.
    pub fn op(&self) -> &str {
        self.op
    }

    /// Get a string field by key.
    pub fn get_str(&self, key: &str) -> Option<&'a str> {
        self.fields.iter().find_map(|(k, v)| {
            if *k == key { if let FieldValue::Str(s) = v { Some(*s) } else { None } } else { None }
        })
    }

    /// Get an i32 field by key (also accepts i64, truncated).
    pub fn get_i32(&self, key: &str) -> Option<i32> {
        self.fields.iter().find_map(|(k, v)| {
            if *k == key {
                match v {
                    FieldValue::I32(n) => Some(*n),
                    FieldValue::I64(n) => Some(*n as i32),
                    _ => None,
                }
            } else { None }
        })
    }

    /// Get an i64 field by key (also accepts i32, widened).
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.fields.iter().find_map(|(k, v)| {
            if *k == key {
                match v {
                    FieldValue::I64(n) => Some(*n),
                    FieldValue::I32(n) => Some(*n as i64),
                    _ => None,
                }
            } else { None }
        })
    }

    /// Get an f64 field by key (also accepts i32/i64, widened).
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.fields.iter().find_map(|(k, v)| {
            if *k == key {
                match v {
                    FieldValue::F64(n) => Some(*n),
                    // dart2js/dart2wasm may encode whole-number doubles as i32/i64
                    FieldValue::I32(n) => Some(*n as f64),
                    FieldValue::I64(n) => Some(*n as f64),
                    _ => None,
                }
            } else { None }
        })
    }

    /// Get a bool field by key.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.fields.iter().find_map(|(k, v)| {
            if *k == key { if let FieldValue::Bool(b) = v { Some(*b) } else { None } } else { None }
        })
    }

    /// Get a byte-slice field by key.
    pub fn get_bytes(&self, key: &str) -> Option<&'a [u8]> {
        self.fields.iter().find_map(|(k, v)| {
            if *k == key { if let FieldValue::Bytes(b) = v { Some(*b) } else { None } } else { None }
        })
    }

    /// Get an i32 list field by key.
    pub fn get_int_list(&self, key: &str) -> Option<&[i32]> {
        self.fields.iter().find_map(|(k, v)| {
            if *k == key { if let FieldValue::IntList(l) = v { Some(l.as_slice()) } else { None } } else { None }
        })
    }

    /// Get an f64 list field by key.
    pub fn get_f64_list(&self, key: &str) -> Option<&[f64]> {
        self.fields.iter().find_map(|(k, v)| {
            if *k == key { if let FieldValue::F64List(l) = v { Some(l.as_slice()) } else { None } } else { None }
        })
    }

    /// Get a string list field by key.
    pub fn get_string_list(&self, key: &str) -> Option<Vec<&'a str>> {
        self.fields.iter().find_map(|(k, v)| {
            if *k == key { if let FieldValue::StringList(l) = v { Some(l.clone()) } else { None } } else { None }
        })
    }
}

fn read_value<'a>(r: &mut Reader<'a>) -> Result<FieldValue<'a>, &'static str> {
    let type_code = r.u8()?;
    match type_code {
        0 => Ok(FieldValue::Null),
        1 => Ok(FieldValue::I32(r.i32()?)),
        2 => Ok(FieldValue::I64(r.i64()?)),
        3 => Ok(FieldValue::F64(r.f64()?)),
        4 => Ok(FieldValue::Bool(r.u8()? != 0)),
        5 => {
            let len = r.u32()? as usize;
            Ok(FieldValue::Str(r.utf8(len)?))
        }
        6 => {
            let len = r.u32()? as usize;
            Ok(FieldValue::Bytes(r.slice(len)?))
        }
        7 => {
            let count = r.u32()? as usize;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count { list.push(r.i32()?); }
            Ok(FieldValue::IntList(list))
        }
        8 => {
            let count = r.u32()? as usize;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count { list.push(r.f64()?); }
            Ok(FieldValue::F64List(list))
        }
        9 => {
            let count = r.u32()? as usize;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                let slen = r.u32()? as usize;
                list.push(r.utf8(slen)?);
            }
            Ok(FieldValue::StringList(list))
        }
        10 => {
            let count = r.u32()? as usize;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                let map_len = r.u32()? as usize;
                let map_bytes = r.slice(map_len)?;
                let mut mr = Reader::new(map_bytes);
                let fc = mr.u16()? as usize;
                let mut fields = Vec::with_capacity(fc);
                for _ in 0..fc {
                    let kl = mr.u8()? as usize;
                    let key = mr.utf8(kl)?;
                    let val = read_value(&mut mr)?;
                    fields.push((key, val));
                }
                list.push(fields);
            }
            Ok(FieldValue::MapList(list))
        }
        _ => Ok(FieldValue::Null),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Response writer
// ═══════════════════════════════════════════════════════════════════

/// Builds a binary response with typed key-value fields.
pub struct ResponseWriter {
    buf: Vec<u8>,
}

impl ResponseWriter {
    /// Create a new success response writer.
    pub fn ok() -> Self {
        let mut buf = Vec::with_capacity(256);
        buf.push(1); // status = ok
        buf.push(0); buf.push(0); // field count placeholder (patched in finish)
        ResponseWriter { buf }
    }

    /// Encode a complete error response with the given message.
    pub fn error(msg: &str) -> Vec<u8> {
        let msg_bytes = msg.as_bytes();
        let mut buf = Vec::with_capacity(5 + msg_bytes.len());
        buf.push(0); // status = error
        buf.extend_from_slice(&(msg_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(msg_bytes);
        buf
    }

    /// Write an i32 field.
    pub fn put_i32(&mut self, key: &str, val: i32) {
        self.write_key(key);
        self.buf.push(1);
        self.buf.extend_from_slice(&val.to_le_bytes());
        self.inc_count();
    }

    /// Write an i64 field.
    pub fn put_i64(&mut self, key: &str, val: i64) {
        self.write_key(key);
        self.buf.push(2);
        self.buf.extend_from_slice(&val.to_le_bytes());
        self.inc_count();
    }

    /// Write an f64 field.
    pub fn put_f64(&mut self, key: &str, val: f64) {
        self.write_key(key);
        self.buf.push(3);
        self.buf.extend_from_slice(&val.to_le_bytes());
        self.inc_count();
    }

    /// Write a bool field.
    pub fn put_bool(&mut self, key: &str, val: bool) {
        self.write_key(key);
        self.buf.push(4);
        self.buf.push(if val { 1 } else { 0 });
        self.inc_count();
    }

    /// Write a string field.
    pub fn put_str(&mut self, key: &str, val: &str) {
        self.write_key(key);
        self.buf.push(5);
        let bytes = val.as_bytes();
        self.buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        self.buf.extend_from_slice(bytes);
        self.inc_count();
    }

    /// Write a byte-slice field.
    pub fn put_bytes(&mut self, key: &str, val: &[u8]) {
        self.write_key(key);
        self.buf.push(6);
        self.buf.extend_from_slice(&(val.len() as u32).to_le_bytes());
        self.buf.extend_from_slice(val);
        self.inc_count();
    }

    /// Write an i32 list field.
    pub fn put_int_list(&mut self, key: &str, val: &[i32]) {
        self.write_key(key);
        self.buf.push(7);
        self.buf.extend_from_slice(&(val.len() as u32).to_le_bytes());
        for &n in val {
            self.buf.extend_from_slice(&n.to_le_bytes());
        }
        self.inc_count();
    }

    /// Write a list of map (key-value) entries.
    pub fn put_map_list<F>(&mut self, key: &str, count: usize, mut write_item: F)
    where F: FnMut(usize, &mut ResponseWriter)
    {
        self.write_key(key);
        self.buf.push(10);
        self.buf.extend_from_slice(&(count as u32).to_le_bytes());
        for i in 0..count {
            let mut item = ResponseWriter::ok();
            write_item(i, &mut item);
            let item_bytes = item.finish_inner();
            self.buf.extend_from_slice(&(item_bytes.len() as u32).to_le_bytes());
            self.buf.extend_from_slice(&item_bytes);
        }
        self.inc_count();
    }

    /// Consume the writer and return the encoded response bytes.
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }

    fn finish_inner(self) -> Vec<u8> {
        // Return the bytes AFTER the status byte (for nested map encoding)
        self.buf[1..].to_vec()
    }

    fn write_key(&mut self, key: &str) {
        let bytes = key.as_bytes();
        self.buf.push(bytes.len() as u8);
        self.buf.extend_from_slice(bytes);
    }

    fn inc_count(&mut self) {
        let count = u16::from_le_bytes([self.buf[1], self.buf[2]]);
        let new_count = count + 1;
        let le = new_count.to_le_bytes();
        self.buf[1] = le[0];
        self.buf[2] = le[1];
    }
}

// ═══════════════════════════════════════════════════════════════════
// Low-level reader
// ═══════════════════════════════════════════════════════════════════

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn u8(&mut self) -> Result<u8, &'static str> {
        if self.remaining() < 1 { return Err("unexpected end of data"); }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16, &'static str> {
        if self.remaining() < 2 { return Err("unexpected end of data"); }
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        if self.remaining() < 4 { return Err("unexpected end of data"); }
        let bytes: [u8; 4] = self.data[self.pos..self.pos + 4].try_into().unwrap();
        let v = u32::from_le_bytes(bytes);
        self.pos += 4;
        Ok(v)
    }

    fn i32(&mut self) -> Result<i32, &'static str> {
        if self.remaining() < 4 { return Err("unexpected end of data"); }
        let bytes: [u8; 4] = self.data[self.pos..self.pos + 4].try_into().unwrap();
        let v = i32::from_le_bytes(bytes);
        self.pos += 4;
        Ok(v)
    }

    fn i64(&mut self) -> Result<i64, &'static str> {
        if self.remaining() < 8 { return Err("unexpected end of data"); }
        let bytes: [u8; 8] = self.data[self.pos..self.pos + 8].try_into().unwrap();
        let v = i64::from_le_bytes(bytes);
        self.pos += 8;
        Ok(v)
    }

    fn f64(&mut self) -> Result<f64, &'static str> {
        if self.remaining() < 8 { return Err("unexpected end of data"); }
        let bytes: [u8; 8] = self.data[self.pos..self.pos + 8].try_into().unwrap();
        let v = f64::from_le_bytes(bytes);
        self.pos += 8;
        Ok(v)
    }

    fn slice(&mut self, len: usize) -> Result<&'a [u8], &'static str> {
        if self.remaining() < len { return Err("unexpected end of data"); }
        let s = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(s)
    }

    fn utf8(&mut self, len: usize) -> Result<&'a str, &'static str> {
        let bytes = self.slice(len)?;
        std::str::from_utf8(bytes).map_err(|_| "invalid utf8")
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_request() {
        // Encode: op="open", 1 field: "password" = "secret"
        let mut data = Vec::new();
        data.push(4); // op_len
        data.extend_from_slice(b"open");
        data.extend_from_slice(&1u16.to_le_bytes()); // 1 field
        data.push(8); // key_len
        data.extend_from_slice(b"password");
        data.push(5); // type = string
        let val = b"secret";
        data.extend_from_slice(&(val.len() as u32).to_le_bytes());
        data.extend_from_slice(val);

        let req = Request::parse(&data).unwrap();
        assert_eq!(req.op(), "open");
        assert_eq!(req.get_str("password"), Some("secret"));
    }

    #[test]
    fn parse_multiple_types() {
        let mut data = Vec::new();
        data.push(4);
        data.extend_from_slice(b"test");
        data.extend_from_slice(&3u16.to_le_bytes()); // 3 fields

        // field 1: "page" = i32(5)
        data.push(4); data.extend_from_slice(b"page");
        data.push(1); data.extend_from_slice(&5i32.to_le_bytes());

        // field 2: "scale" = f64(1.5)
        data.push(5); data.extend_from_slice(b"scale");
        data.push(3); data.extend_from_slice(&1.5f64.to_le_bytes());

        // field 3: "compress" = bool(true)
        data.push(8); data.extend_from_slice(b"compress");
        data.push(4); data.push(1);

        let req = Request::parse(&data).unwrap();
        assert_eq!(req.op(), "test");
        assert_eq!(req.get_i32("page"), Some(5));
        assert_eq!(req.get_f64("scale"), Some(1.5));
        assert_eq!(req.get_bool("compress"), Some(true));
    }

    #[test]
    fn response_writer_ok() {
        let mut w = ResponseWriter::ok();
        w.put_i32("pageCount", 10);
        w.put_str("version", "2.0");
        w.put_bool("isEncrypted", false);
        let bytes = w.finish();

        assert_eq!(bytes[0], 1); // status ok
        let field_count = u16::from_le_bytes([bytes[1], bytes[2]]);
        assert_eq!(field_count, 3);
    }

    #[test]
    fn response_writer_error() {
        let bytes = ResponseWriter::error("something broke");
        assert_eq!(bytes[0], 0); // status error
        let msg_len = u32::from_le_bytes(bytes[1..5].try_into().unwrap()) as usize;
        let msg = std::str::from_utf8(&bytes[5..5 + msg_len]).unwrap();
        assert_eq!(msg, "something broke");
    }

    #[test]
    fn round_trip_bytes_field() {
        let mut data = Vec::new();
        data.push(4);
        data.extend_from_slice(b"test");
        data.extend_from_slice(&1u16.to_le_bytes());
        data.push(4); data.extend_from_slice(b"data");
        data.push(6); // bytes
        let payload = [0xDE, 0xAD, 0xBE, 0xEF];
        data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        data.extend_from_slice(&payload);

        let req = Request::parse(&data).unwrap();
        assert_eq!(req.get_bytes("data"), Some(&payload[..]));
    }

    #[test]
    fn round_trip_int_list() {
        let mut data = Vec::new();
        data.push(4);
        data.extend_from_slice(b"test");
        data.extend_from_slice(&1u16.to_le_bytes());
        data.push(5); data.extend_from_slice(b"pages");
        data.push(7); // int_list
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());
        data.extend_from_slice(&5i32.to_le_bytes());
        data.extend_from_slice(&10i32.to_le_bytes());

        let req = Request::parse(&data).unwrap();
        assert_eq!(req.get_int_list("pages"), Some(&[0, 5, 10][..]));
    }
}
