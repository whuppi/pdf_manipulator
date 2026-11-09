//! LZWDecode's `/EarlyChange` switch.

use crate::decoders::{LzwDecoder, StreamDecoder};

/// No Dart battery sees it: the page reader recovers the bytes decoded before a bad code, so a corrupt stream reads as the text it held up to that code, and only the decoder's own `Err` says it was refused.
#[test]
fn a_corrupt_stream_is_refused() {
    // 9-bit codes: 'A' (65), then 300, a code far past the next free one.
    let bits: u32 = (65 << 9) | 300;
    let bytes = [(bits >> 10) as u8, (bits >> 2) as u8, ((bits & 3) << 6) as u8];
    assert!(LzwDecoder.decode(&bytes).is_err());
}
