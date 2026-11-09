//! Wire-format encoder and response reader for the probe.
//!
//! Mirrors the layout documented in
//! vendor/pdf_oxide/src/host/binary_codec.rs exactly:
//!
//!   Request:  [op_len:u8] [op:utf8] [num_fields:u16le] [fields...]
//!   Field:    [key_len:u8] [key:utf8] [type:u8] [value...]
//!   Response: [status:u8] [num_fields:u16le] [fields...]
//!             status 1 = ok, 0 = error (kind u8 first, `message`
//!             string field always present), 2 = cancelled.
//!   Type codes: 0 null 1 i32 2 i64 3 f64 4 bool 5 string 6 bytes
//!               7 int_list 9 string_list (u32le length prefixes),
//!               10 map_list (skipped when read).

/// One typed field value the probe may send or read back.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    I32(i32),
    I64(i64),
    F64(f64),
    Bool(bool),
    Str(String),
    Bytes(Vec<u8>),
    IntList(Vec<i32>),
    StrList(Vec<String>),
}

/// One wire request: an op name and typed fields.
pub struct Request {
    pub op: String,
    pub fields: Vec<(String, Value)>,
}

impl Request {
    pub fn new(op: &str) -> Self {
        Request { op: op.to_string(), fields: Vec::new() }
    }

    pub fn put(mut self, key: &str, value: Value) -> Self {
        self.fields.push((key.to_string(), value));
        self
    }

    /// Encodes the request per the layout above.
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::new();
        let op = self.op.as_bytes();
        b.push(op.len() as u8);
        b.extend_from_slice(op);
        b.extend_from_slice(&(self.fields.len() as u16).to_le_bytes());
        for (key, value) in &self.fields {
            let k = key.as_bytes();
            b.push(k.len() as u8);
            b.extend_from_slice(k);
            write_value(&mut b, value);
        }
        b
    }
}

fn write_value(b: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Null => b.push(0),
        Value::I32(n) => {
            b.push(1);
            b.extend_from_slice(&n.to_le_bytes());
        }
        Value::I64(n) => {
            b.push(2);
            b.extend_from_slice(&n.to_le_bytes());
        }
        Value::F64(n) => {
            b.push(3);
            b.extend_from_slice(&n.to_le_bytes());
        }
        Value::Bool(v) => {
            b.push(4);
            b.push(if *v { 1 } else { 0 });
        }
        Value::Str(s) => {
            b.push(5);
            let bytes = s.as_bytes();
            b.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            b.extend_from_slice(bytes);
        }
        Value::Bytes(v) => {
            b.push(6);
            b.extend_from_slice(&(v.len() as u32).to_le_bytes());
            b.extend_from_slice(v);
        }
        Value::IntList(list) => {
            b.push(7);
            b.extend_from_slice(&(list.len() as u32).to_le_bytes());
            for n in list {
                b.extend_from_slice(&n.to_le_bytes());
            }
        }
        Value::StrList(list) => {
            b.push(9);
            b.extend_from_slice(&(list.len() as u32).to_le_bytes());
            for s in list {
                let bytes = s.as_bytes();
                b.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                b.extend_from_slice(bytes);
            }
        }
    }
}

/// A decoded ok response: the fields the probe cares about, by key.
#[derive(Debug)]
pub struct OkResponse {
    pub fields: Vec<(String, Value)>,
}

impl OkResponse {
    pub fn i32(&self, key: &str) -> Option<i32> {
        self.fields.iter().find_map(|(k, v)| match v {
            Value::I32(n) if k == key => Some(*n),
            Value::I64(n) if k == key => Some(*n as i32),
            _ => None,
        })
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.fields.iter().find_map(|(k, v)| match v {
            Value::Str(s) if k == key => Some(s.as_str()),
            _ => None,
        })
    }

    pub fn bytes(&self, key: &str) -> Option<&[u8]> {
        self.fields.iter().find_map(|(k, v)| match v {
            Value::Bytes(b) if k == key => Some(b.as_slice()),
            _ => None,
        })
    }
}

/// Reads one ok response or turns an error/cancelled response into a
/// printable Err(message). A length-prefixed slice shorter than its
/// prefix claims returns Err without panicking.
pub fn parse_response(bytes: &[u8]) -> Result<OkResponse, String> {
    let mut r = Reader { data: bytes, pos: 0 };
    let status = r.u8()?;
    match status {
        1 => read_fields(&mut r)
            .map(|fields| OkResponse { fields })
            .map_err(|e: &'static str| e.to_string()),
        0 => {
            let kind = r.u8()?;
            let fields = read_fields(&mut r).map_err(|e| e.to_string())?;
            let message = fields
                .iter()
                .find_map(|(k, v)| match v {
                    Value::Str(s) if k == "message" => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "no message".to_string());
            Err(format!("engine error kind {kind}: {message}"))
        }
        2 => Err("cancelled".to_string()),
        other => Err(format!("unknown response status {other}")),
    }
}

/// A reader over unsized slices; every read checks the slice end so a
/// malformed response surfaces as Err, never as a panic or an index
/// past the end.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

type ReadResult<T> = Result<T, &'static str>;

impl<'a> Reader<'a> {
    fn expect(&self, n: usize) -> ReadResult<()> {
        if self.pos + n > self.data.len() {
            return Err("response ended early");
        }
        Ok(())
    }

    fn u8(&mut self) -> ReadResult<u8> {
        self.expect(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn u16(&mut self) -> ReadResult<u16> {
        self.expect(2)?;
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn u32(&mut self) -> ReadResult<u32> {
        self.expect(4)?;
        let v = u32::from_le_bytes(
            self.data[self.pos..self.pos + 4].try_into().expect("checked by expect"),
        );
        self.pos += 4;
        Ok(v)
    }

    fn i32(&mut self) -> ReadResult<i32> {
        self.expect(4)?;
        let v = i32::from_le_bytes(
            self.data[self.pos..self.pos + 4].try_into().expect("checked by expect"),
        );
        self.pos += 4;
        Ok(v)
    }

    fn i64(&mut self) -> ReadResult<i64> {
        self.expect(8)?;
        let v = i64::from_le_bytes(
            self.data[self.pos..self.pos + 8].try_into().expect("checked by expect"),
        );
        self.pos += 8;
        Ok(v)
    }

    fn f64(&mut self) -> ReadResult<f64> {
        self.expect(8)?;
        let v = f64::from_le_bytes(
            self.data[self.pos..self.pos + 8].try_into().expect("checked by expect"),
        );
        self.pos += 8;
        Ok(v)
    }

    fn slice(&mut self, n: usize) -> ReadResult<&'a [u8]> {
        self.expect(n)?;
        let v = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(v)
    }

    fn utf8(&mut self, n: usize) -> ReadResult<String> {
        std::str::from_utf8(self.slice(n)?)
            .map(|s| s.to_string())
            .map_err(|_| "invalid utf8 in response")
    }
}

fn read_fields(r: &mut Reader<'_>) -> ReadResult<Vec<(String, Value)>> {
    let count = r.u16()? as usize;
    let mut fields = Vec::with_capacity(count);
    for _ in 0..count {
        let kl = r.u8()? as usize;
        let key = r.utf8(kl)?;
        let value = read_value(r)?;
        fields.push((key, value));
    }
    Ok(fields)
}

fn read_value(r: &mut Reader<'_>) -> ReadResult<Value> {
    let type_code = r.u8()?;
    match type_code {
        0 => Ok(Value::Null),
        1 => Ok(Value::I32(r.i32()?)),
        2 => Ok(Value::I64(r.i64()?)),
        3 => Ok(Value::F64(r.f64()?)),
        4 => Ok(Value::Bool(r.u8()? != 0)),
        5 => {
            let len = r.u32()? as usize;
            Ok(Value::Str(r.utf8(len)?))
        }
        6 => {
            let len = r.u32()? as usize;
            Ok(Value::Bytes(r.slice(len)?.to_vec()))
        }
        7 => {
            let count = r.u32()? as usize;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                list.push(r.i32()?);
            }
            Ok(Value::IntList(list))
        }
        9 => {
            let count = r.u32()? as usize;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                let slen = r.u32()? as usize;
                list.push(r.utf8(slen)?);
            }
            Ok(Value::StrList(list))
        }
        // A list of maps (reduceImages' report): skipped whole, the probe reads
        // its effect from the file the op wrote.
        10 => {
            let count = r.u32()? as usize;
            for _ in 0..count {
                let map_len = r.u32()? as usize;
                r.slice(map_len)?;
            }
            Ok(Value::Null)
        }
        _ => Ok(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The encoder's bytes equal one request assembled by hand, byte
    /// for byte, per the layout in binary_codec.rs's module doc.
    #[test]
    fn encoder_matches_hand_built_request() {
        let built = Request::new("open").put("handleId", Value::I32(3)).put("compress", Value::Bool(true)).encode();
        let mut hand = Vec::new();
        hand.push(4u8);
        hand.extend_from_slice(b"open");
        hand.extend_from_slice(&2u16.to_le_bytes());
        hand.push(8u8);
        hand.extend_from_slice(b"handleId");
        hand.push(1u8);
        hand.extend_from_slice(&3i32.to_le_bytes());
        hand.push(8u8);
        hand.extend_from_slice(b"compress");
        hand.push(4u8);
        hand.push(1u8);
        assert_eq!(built, hand);
    }

    #[test]
    fn error_response_becomes_err_message() {
        // hand-built error: status 0, kind 2 (io), one field message
        let mut bytes = vec![0u8, 2u8];
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.push(7u8);
        bytes.extend_from_slice(b"message");
        bytes.push(5u8);
        bytes.extend_from_slice(&5u32.to_le_bytes());
        bytes.extend_from_slice(b"crazy");
        let err = parse_response(&bytes).unwrap_err();
        assert_eq!(err, "engine error kind 2: crazy");
    }

    #[test]
    fn truncated_response_is_err_not_panic() {
        assert!(parse_response(&[1u8]).is_err());
        assert!(parse_response(&[]).is_err());
    }
}
