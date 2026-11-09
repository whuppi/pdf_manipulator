//! The DCT-domain copy of a JPEG for redaction: the quantised coefficients
//! are read one MCU row at a time and written back as a baseline JPEG, so no
//! pixel is decoded and every block the redaction does not cover keeps its
//! coefficients exactly.
//!
//! Accepted: 8-bit Huffman frames, baseline (SOF0, SOF1) or progressive (SOF2),
//! with one, three or four components. Any other frame is refused by
//! [`Jpeg::read`]. A progressive frame's scans are decoded in lockstep: each
//! scan has its own decoder over the stream, all advanced one MCU row at a
//! time, so a refinement scan sees what the earlier scans left in the same row
//! and memory is one MCU row of coefficients plus the decoders' state.
//!
//! Coefficients are held in zigzag order, the order the entropy coder codes
//! them, so the block buffers need no reordering.

use std::io::{self, BufReader, Read, Write};

use crate::cancel::CancelCheck;
use crate::encryption::stream::from_io;
use crate::error::{Error, Result};
use crate::object::StreamBody;

/// Annex K.3 Huffman tables, the baseline writer's: index 0 luminance, 1 chrominance.
const DC_BITS: [[u8; 16]; 2] = [
    [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
];
const DC_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const AC_BITS: [[u8; 16]; 2] = [
    [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7D],
    [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77],
];
const AC_VALUES: [[u8; 162]; 2] = [
    [
        0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
        0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0,
        0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
        0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
        0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
        0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
        0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
        0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5,
        0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
        0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
        0xF9, 0xFA,
    ],
    [
        0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
        0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0,
        0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24, 0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26,
        0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
        0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
        0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
        0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5,
        0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3,
        0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA,
        0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
        0xF9, 0xFA,
    ],
];

const EOI: u8 = 0xD9;

fn refuse(why: &str) -> Error {
    Error::Unsupported(format!(
        "the JPEG under this redaction cannot be copied in the DCT domain: {why}; refusing to \
         redact rather than decode it whole"
    ))
}

fn corrupt(why: &str) -> Error {
    Error::Decode(format!("the JPEG under this redaction is damaged: {why}"))
}

/// One Huffman table as its code lengths and symbols, with the decode bounds of
/// ITU-T T.81 Annex C.
#[derive(Clone)]
struct Huffman {
    bits: [u8; 16],
    values: Vec<u8>,
    maxcode: [i32; 17],
    valptr: [i32; 17],
    mincode: [i32; 17],
}

impl Huffman {
    fn new(bits: [u8; 16], values: Vec<u8>) -> Result<Self> {
        let mut table = Self {
            bits,
            values,
            maxcode: [-1; 17],
            valptr: [0; 17],
            mincode: [0; 17],
        };
        let (mut code, mut index) = (0i32, 0i32);
        for len in 1..=16 {
            let count = i32::from(bits[len - 1]);
            table.valptr[len] = index;
            table.mincode[len] = code;
            code += count;
            index += count;
            if count > 0 {
                table.maxcode[len] = code - 1;
            }
            if code > 1 << len {
                return Err(corrupt("a Huffman table has more codes than its length allows"));
            }
            code <<= 1;
        }
        if index as usize > table.values.len() {
            return Err(corrupt("a Huffman table names more symbols than it lists"));
        }
        Ok(table)
    }

    /// The `(code, length)` of every symbol the table lists, by symbol; a symbol
    /// it does not list has length 0.
    fn encoder(&self) -> [(u32, u32); 256] {
        let mut out = [(0u32, 0u32); 256];
        let (mut code, mut index) = (0u32, 0usize);
        for len in 1..=16u32 {
            for _ in 0..self.bits[len as usize - 1] {
                out[usize::from(self.values[index])] = (code, len);
                code += 1;
                index += 1;
            }
            code <<= 1;
        }
        out
    }
}

/// One component of the frame. `h` and `v` are its sampling factors; a
/// one-component frame has 1 and 1, since its scans are block-interleaved.
#[derive(Clone, Copy)]
struct Component {
    id: u8,
    h: u32,
    v: u32,
    tq: usize,
}

/// The frame header (SOF) with the derived MCU geometry.
struct Frame {
    width: u32,
    height: u32,
    components: Vec<Component>,
    hmax: u32,
    vmax: u32,
    mcus_x: u32,
    mcus_y: u32,
}

impl Frame {
    /// The component's blocks across and down its own sample grid, the grid its
    /// samples cover: a non-interleaved scan codes exactly these.
    fn blocks(&self, c: Component) -> (u32, u32) {
        let samples_x = (self.width * c.h).div_ceil(self.hmax);
        let samples_y = (self.height * c.v).div_ceil(self.vmax);
        (samples_x.div_ceil(8), samples_y.div_ceil(8))
    }

    /// The blocks one MCU row holds across a component's plane, MCU padding
    /// included.
    fn stride(&self, c: Component) -> u32 {
        self.mcus_x * c.h
    }

    fn planes(&self) -> Vec<Vec<i16>> {
        self.components
            .iter()
            .map(|c| vec![0i16; (self.stride(*c) * c.v) as usize * 64])
            .collect()
    }
}

/// One component of a scan, with the tables it codes with.
struct ScanComponent {
    index: usize,
    dc: Option<Huffman>,
    ac: Option<Huffman>,
}

/// One scan's header and the place its entropy-coded data starts.
struct Scan {
    comps: Vec<ScanComponent>,
    ss: u32,
    se: u32,
    ah: u32,
    al: u32,
    restart: u32,
    offset: u64,
}

/// The JPEG's metadata, read by a walk over its markers. Scan data is never
/// held: the walk passes over it and records where each scan starts.
pub(crate) struct Jpeg {
    frame: Frame,
    /// The DQT segments before the first scan, copied as they are.
    quant: Vec<Vec<u8>>,
    /// The DC quantiser of each table id, the one a flat block's DC divides by.
    dc_quant: [u32; 4],
    /// The APP14 (Adobe) payload when the frame has one.
    app14: Option<Vec<u8>>,
    scans: Vec<Scan>,
}

/// The sequential walk over the stream's markers.
struct Walk<'a> {
    input: BufReader<Box<dyn Read + 'a>>,
    pos: u64,
    pending: Option<u8>,
}

impl Walk<'_> {
    fn byte(&mut self) -> Result<Option<u8>> {
        let mut one = [0u8; 1];
        loop {
            match self.input.read(&mut one) {
                Ok(0) => return Ok(None),
                Ok(_) => {
                    self.pos += 1;
                    return Ok(Some(one[0]));
                },
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {},
                Err(e) => return Err(from_io(e)),
            }
        }
    }

    fn need(&mut self) -> Result<u8> {
        self.byte()?
            .ok_or_else(|| corrupt("the stream ends inside its headers"))
    }

    fn take(&mut self, n: usize) -> Result<Vec<u8>> {
        let mut out = vec![0u8; n];
        self.input.read_exact(&mut out).map_err(from_io)?;
        self.pos += n as u64;
        Ok(out)
    }

    /// The next marker byte, skipping fill bytes; `None` at the end of the stream.
    fn marker(&mut self) -> Result<Option<u8>> {
        if let Some(m) = self.pending.take() {
            return Ok(Some(m));
        }
        loop {
            match self.byte()? {
                None => return Ok(None),
                Some(0xFF) => break,
                Some(_) => {},
            }
        }
        let mut m = self.byte()?;
        while m == Some(0xFF) {
            m = self.byte()?;
        }
        Ok(m)
    }

    /// A segment's payload: its length field less the two length bytes.
    fn segment(&mut self) -> Result<Vec<u8>> {
        let len = usize::from(u16::from_be_bytes([self.need()?, self.need()?]));
        if len < 2 {
            return Err(corrupt("a segment's length is below its own field"));
        }
        self.take(len - 2)
    }

    /// Passes the entropy-coded data after a scan header, up to the next marker
    /// that is not a stuffed byte or a restart. That marker is kept pending.
    fn skip_entropy(&mut self) -> Result<()> {
        while let Some(b) = self.byte()? {
            if b != 0xFF {
                continue;
            }
            let mut c = self.byte()?;
            while c == Some(0xFF) {
                c = self.byte()?;
            }
            match c {
                None => return Ok(()),
                Some(0) => {},
                Some(m) if (0xD0..=0xD7).contains(&m) => {},
                Some(m) => {
                    self.pending = Some(m);
                    return Ok(());
                },
            }
        }
        Ok(())
    }
}

/// `extend` of ITU-T T.81 F.2.2.1: the signed value of `s` bits read as `v`.
fn extend(v: u32, s: u32) -> i32 {
    if v < 1 << (s - 1) {
        v as i32 - (1 << s) + 1
    } else {
        v as i32
    }
}

impl Jpeg {
    /// Reads the marker segments of the stream, refusing every frame and table
    /// the copy does not carry. Scan data is passed over, never kept.
    pub(crate) fn read(body: &StreamBody) -> Result<Self> {
        let mut walk = Walk {
            input: BufReader::new(body.raw_reader()?),
            pos: 0,
            pending: None,
        };
        if [walk.need()?, walk.need()?] != [0xFF, 0xD8] {
            return Err(corrupt("the stream does not start with SOI"));
        }
        let mut frame: Option<Frame> = None;
        let mut dc_tables: [Option<Huffman>; 4] = Default::default();
        let mut ac_tables: [Option<Huffman>; 4] = Default::default();
        let mut quant = Vec::new();
        let mut dc_quant = [0u32; 4];
        let mut app14 = None;
        let mut restart = 0u32;
        let mut scans = Vec::new();
        while let Some(m) = walk.marker()? {
            match m {
                EOI => break,
                0x01 | 0xD0..=0xD8 => {},
                0xC0..=0xC2 => {
                    if frame.is_some() {
                        return Err(refuse("the stream has two frame headers"));
                    }
                    frame = Some(parse_frame(&walk.segment()?)?);
                },
                0xC3 | 0xC5..=0xC7 | 0xC8 | 0xC9..=0xCB | 0xCC | 0xCD..=0xCF => {
                    return Err(refuse("the frame is lossless, hierarchical or arithmetic-coded"));
                },
                0xC4 => {
                    let payload = walk.segment()?;
                    let mut at = 0usize;
                    while at < payload.len() {
                        let class = payload[at] >> 4;
                        let slot = usize::from(payload[at] & 3);
                        if payload[at] & 0x0C != 0 || class > 1 {
                            return Err(corrupt("a Huffman table names a class or slot out of range"));
                        }
                        let counts: [u8; 16] = payload
                            .get(at + 1..at + 17)
                            .ok_or_else(|| corrupt("a Huffman table is cut short"))?
                            .try_into()
                            .expect("sixteen counts");
                        let n: usize = counts.iter().map(|&c| usize::from(c)).sum();
                        let values = payload
                            .get(at + 17..at + 17 + n)
                            .ok_or_else(|| corrupt("a Huffman table's symbols are cut short"))?
                            .to_vec();
                        let table = Some(Huffman::new(counts, values)?);
                        if class == 0 {
                            dc_tables[slot] = table;
                        } else {
                            ac_tables[slot] = table;
                        }
                        at += 17 + n;
                    }
                },
                0xDB => {
                    if !scans.is_empty() {
                        return Err(refuse("a quantisation table follows the first scan"));
                    }
                    let payload = walk.segment()?;
                    let mut at = 0usize;
                    while at < payload.len() {
                        let (precision, id) = (payload[at] >> 4, usize::from(payload[at] & 15));
                        if precision > 1 || id > 3 {
                            return Err(corrupt("a quantisation table names a precision or slot out of range"));
                        }
                        let value = if precision == 0 {
                            u32::from(*payload.get(at + 1).ok_or_else(|| corrupt("a quantisation table is cut short"))?)
                        } else {
                            let hi = payload.get(at + 1..at + 3).ok_or_else(|| corrupt("a quantisation table is cut short"))?;
                            u32::from(u16::from_be_bytes([hi[0], hi[1]]))
                        };
                        dc_quant[id] = value;
                        at += 1 + 64 * (1 + usize::from(precision));
                    }
                    quant.push(payload);
                },
                0xDD => {
                    let payload = walk.segment()?;
                    restart = u32::from(u16::from_be_bytes([
                        *payload.first().ok_or_else(|| corrupt("a restart interval is cut short"))?,
                        *payload.get(1).ok_or_else(|| corrupt("a restart interval is cut short"))?,
                    ]));
                },
                0xDA => {
                    let frame = frame.as_ref().ok_or_else(|| corrupt("a scan precedes the frame header"))?;
                    let payload = walk.segment()?;
                    scans.push(parse_scan(&payload, frame, &dc_tables, &ac_tables, restart, walk.pos)?);
                    walk.skip_entropy()?;
                },
                0xEE => {
                    let payload = walk.segment()?;
                    if payload.starts_with(b"Adobe") {
                        app14 = Some(payload);
                    }
                },
                _ => {
                    walk.segment()?;
                },
            }
        }
        let frame = frame.ok_or_else(|| corrupt("the stream has no frame header"))?;
        if scans.is_empty() {
            return Err(corrupt("the stream has no scan"));
        }
        let nc = frame.components.len();
        if !matches!(nc, 1 | 3 | 4) {
            return Err(refuse("the frame has a component count other than one, three or four"));
        }
        for c in &frame.components {
            if dc_quant[c.tq] == 0 {
                return Err(refuse("a component names no quantisation table before the first scan"));
            }
        }
        Ok(Self {
            frame,
            quant,
            dc_quant,
            app14,
            scans,
        })
    }

    /// The frame's `(width, height)`.
    pub(crate) fn size(&self) -> (u32, u32) {
        (self.frame.width, self.frame.height)
    }

    /// The Adobe APP14 colour transform: 0 none, 1 YCbCr, 2 YCCK.
    fn transform(&self) -> Option<u8> {
        self.app14.as_ref().and_then(|p| p.get(11).copied())
    }

    /// The stored sample values of a fill given as straight colour samples: the
    /// YCbCr of an RGB fill, the YCC of a YCCK fill's inverted CMY with K as it
    /// is, and the samples as they are otherwise. The extractor undoes the
    /// decoder's `255 - x` on every CMYK sample, so a CMYK fill is stored as it
    /// is, and so is a YCCK fill's K.
    fn stored(&self, samples: &[u32]) -> Result<Vec<u8>> {
        let nc = self.frame.components.len();
        if samples.len() != nc {
            return Err(Error::Image(format!(
                "a fill has {} samples for a {nc}-component JPEG",
                samples.len()
            )));
        }
        let byte = |v: f64| v.round().clamp(0.0, 255.0) as u8;
        let ycc = |s: &[u32]| {
            let [r, g, b] = [s[0], s[1], s[2]].map(f64::from);
            [
                byte(0.299 * r + 0.587 * g + 0.114 * b),
                byte(128.0 - 0.168_736 * r - 0.331_264 * g + 0.5 * b),
                byte(128.0 + 0.5 * r - 0.418_688 * g - 0.081_312 * b),
            ]
        };
        Ok(match (nc, self.transform()) {
            // The decoder's YCbCr to RGB, read back: the fill's RGB, converted.
            (3, t) if t != Some(0) => ycc(samples).to_vec(),
            // jpeg-decoder hands back the stored YCC's RGB and 255 - K, and the
            // extractor inverts all four: the RGB it reads is the inverse of the
            // fill's CMY, so the YCC is that inverse's YCC.
            (4, Some(2)) => {
                let inverse: Vec<u32> = samples[..3].iter().map(|&s| 255 - s.min(255)).collect();
                let [y, cb, cr] = ycc(&inverse);
                vec![y, cb, cr, samples[3].min(255) as u8]
            },
            _ => samples.iter().map(|&s| s.min(255) as u8).collect(),
        })
    }
}

fn parse_frame(payload: &[u8]) -> Result<Frame> {
    if payload.len() < 6 || payload[0] != 8 {
        return Err(refuse("the frame is not 8-bit"));
    }
    let height = u32::from(u16::from_be_bytes([payload[1], payload[2]]));
    let width = u32::from(u16::from_be_bytes([payload[3], payload[4]]));
    let nc = usize::from(payload[5]);
    if width == 0 || height == 0 {
        return Err(corrupt("the frame has no size"));
    }
    if payload.len() != 6 + 3 * nc {
        return Err(corrupt("the frame header's length does not match its components"));
    }
    let mut components = Vec::with_capacity(nc);
    for i in 0..nc {
        let at = 6 + 3 * i;
        let (h, v) = (u32::from(payload[at + 1] >> 4), u32::from(payload[at + 1] & 15));
        let tq = usize::from(payload[at + 2]);
        if !(1..=4).contains(&h) || !(1..=4).contains(&v) || tq > 3 {
            return Err(corrupt("a component's sampling or quantisation table is out of range"));
        }
        components.push(Component { id: payload[at], h, v, tq });
    }
    if nc == 1 {
        components[0].h = 1;
        components[0].v = 1;
    }
    let hmax = components.iter().map(|c| c.h).max().unwrap_or(1);
    let vmax = components.iter().map(|c| c.v).max().unwrap_or(1);
    Ok(Frame {
        width,
        height,
        components,
        hmax,
        vmax,
        mcus_x: width.div_ceil(8 * hmax),
        mcus_y: height.div_ceil(8 * vmax),
    })
}

fn parse_scan(
    payload: &[u8],
    frame: &Frame,
    dc_tables: &[Option<Huffman>; 4],
    ac_tables: &[Option<Huffman>; 4],
    restart: u32,
    offset: u64,
) -> Result<Scan> {
    let ns = usize::from(*payload.first().ok_or_else(|| corrupt("a scan header is empty"))?);
    if ns == 0 || payload.len() != 4 + 2 * ns {
        return Err(corrupt("a scan header's length does not match its components"));
    }
    let (ss, se) = (u32::from(payload[1 + 2 * ns]), u32::from(payload[2 + 2 * ns]));
    let (ah, al) = (u32::from(payload[3 + 2 * ns] >> 4), u32::from(payload[3 + 2 * ns] & 15));
    if se > 63 || ss > se.max(ss) || (ss == 0 && se > 0 && (se != 63 || ah != 0)) || (ss > 0 && ss > se) {
        return Err(corrupt("a scan's spectral selection is out of range"));
    }
    let mut comps = Vec::with_capacity(ns);
    for i in 0..ns {
        let id = payload[1 + 2 * i];
        let index = frame
            .components
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| corrupt("a scan names a component the frame does not have"))?;
        let (td, ta) = (usize::from(payload[2 + 2 * i] >> 4), usize::from(payload[2 + 2 * i] & 15));
        if td > 3 || ta > 3 {
            return Err(corrupt("a scan names a Huffman table out of range"));
        }
        let need_dc = ss == 0 && ah == 0;
        let need_ac = se > 0;
        let dc = if need_dc {
            Some(dc_tables[td].clone().ok_or_else(|| corrupt("a scan names a DC table never defined"))?)
        } else {
            None
        };
        let ac = if need_ac {
            Some(ac_tables[ta].clone().ok_or_else(|| corrupt("a scan names an AC table never defined"))?)
        } else {
            None
        };
        comps.push(ScanComponent { index, dc, ac });
    }
    if ns > 1 && ss > 0 {
        return Err(corrupt("an interleaved scan carries AC coefficients"));
    }
    Ok(Scan {
        comps,
        ss,
        se,
        ah,
        al,
        restart,
        offset,
    })
}

/// The entropy decoder of one scan, advanced by MCU row.
struct ScanDecoder<'a> {
    scan: &'a Scan,
    frame: &'a Frame,
    input: BufReader<Box<dyn Read + 'a>>,
    acc: u32,
    n: u32,
    /// The marker the data ended at, read as zero bits until a restart takes it.
    marker: Option<u8>,
    eobrun: u32,
    preds: [i32; 4],
    decoded: u32,
}

impl ScanDecoder<'_> {
    fn data_byte(&mut self) -> Result<u8> {
        let Some(b) = self.byte()? else {
            self.marker = Some(EOI);
            return Ok(0);
        };
        if b != 0xFF {
            return Ok(b);
        }
        let mut c = self.byte()?;
        while c == Some(0xFF) {
            c = self.byte()?;
        }
        match c {
            Some(0) => Ok(0xFF),
            Some(m) => {
                self.marker = Some(m);
                Ok(0)
            },
            None => {
                self.marker = Some(EOI);
                Ok(0)
            },
        }
    }

    fn byte(&mut self) -> Result<Option<u8>> {
        let mut one = [0u8; 1];
        loop {
            match self.input.read(&mut one) {
                Ok(0) => return Ok(None),
                Ok(_) => return Ok(Some(one[0])),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {},
                Err(e) => return Err(from_io(e)),
            }
        }
    }

    fn bits(&mut self, count: u32) -> Result<u32> {
        if count == 0 {
            return Ok(0);
        }
        while self.n < count {
            let b = if self.marker.is_some() { 0 } else { self.data_byte()? };
            self.acc = (self.acc << 8) | u32::from(b);
            self.n += 8;
        }
        self.n -= count;
        Ok((self.acc >> self.n) & ((1 << count) - 1))
    }

    fn bit(&mut self) -> Result<u32> {
        self.bits(1)
    }

    fn decode(&mut self, table: &Huffman) -> Result<u8> {
        let mut code = 0i32;
        for len in 1..=16usize {
            code = (code << 1) | self.bit()? as i32;
            if code <= table.maxcode[len] {
                let index = table.valptr[len] + code - table.mincode[len];
                return table
                    .values
                    .get(index as usize)
                    .copied()
                    .ok_or_else(|| corrupt("a Huffman code names no symbol"));
            }
        }
        Err(corrupt("a Huffman code is longer than 16 bits"))
    }

    /// The restart marker that ends an interval: the rest of the byte is
    /// dropped, the marker taken, the predictors and the EOB run reset.
    fn restart(&mut self) -> Result<()> {
        self.acc = 0;
        self.n = 0;
        let marker = match self.marker.take() {
            Some(m) => m,
            None => {
                let mut b = self.byte()?.ok_or_else(|| corrupt("the scan ends before a restart marker"))?;
                if b != 0xFF {
                    return Err(corrupt("a restart marker is missing"));
                }
                while b == 0xFF {
                    b = self.byte()?.ok_or_else(|| corrupt("the scan ends before a restart marker"))?;
                }
                b
            },
        };
        if !(0xD0..=0xD7).contains(&marker) {
            return Err(corrupt("a restart interval ends at a marker other than RST"));
        }
        self.preds = [0; 4];
        self.eobrun = 0;
        Ok(())
    }

    fn restart_check(&mut self) -> Result<()> {
        if self.scan.restart > 0 && self.decoded > 0 && self.decoded % self.scan.restart == 0 {
            self.restart()?;
        }
        self.decoded += 1;
        Ok(())
    }

    /// Decodes this scan's part of MCU row `row` into `planes`.
    fn decode_row(&mut self, row: u32, planes: &mut [Vec<i16>]) -> Result<()> {
        let scan = self.scan;
        let frame = self.frame;
        if let [only] = scan.comps.as_slice() {
            let comp = frame.components[only.index];
            let (bw, bh) = frame.blocks(comp);
            let stride = frame.stride(comp) as usize;
            let first = row * comp.v;
            let last = (first + comp.v).min(bh);
            for by in first..last {
                for bx in 0..bw {
                    self.restart_check()?;
                    let at = ((by - first) as usize * stride + bx as usize) * 64;
                    self.block(0, only, &mut planes[only.index][at..at + 64])?;
                }
            }
        } else {
            for mx in 0..frame.mcus_x {
                self.restart_check()?;
                for (slot, sc) in scan.comps.iter().enumerate() {
                    let comp = frame.components[sc.index];
                    let stride = frame.stride(comp) as usize;
                    for vv in 0..comp.v as usize {
                        for hh in 0..comp.h as usize {
                            let at = (vv * stride + mx as usize * comp.h as usize + hh) * 64;
                            self.block(slot, sc, &mut planes[sc.index][at..at + 64])?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn block(&mut self, slot: usize, sc: &ScanComponent, blk: &mut [i16]) -> Result<()> {
        let scan = self.scan;
        if scan.ss == 0 {
            if scan.ah == 0 {
                let table = sc.dc.as_ref().ok_or_else(|| corrupt("a DC scan has no table"))?;
                let size = u32::from(self.decode(table)?);
                if size > 15 {
                    return Err(corrupt("a DC difference is longer than 15 bits"));
                }
                // ITU-T T.81 F.2.2.1: a category of 0 is a difference of 0, with no bits.
                let diff = if size == 0 { 0 } else { extend(self.bits(size)?, size) };
                self.preds[slot] += diff;
                blk[0] = (self.preds[slot] << scan.al) as i16;
            } else if self.bit()? == 1 {
                blk[0] |= (1i32 << scan.al) as i16;
            }
        }
        if scan.se > 0 {
            let table = sc.ac.as_ref().ok_or_else(|| corrupt("an AC scan has no table"))?;
            let (ss, se, al) = (scan.ss.max(1) as usize, scan.se as usize, scan.al);
            if scan.ah == 0 {
                self.ac_first(table, blk, ss, se, al)?;
            } else {
                self.ac_refine(table, blk, ss, se, al)?;
            }
        }
        Ok(())
    }

    fn ac_first(&mut self, table: &Huffman, blk: &mut [i16], ss: usize, se: usize, al: u32) -> Result<()> {
        if self.eobrun > 0 {
            self.eobrun -= 1;
            return Ok(());
        }
        let mut k = ss;
        while k <= se {
            let rs = u32::from(self.decode(table)?);
            let (r, s) = (rs >> 4, rs & 15);
            if s == 0 {
                if r < 15 {
                    self.eobrun = (1 << r) - 1 + self.bits(r)?;
                    break;
                }
                k += 16;
                continue;
            }
            k += r as usize;
            if k > se {
                return Err(corrupt("a run of zeros ends past the scan's last coefficient"));
            }
            blk[k] = (extend(self.bits(s)?, s) << al) as i16;
            k += 1;
        }
        Ok(())
    }

    fn ac_refine(&mut self, table: &Huffman, blk: &mut [i16], ss: usize, se: usize, al: u32) -> Result<()> {
        let p1 = (1i32 << al) as i16;
        let m1 = (-1i32 << al) as i16;
        let mut k = ss;
        if self.eobrun == 0 {
            while k <= se {
                let rs = u32::from(self.decode(table)?);
                let mut r = rs >> 4;
                let s = rs & 15;
                let mut value = 0i16;
                if s != 0 {
                    if s != 1 {
                        return Err(corrupt("a refinement symbol's size is not one"));
                    }
                    value = if self.bit()? == 1 { p1 } else { m1 };
                } else if r != 15 {
                    self.eobrun = (1 << r) + self.bits(r)?;
                    break;
                }
                while k <= se {
                    if blk[k] != 0 {
                        self.refine(&mut blk[k], p1, m1)?;
                    } else {
                        if r == 0 {
                            break;
                        }
                        r -= 1;
                    }
                    k += 1;
                }
                if value != 0 {
                    if k > se {
                        return Err(corrupt("a refinement lands past the scan's last coefficient"));
                    }
                    blk[k] = value;
                }
                k += 1;
            }
        }
        if self.eobrun > 0 {
            while k <= se {
                if blk[k] != 0 {
                    self.refine(&mut blk[k], p1, m1)?;
                }
                k += 1;
            }
            self.eobrun -= 1;
        }
        Ok(())
    }

    fn refine(&mut self, coef: &mut i16, p1: i16, m1: i16) -> Result<()> {
        if self.bit()? == 1 && (*coef & p1) == 0 {
            *coef += if *coef >= 0 { p1 } else { m1 };
        }
        Ok(())
    }
}

/// The entropy writer of a baseline scan with the Annex K tables.
struct Encoder {
    dc: [[(u32, u32); 256]; 2],
    ac: [[(u32, u32); 256]; 2],
    pred: [i32; 4],
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Encoder {
    fn new() -> Result<Self> {
        let table = |bits: [u8; 16], values: Vec<u8>| Huffman::new(bits, values).map(|h| h.encoder());
        Ok(Self {
            dc: [
                table(DC_BITS[0], DC_VALUES.to_vec())?,
                table(DC_BITS[1], DC_VALUES.to_vec())?,
            ],
            ac: [
                table(AC_BITS[0], AC_VALUES[0].to_vec())?,
                table(AC_BITS[1], AC_VALUES[1].to_vec())?,
            ],
            pred: [0; 4],
            out: Vec::new(),
            acc: 0,
            n: 0,
        })
    }

    fn put(&mut self, value: u32, len: u32) {
        self.acc = (self.acc << len) | (value & ((1 << len) - 1));
        self.n += len;
        while self.n >= 8 {
            self.n -= 8;
            let b = (self.acc >> self.n) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0);
            }
        }
        self.acc &= (1 << self.n) - 1;
    }

    /// Codes one block; `slot` is the component's position in the scan and
    /// selects its table: luminance for the first, chrominance for the rest.
    fn block(&mut self, slot: usize, blk: &[i16]) -> Result<()> {
        let table = usize::from(slot > 0);
        let diff = i32::from(blk[0]) - self.pred[slot];
        self.pred[slot] = i32::from(blk[0]);
        let size = 32 - diff.unsigned_abs().leading_zeros();
        if size > 11 {
            return Err(Error::Unsupported(
                "a DC difference needs more than the baseline table's eleven bits".to_string(),
            ));
        }
        let (code, len) = symbol_code(&self.dc[table], size as u8)?;
        self.put(code, len);
        if size > 0 {
            self.put(magnitude(diff, size), size);
        }
        let mut run = 0u32;
        for &v in &blk[1..] {
            if v == 0 {
                run += 1;
                continue;
            }
            while run > 15 {
                let (code, len) = symbol_code(&self.ac[table], 0xF0)?;
                self.put(code, len);
                run -= 16;
            }
            let v = i32::from(v);
            let size = 32 - v.unsigned_abs().leading_zeros();
            if size > 10 {
                return Err(Error::Unsupported(
                    "a coefficient needs more than the baseline AC table's ten bits".to_string(),
                ));
            }
            let (code, len) = symbol_code(&self.ac[table], ((run << 4) | size) as u8)?;
            self.put(code, len);
            self.put(magnitude(v, size), size);
            run = 0;
        }
        if run > 0 {
            let (code, len) = symbol_code(&self.ac[table], 0x00)?;
            self.put(code, len);
        }
        Ok(())
    }

    /// Writes the MCU row `planes` holds, in the scan's order.
    fn row(&mut self, frame: &Frame, planes: &[Vec<i16>]) -> Result<()> {
        if let [only] = frame.components.as_slice() {
            let (bw, _) = frame.blocks(*only);
            for bx in 0..bw as usize {
                self.block(0, &planes[0][bx * 64..bx * 64 + 64])?;
            }
            return Ok(());
        }
        for mx in 0..frame.mcus_x as usize {
            for (slot, comp) in frame.components.iter().enumerate() {
                let stride = frame.stride(*comp) as usize;
                for vv in 0..comp.v as usize {
                    for hh in 0..comp.h as usize {
                        let at = (vv * stride + mx * comp.h as usize + hh) * 64;
                        self.block(slot, &planes[slot][at..at + 64])?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Pads the last byte with one bits and returns what is left to write.
    fn finish(&mut self) -> Vec<u8> {
        if self.n > 0 {
            let pad = 8 - self.n;
            self.put((1 << pad) - 1, pad);
        }
        std::mem::take(&mut self.out)
    }
}

/// The `(code, length)` of a symbol the table lists.
fn symbol_code(table: &[(u32, u32); 256], symbol: u8) -> Result<(u32, u32)> {
    let (code, len) = table[usize::from(symbol)];
    if len == 0 {
        return Err(Error::Unsupported(
            "a coefficient needs a Huffman symbol the baseline tables do not carry".to_string(),
        ));
    }
    Ok((code, len))
}

/// The bits of a coefficient of `size` bits: the value, or for a negative one
/// the value less one (ITU-T T.81 F.1.2.1.1).
fn magnitude(v: i32, size: u32) -> u32 {
    if v >= 0 {
        v as u32
    } else {
        (v + (1 << size) - 1) as u32
    }
}

/// The region to replace, in image pixels, and the fill of each pixel in it.
pub(crate) struct Cover<'a> {
    pub x0: usize,
    pub x1: usize,
    pub y0: usize,
    pub y1: usize,
    /// The decoded colour samples (0..=255) of the fill a block takes. The block's
    /// covered pixels are `x0..x1` by `y0..y1`; the caller picks among the fills
    /// that meet that rect.
    pub fill_at: &'a dyn Fn(usize, usize, usize, usize) -> Option<Vec<u32>>,
}

/// The inclusive block range a component's covered pixels meet.
fn covered_blocks(
    jpeg: &Jpeg,
    comp: Component,
    cover: &Cover,
) -> Option<(u32, u32, u32, u32)> {
    let f = &jpeg.frame;
    let span = |lo: usize, hi: usize, s: u32, smax: u32| -> Option<(u32, u32)> {
        let a = (lo * s as usize) / smax as usize;
        let b = (hi * s as usize).div_ceil(smax as usize);
        (b > a).then(|| (a as u32 / 8, (b as u32 - 1) / 8))
    };
    let (bx0, bx1) = span(cover.x0, cover.x1, comp.h, f.hmax)?;
    let (by0, by1) = span(cover.y0, cover.y1, comp.v, f.vmax)?;
    Some((bx0, bx1, by0, by1))
}

impl Jpeg {
    /// The stream's header as the copy writes it: SOI, the APP14 when there is
    /// one, the source's DQT, a baseline SOF0 with the source's sampling, the
    /// Annex K tables, one scan over every component.
    fn write_header(&self, out: &mut dyn Write) -> Result<()> {
        out.write_all(&[0xFF, 0xD8])?;
        if let Some(app) = &self.app14 {
            segment(out, 0xEE, app)?;
        }
        for q in &self.quant {
            segment(out, 0xDB, q)?;
        }
        let f = &self.frame;
        let mut sof = vec![8u8];
        sof.extend_from_slice(&(f.height as u16).to_be_bytes());
        sof.extend_from_slice(&(f.width as u16).to_be_bytes());
        sof.push(f.components.len() as u8);
        for c in &f.components {
            sof.extend_from_slice(&[c.id, ((c.h << 4) | c.v) as u8, c.tq as u8]);
        }
        segment(out, 0xC0, &sof)?;
        let tables = if f.components.len() == 1 { 1 } else { 2 };
        for t in 0..tables {
            let mut dc = vec![t as u8];
            dc.extend_from_slice(&DC_BITS[t]);
            dc.extend_from_slice(&DC_VALUES);
            segment(out, 0xC4, &dc)?;
            let mut ac = vec![0x10 | t as u8];
            ac.extend_from_slice(&AC_BITS[t]);
            ac.extend_from_slice(&AC_VALUES[t]);
            segment(out, 0xC4, &ac)?;
        }
        let mut sos = vec![f.components.len() as u8];
        for (slot, c) in f.components.iter().enumerate() {
            let t = u8::from(slot > 0);
            sos.extend_from_slice(&[c.id, (t << 4) | t]);
        }
        sos.extend_from_slice(&[0, 63, 0]);
        segment(out, 0xDA, &sos)
    }

    /// Replaces the covered blocks of MCU row `row` with flat blocks and returns
    /// nothing else: every other block keeps the coefficients it was read with.
    fn flatten(&self, row: u32, planes: &mut [Vec<i16>], cover: &Cover, covered: &[Option<(u32, u32, u32, u32)>]) -> Result<()> {
        let f = &self.frame;
        for (c, comp) in f.components.iter().enumerate() {
            let Some((bx0, bx1, by0, by1)) = covered[c] else { continue };
            let stride = f.stride(*comp) as usize;
            for vv in 0..comp.v {
                let by = row * comp.v + vv;
                if by < by0 || by > by1 {
                    continue;
                }
                for bx in bx0..=bx1 {
                    // The block's pixels in image space, clipped to the covered rect.
                    let span = |start: u32, size: u32, hmax: u32, rect: (usize, usize)| {
                        let a = (start as usize * 8 * hmax as usize) / size as usize;
                        let b = ((start as usize + 1) * 8 * hmax as usize) / size as usize;
                        (a.max(rect.0), b.min(rect.1))
                    };
                    let (x0, x1) = span(bx, comp.h, f.hmax, (cover.x0, cover.x1));
                    let (y0, y1) = span(by, comp.v, f.vmax, (cover.y0, cover.y1));
                    let samples = (cover.fill_at)(x0, x1, y0, y1).ok_or_else(|| {
                        Error::Image("a covered block meets no fill".to_string())
                    })?;
                    let stored = self.stored(&samples)?;
                    let q0 = self.dc_quant[comp.tq];
                    let value = (8.0 * (f64::from(stored[c]) - 128.0) / f64::from(q0)).round();
                    let at = (vv as usize * stride + bx as usize) * 64;
                    let blk = &mut planes[c][at..at + 64];
                    blk.fill(0);
                    blk[0] = value.clamp(-2048.0, 2047.0) as i16;
                }
            }
        }
        Ok(())
    }

    /// Writes the whole copy: the header, each MCU row read, flattened where it
    /// is covered and coded, then EOI. A `Cover` with no region writes the
    /// stream unchanged, which the dry run uses to prove every row codes.
    pub(crate) fn write(&self, body: &StreamBody, cover: &Cover, cancel: &CancelCheck, out: &mut dyn Write) -> Result<()> {
        self.write_header(out)?;
        let covered: Vec<Option<(u32, u32, u32, u32)>> = self
            .frame
            .components
            .iter()
            .map(|c| covered_blocks(self, *c, cover))
            .collect();
        let mut rows = Rows::open(body, self)?;
        let mut planes = self.frame.planes();
        let mut enc = Encoder::new()?;
        for row in 0..self.frame.mcus_y {
            cancel.check()?;
            rows.next_row(row, &mut planes)?;
            self.flatten(row, &mut planes, cover, &covered)?;
            enc.row(&self.frame, &planes)?;
            out.write_all(&std::mem::take(&mut enc.out))?;
        }
        out.write_all(&enc.finish())?;
        out.write_all(&[0xFF, EOI])?;
        Ok(())
    }

    /// Whether every block of the stream codes in the baseline tables. The dry
    /// run calls it, so a copy that would fail halfway is refused before anything
    /// is recorded.
    pub(crate) fn check_codes(&self, body: &StreamBody, cancel: &CancelCheck) -> Result<()> {
        let none = |_: usize, _: usize, _: usize, _: usize| -> Option<Vec<u32>> { None };
        let cover = Cover {
            x0: 0,
            x1: 0,
            y0: 0,
            y1: 0,
            fill_at: &none,
        };
        self.write(body, &cover, cancel, &mut io::sink())
    }
}

fn segment(out: &mut dyn Write, marker: u8, payload: &[u8]) -> Result<()> {
    let len = u16::try_from(payload.len() + 2)
        .map_err(|_| Error::Image("a JPEG segment is longer than 64 KiB".to_string()))?;
    out.write_all(&[0xFF, marker])?;
    out.write_all(&len.to_be_bytes())?;
    out.write_all(payload)?;
    Ok(())
}

/// The lockstep reader of a whole frame: one decoder per scan, all advanced one
/// MCU row per call.
struct Rows<'a> {
    jpeg: &'a Jpeg,
    decoders: Vec<ScanDecoder<'a>>,
}

impl<'a> Rows<'a> {
    fn open(body: &'a StreamBody, jpeg: &'a Jpeg) -> Result<Self> {
        let mut decoders = Vec::with_capacity(jpeg.scans.len());
        for scan in &jpeg.scans {
            let mut raw = body.raw_reader()?;
            io::copy(&mut raw.by_ref().take(scan.offset), &mut io::sink()).map_err(from_io)?;
            decoders.push(ScanDecoder {
                scan,
                frame: &jpeg.frame,
                input: BufReader::new(raw),
                acc: 0,
                n: 0,
                marker: None,
                eobrun: 0,
                preds: [0; 4],
                decoded: 0,
            });
        }
        Ok(Self { jpeg, decoders })
    }

    /// Fills `planes` with MCU row `row` of every component. Rows are asked in
    /// order, once each.
    fn next_row(&mut self, row: u32, planes: &mut [Vec<i16>]) -> Result<()> {
        debug_assert!(row < self.jpeg.frame.mcus_y);
        for plane in planes.iter_mut() {
            plane.fill(0);
        }
        for d in &mut self.decoders {
            d.decode_row(row, planes)?;
        }
        Ok(())
    }
}
