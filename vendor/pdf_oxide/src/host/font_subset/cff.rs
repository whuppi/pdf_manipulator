//! CFF subsetting that keeps every glyph id.
//!
//! A charstring outside the kept set becomes the minimal empty one, a
//! single `endchar`. Everything else, the charset, the subroutines and the
//! private dictionaries included, is copied as it is. The `CharStrings`
//! INDEX shrinks, so the offsets that name what follows it are written
//! again, in the fixed five-byte form that keeps a dictionary's size
//! independent of the offsets it holds.

use std::collections::BTreeSet;

const ENDCHAR: u8 = 14;

const CHARSET: u16 = 15;
const ENCODING: u16 = 16;
const CHAR_STRINGS: u16 = 17;
const PRIVATE: u16 = 18;
const SUBRS: u16 = 19;
const FD_ARRAY: u16 = 0x0c24;
const FD_SELECT: u16 = 0x0c25;

/// A parsed INDEX: the byte range of each item and where the INDEX ends.
struct Index {
    items: Vec<(usize, usize)>,
    end: usize,
}

fn index(data: &[u8], at: usize) -> Option<Index> {
    let count = u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?) as usize;
    if count == 0 {
        return Some(Index { items: Vec::new(), end: at + 2 });
    }
    let size = *data.get(at + 2)? as usize;
    if !(1..=4).contains(&size) {
        return None;
    }
    let offsets_at = at + 3;
    let base = offsets_at + (count + 1) * size - 1;
    let offset = |i: usize| -> Option<usize> {
        let bytes = data.get(offsets_at + i * size..offsets_at + (i + 1) * size)?;
        Some(bytes.iter().fold(0usize, |acc, &b| (acc << 8) | b as usize))
    };
    let mut items = Vec::with_capacity(count);
    let mut previous = offset(0)?;
    if previous != 1 {
        return None;
    }
    for i in 1..=count {
        let next = offset(i)?;
        if next < previous || base + next > data.len() {
            return None;
        }
        items.push((base + previous, base + next));
        previous = next;
    }
    Some(Index { items, end: base + previous })
}

fn write_index(items: &[&[u8]]) -> Vec<u8> {
    let mut out = (items.len() as u16).to_be_bytes().to_vec();
    if items.is_empty() {
        return out;
    }
    let total: usize = items.iter().map(|i| i.len()).sum::<usize>() + 1;
    let size = match total {
        0..=0xff => 1,
        0x100..=0xffff => 2,
        0x1_0000..=0xff_ffff => 3,
        _ => 4,
    };
    out.push(size as u8);
    let mut offset = 1usize;
    for step in std::iter::once(0).chain(items.iter().map(|i| i.len())) {
        offset += step;
        out.extend_from_slice(&(offset as u32).to_be_bytes()[4 - size..]);
    }
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

/// One operand of a DICT: its bytes as written, and its value when it is
/// an integer.
#[derive(Clone)]
struct Operand {
    raw: Vec<u8>,
    value: Option<i32>,
}

#[derive(Clone)]
struct Entry {
    op: u16,
    operands: Vec<Operand>,
}

fn dict(data: &[u8]) -> Option<Vec<Entry>> {
    let mut entries = Vec::new();
    let mut operands = Vec::new();
    let mut at = 0;
    while at < data.len() {
        let b0 = data[at];
        let (len, value) = match b0 {
            0..=21 => {
                let (op, len) = if b0 == 12 { (0x0c00 | *data.get(at + 1)? as u16, 2) } else { (b0 as u16, 1) };
                entries.push(Entry { op, operands: std::mem::take(&mut operands) });
                at += len;
                continue;
            },
            28 => (3, Some(i16::from_be_bytes(data.get(at + 1..at + 3)?.try_into().ok()?) as i32)),
            29 => (5, Some(i32::from_be_bytes(data.get(at + 1..at + 5)?.try_into().ok()?))),
            30 => {
                let mut len = 1;
                loop {
                    let byte = *data.get(at + len)?;
                    len += 1;
                    if byte & 0x0f == 0x0f || byte >> 4 == 0x0f {
                        break;
                    }
                }
                (len, None)
            },
            32..=246 => (1, Some(b0 as i32 - 139)),
            247..=250 => (2, Some((b0 as i32 - 247) * 256 + *data.get(at + 1)? as i32 + 108)),
            251..=254 => (2, Some(-(b0 as i32 - 251) * 256 - *data.get(at + 1)? as i32 - 108)),
            _ => return None,
        };
        operands.push(Operand { raw: data.get(at..at + len)?.to_vec(), value });
        at += len;
    }
    operands.is_empty().then_some(entries)
}

fn fixed(value: i32) -> Operand {
    let mut raw = vec![29];
    raw.extend_from_slice(&value.to_be_bytes());
    Operand { raw, value: Some(value) }
}

fn write_dict(entries: &[Entry]) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in entries {
        for operand in &entry.operands {
            out.extend_from_slice(&operand.raw);
        }
        if entry.op >= 0x0c00 {
            out.push(12);
        }
        out.push(entry.op as u8);
    }
    out
}

/// The offset operand of `entry`, if it names a place in the file.
fn pointer(entry: &Entry) -> Option<usize> {
    let operand = match entry.op {
        CHARSET | ENCODING => {
            let v = entry.operands.first()?.value?;
            // 0, 1 and 2 name predefined sets
            if v <= if entry.op == CHARSET { 2 } else { 1 } {
                return None;
            }
            v
        },
        CHAR_STRINGS | FD_ARRAY | FD_SELECT => entry.operands.first()?.value?,
        PRIVATE => entry.operands.get(1)?.value?,
        _ => return None,
    };
    usize::try_from(operand).ok()
}

/// Where the bytes of the old file go in the new one: the head is rebuilt
/// and the tail is copied around the blocks that change length.
struct Layout {
    tail_start: usize,
    new_tail_start: usize,
    /// `(old start, old end, new length)`, by start.
    blocks: Vec<(usize, usize, usize)>,
}

impl Layout {
    fn map(&self, old: usize) -> Option<usize> {
        if old < self.tail_start {
            return None;
        }
        let mut delta = 0isize;
        for &(start, end, new_len) in &self.blocks {
            if old > start && old < end {
                return None;
            }
            if old >= end {
                delta += new_len as isize - (end - start) as isize;
            }
        }
        usize::try_from((old - self.tail_start + self.new_tail_start) as isize + delta).ok()
    }
}

fn rewrite_pointers(entries: &mut [Entry], layout: &Layout) -> Option<()> {
    for entry in entries {
        if let Some(old) = pointer(entry) {
            let new = i32::try_from(layout.map(old)?).ok()?;
            let slot = if entry.op == PRIVATE { 1 } else { 0 };
            entry.operands[slot] = fixed(new);
        }
    }
    Some(())
}

/// `data` (a CFF font program) with the charstrings outside `keep` (and
/// glyph 0) replaced by `endchar`, or `None` when `data` is not a font this
/// can rewrite.
pub(crate) fn subset(data: &[u8], keep: &BTreeSet<u16>) -> Option<Vec<u8>> {
    if *data.first()? != 1 {
        return None;
    }
    let header = *data.get(2)? as usize;
    let names = index(data, header)?;
    let tops = index(data, names.end)?;
    let strings = index(data, tops.end)?;
    let subrs = index(data, strings.end)?;
    let tail_start = subrs.end;
    if tops.items.len() != 1 {
        return None;
    }
    let (top_start, top_end) = tops.items[0];
    let mut top = dict(&data[top_start..top_end])?;

    let charstrings_at = top.iter().find(|e| e.op == CHAR_STRINGS).and_then(pointer)?;
    let charstrings = index(data, charstrings_at)?;
    if charstrings_at < tail_start {
        return None;
    }
    let new_strings: Vec<&[u8]> = charstrings
        .items
        .iter()
        .enumerate()
        .map(|(gid, &(start, end))| {
            if gid == 0 || keep.contains(&(gid as u16)) {
                &data[start..end]
            } else {
                &[ENDCHAR][..]
            }
        })
        .collect();
    let new_charstrings = write_index(&new_strings);

    let font_dicts = match top.iter().find(|e| e.op == FD_ARRAY).and_then(pointer) {
        Some(at) => Some((at, index(data, at)?)),
        None => None,
    };
    let fd_entries: Option<Vec<Vec<Entry>>> = match &font_dicts {
        Some((_, array)) => {
            Some(array.items.iter().map(|&(s, e)| dict(&data[s..e])).collect::<Option<_>>()?)
        },
        None => None,
    };

    // The size of the head does not depend on the offsets in it.
    let rebuilt_top = |entries: &[Entry]| -> Vec<u8> {
        let mut probe = entries.to_vec();
        for entry in &mut probe {
            if pointer(entry).is_some() {
                let slot = if entry.op == PRIVATE { 1 } else { 0 };
                entry.operands[slot] = fixed(0);
            }
        }
        write_dict(&probe)
    };
    let top_len = write_index(&[&rebuilt_top(&top)]).len();
    let new_tail_start = names.end + top_len + (tail_start - tops.end);

    let mut blocks = vec![(charstrings_at, charstrings.end, new_charstrings.len())];
    if let (Some((at, array)), Some(fds)) = (&font_dicts, &fd_entries) {
        let probes: Vec<Vec<u8>> = fds.iter().map(|e| rebuilt_top(e)).collect();
        let items: Vec<&[u8]> = probes.iter().map(|p| p.as_slice()).collect();
        blocks.push((*at, array.end, write_index(&items).len()));
    }
    blocks.sort_unstable();
    if blocks.windows(2).any(|w| w[0].1 > w[1].0) {
        return None;
    }
    let layout = Layout { tail_start, new_tail_start, blocks };

    // A local subroutine INDEX is addressed from its private dictionary:
    // the two have to move by the same amount.
    let mut privates: Vec<(usize, usize)> = Vec::new();
    for entries in std::iter::once(&top).chain(fd_entries.iter().flatten()) {
        if let Some(entry) = entries.iter().find(|e| e.op == PRIVATE) {
            let size = usize::try_from(entry.operands.first()?.value?).ok()?;
            privates.push((size, pointer(entry)?));
        }
    }
    for (size, at) in privates {
        let body = dict(data.get(at..at.checked_add(size)?)?)?;
        if let Some(rel) = body.iter().find(|e| e.op == SUBRS) {
            let rel = usize::try_from(rel.operands.first()?.value?).ok()?;
            let (private, local) = (layout.map(at)?, layout.map(at + rel)?);
            if local.checked_sub(private)? != rel {
                return None;
            }
        }
    }

    rewrite_pointers(&mut top, &layout)?;
    let mut new_fds = Vec::new();
    if let Some(fds) = fd_entries {
        for mut entries in fds {
            rewrite_pointers(&mut entries, &layout)?;
            new_fds.push(write_dict(&entries));
        }
    }
    let new_top = write_index(&[&write_dict(&top)]);
    let new_array = {
        let items: Vec<&[u8]> = new_fds.iter().map(|d| d.as_slice()).collect();
        write_index(&items)
    };

    let mut out = data[..names.end].to_vec();
    out.extend_from_slice(&new_top);
    out.extend_from_slice(&data[tops.end..tail_start]);
    if out.len() != new_tail_start {
        return None;
    }
    let mut cursor = tail_start;
    for &(start, end, _) in &layout.blocks {
        out.extend_from_slice(data.get(cursor..start)?);
        if start == charstrings_at {
            out.extend_from_slice(&new_charstrings);
        } else {
            out.extend_from_slice(&new_array);
        }
        cursor = end;
    }
    out.extend_from_slice(data.get(cursor..)?);
    Some(out)
}
