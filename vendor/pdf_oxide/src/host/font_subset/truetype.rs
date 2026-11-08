//! TrueType (`glyf`/`loca`) subsetting that keeps every glyph id.
//!
//! A glyph outside the kept set gets a zero-length `loca` entry; every
//! table stays in the file, so `cmap`, `hmtx`, `post` and the rest still
//! answer for every id. The composite glyphs of a kept glyph keep their
//! components.

use std::collections::BTreeSet;

fn be16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

struct Table {
    tag: [u8; 4],
    offset: usize,
    length: usize,
}

fn tables(data: &[u8]) -> Option<Vec<Table>> {
    if !matches!(data.get(..4)?, [0, 1, 0, 0] | b"true") {
        return None;
    }
    let count = be16(data, 4)? as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let at = 12 + 16 * i;
        let tag: [u8; 4] = data.get(at..at + 4)?.try_into().ok()?;
        let offset = be32(data, at + 8)? as usize;
        let length = be32(data, at + 12)? as usize;
        if offset.checked_add(length)? > data.len() {
            return None;
        }
        out.push(Table { tag, offset, length });
    }
    Some(out)
}

fn checksum(bytes: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in bytes.chunks(4) {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(word));
    }
    sum
}

/// The ids a composite glyph is made of.
fn components(glyph: &[u8]) -> Option<Vec<u16>> {
    const WORDS: u16 = 0x1;
    const SCALE: u16 = 0x8;
    const MORE: u16 = 0x20;
    const XY_SCALE: u16 = 0x40;
    const TWO_BY_TWO: u16 = 0x80;
    if glyph.len() < 10 || (be16(glyph, 0)? as i16) >= 0 {
        return Some(Vec::new());
    }
    let mut at = 10;
    let mut out = Vec::new();
    loop {
        let flags = be16(glyph, at)?;
        out.push(be16(glyph, at + 2)?);
        at += 4 + if flags & WORDS != 0 { 4 } else { 2 };
        at += if flags & SCALE != 0 {
            2
        } else if flags & XY_SCALE != 0 {
            4
        } else if flags & TWO_BY_TWO != 0 {
            8
        } else {
            0
        };
        if flags & MORE == 0 {
            return Some(out);
        }
    }
}

/// `data` with the glyphs outside `keep` (and the components of those in
/// it, and glyph 0) emptied, or `None` when `data` is not a plain
/// `glyf`-based sfnt this can rewrite or has a `GSUB` table.
pub(crate) fn subset(data: &[u8], keep: &BTreeSet<u16>) -> Option<Vec<u8>> {
    let all = tables(data)?;
    let find = |tag: &[u8; 4]| all.iter().find(|t| &t.tag == tag);
    if find(b"GSUB").is_some() {
        return None;
    }
    let (head, maxp, loca, glyf) = (find(b"head")?, find(b"maxp")?, find(b"loca")?, find(b"glyf")?);
    let glyph_count = be16(data, maxp.offset + 4)? as usize;
    let long = match be16(data, head.offset + 50)? {
        0 => false,
        1 => true,
        _ => return None,
    };
    let entry = if long { 4 } else { 2 };
    if loca.length < (glyph_count + 1) * entry {
        return None;
    }
    let glyf_data = &data[glyf.offset..glyf.offset + glyf.length];
    let offset_of = |gid: usize| -> Option<usize> {
        let at = loca.offset + gid * entry;
        Some(if long { be32(data, at)? as usize } else { be16(data, at)? as usize * 2 })
    };
    let mut ranges = Vec::with_capacity(glyph_count);
    let mut previous = 0usize;
    for gid in 0..glyph_count {
        let (start, end) = (offset_of(gid)?, offset_of(gid + 1)?);
        if start < previous || end < start || end > glyf_data.len() {
            return None;
        }
        previous = end;
        ranges.push((start, end));
    }

    let mut used: BTreeSet<u16> = BTreeSet::new();
    let mut stack: Vec<u16> = keep.iter().copied().chain([0]).collect();
    while let Some(gid) = stack.pop() {
        if gid as usize >= glyph_count {
            // an id past the font is what the renderer paints as nothing
            continue;
        }
        if !used.insert(gid) {
            continue;
        }
        let (start, end) = ranges[gid as usize];
        for part in components(&glyf_data[start..end])? {
            if part as usize >= glyph_count {
                return None;
            }
            stack.push(part);
        }
    }

    let mut new_glyf = Vec::new();
    let mut new_offsets = Vec::with_capacity(glyph_count + 1);
    for (gid, &(start, end)) in ranges.iter().enumerate() {
        new_offsets.push(new_glyf.len());
        if used.contains(&(gid as u16)) {
            new_glyf.extend_from_slice(&glyf_data[start..end]);
            if new_glyf.len() % 2 == 1 {
                new_glyf.push(0);
            }
        }
    }
    new_offsets.push(new_glyf.len());
    let mut new_loca = Vec::with_capacity((glyph_count + 1) * entry);
    for &offset in &new_offsets {
        if long {
            new_loca.extend_from_slice(&(offset as u32).to_be_bytes());
        } else {
            new_loca.extend_from_slice(&((offset / 2) as u16).to_be_bytes());
        }
    }

    // Tables keep the order they have in the file; the directory keeps its own.
    let mut order: Vec<usize> = (0..all.len()).collect();
    order.sort_by_key(|&i| all[i].offset);
    let header = 12 + 16 * all.len();
    let mut out = data[..12].to_vec();
    out.resize(header, 0);
    let mut placed = vec![(0usize, 0usize, 0u32); all.len()];
    for i in order {
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let table = &all[i];
        let mut bytes = if &table.tag == b"glyf" {
            new_glyf.clone()
        } else if &table.tag == b"loca" {
            new_loca.clone()
        } else {
            data[table.offset..table.offset + table.length].to_vec()
        };
        if &table.tag == b"head" && bytes.len() >= 12 {
            bytes[8..12].fill(0);
        }
        placed[i] = (out.len(), bytes.len(), checksum(&bytes));
        out.extend_from_slice(&bytes);
    }
    while out.len() % 4 != 0 {
        out.push(0);
    }
    for (i, table) in all.iter().enumerate() {
        let at = 12 + 16 * i;
        let (offset, length, sum) = placed[i];
        out[at..at + 4].copy_from_slice(&table.tag);
        out[at + 4..at + 8].copy_from_slice(&sum.to_be_bytes());
        out[at + 8..at + 12].copy_from_slice(&(offset as u32).to_be_bytes());
        out[at + 12..at + 16].copy_from_slice(&(length as u32).to_be_bytes());
    }
    if let Some(i) = all.iter().position(|t| &t.tag == b"head") {
        let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        let at = placed[i].0 + 8;
        out[at..at + 4].copy_from_slice(&adjustment.to_be_bytes());
    }
    Some(out)
}
