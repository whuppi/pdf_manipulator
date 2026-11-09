//! A table of groups found by hash and told apart by the caller, so that a
//! page's text runs are indexed without a heap object for each of them.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! The span passes that look for a run drawn twice (the stroke and fill
//! pass, the positional filter) used to key a `HashMap<String, ...>` by each
//! run's text, one owned string and one owned map or list per distinct text:
//! hundreds of bytes for each run of a page. A [`GroupTable`] holds one `u32`
//! slot (two once the table has grown) and one `u64` hash for each group, and nothing else: the group's
//! own data lives in the caller's flat vectors, and the caller decides, by
//! `same`, whether a group with an equal hash is the one it wants. Two
//! different keys with one hash are two groups, never one: the answer is
//! exactly the answer of a map keyed by the key itself.

const EMPTY: u32 = u32::MAX;

/// Groups numbered 0, 1, 2, ... in the order they are inserted.
#[derive(Default)]
pub(crate) struct GroupTable {
    slots: Vec<u32>,
    hashes: Vec<u64>,
}

impl GroupTable {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The group whose hash is `hash` and for which `same` answers true.
    pub(crate) fn find(&self, hash: u64, mut same: impl FnMut(u32) -> bool) -> Option<u32> {
        if self.slots.is_empty() {
            return None;
        }
        let mask = self.slots.len() - 1;
        let mut at = hash as usize & mask;
        loop {
            let group = self.slots[at];
            if group == EMPTY {
                return None;
            }
            if self.hashes[group as usize] == hash && same(group) {
                return Some(group);
            }
            at = (at + 1) & mask;
        }
    }

    /// Adds a new group with `hash` and returns its number.
    pub(crate) fn insert(&mut self, hash: u64) -> u32 {
        if (self.hashes.len() + 1) * 10 > self.slots.len() * 7 {
            self.grow();
        }
        let group = self.hashes.len() as u32;
        self.hashes.push(hash);
        self.place(group, hash);
        group
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.hashes.len()
    }

    fn place(&mut self, group: u32, hash: u64) {
        let mask = self.slots.len() - 1;
        let mut at = hash as usize & mask;
        while self.slots[at] != EMPTY {
            at = (at + 1) & mask;
        }
        self.slots[at] = group;
    }

    fn grow(&mut self) {
        let size = (self.slots.len() * 2).max(16);
        self.slots = vec![EMPTY; size];
        for group in 0..self.hashes.len() {
            let hash = self.hashes[group];
            self.place(group as u32, hash);
        }
    }
}

/// FNV-1a over the bytes the iterator yields, so a hash can be taken of a
/// text as another text (its ASCII lowercase) without making that text.
pub(crate) fn hash_bytes(bytes: impl Iterator<Item = u8>, seed: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325 ^ seed;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    // Mixed once more so the low bits, which pick the slot, depend on all of it.
    hash ^= hash >> 32;
    hash = hash.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    hash ^ (hash >> 29)
}

#[cfg(test)]
mod tests {
    use super::{hash_bytes, GroupTable};

    #[test]
    fn keys_with_one_hash_are_two_groups() {
        let keys = ["a", "b", "c"];
        let mut table = GroupTable::new();
        let mut made: Vec<&str> = Vec::new();
        for key in keys {
            // All three keys share one hash: only `same` tells them apart.
            let found = table.find(7, |g| made[g as usize] == key);
            assert!(found.is_none());
            table.insert(7);
            made.push(key);
        }
        for (i, key) in keys.iter().enumerate() {
            assert_eq!(table.find(7, |g| made[g as usize] == *key), Some(i as u32));
        }
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn a_grown_table_still_finds_every_group() {
        let mut table = GroupTable::new();
        let hashes: Vec<u64> = (0..10_000u32).map(|i| hash_bytes(i.to_le_bytes().into_iter(), 1)).collect();
        for h in &hashes {
            table.insert(*h);
        }
        for (i, h) in hashes.iter().enumerate() {
            assert_eq!(table.find(*h, |g| g as usize == i), Some(i as u32));
        }
    }
}
