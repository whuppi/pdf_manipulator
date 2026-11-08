//! The whole-buffer decoders as they were before the filter readers, kept
//! verbatim (minus their size caps) as the reference the readers are proved
//! equal to. Test code only.

#![allow(clippy::all)]

use crate::error::{Error, Result};


pub(super) fn flate(input: &[u8]) -> Result<Vec<u8>> {
    use crate::decoders::flate::{is_truncation, looks_like_real_stream};
    use flate2::read::{DeflateDecoder, ZlibDecoder};
    use std::io::Read;
        // A zero-length stream decodes to zero bytes. Every deflate
        // implementation this crate has used said so by returning success;
        // rejecting incomplete streams at EOF turned the empty stream into an
        // error, and the partial-recovery path below cannot rescue it because
        // it requires a non-empty buffer to inspect. Real files carry these:
        // a zero-area transparency group is written as an empty Form XObject,
        // and failing one aborts the parent content stream part-way through,
        // dropping every mark that would have been painted after it.
        if input.is_empty() {
            return Ok(Vec::new());
        }

        let mut decoder = ZlibDecoder::new(input).take(u64::MAX);
        let mut output = Vec::new();

        // Try to read all data with standard zlib
        match decoder.read_to_end(&mut output) {
            Ok(_) => {
                Ok(output)
            },
            Err(e) => {
                // Partial recovery: return only if output *looks like* a
                // plausible stream. The pre-fix behaviour accepted any
                // non-empty buffer, which let strategies 2 and 3 return
                // misaligned-deflate garbage (`P\xffj!}` × 16 on
                // nougat_026.pdf pages 1/2/5) that the text extractor then
                // emitted as zero bytes of output.
                // `looks_like_real_stream` is a *content-stream* heuristic — it
                // wants BT/Tj markers or mostly-printable ASCII. A truncated
                // xref or object stream is binary, so it fails that test and the
                // whole file then falls back to xref reconstruction, losing
                // objects. Gate on the failure mode instead: a truncated stream's
                // prefix is valid, a corrupt one's is not.
                if !output.is_empty() && (is_truncation(&e) || looks_like_real_stream(&output)) {
                    log::warn!(
                        "FlateDecode partial recovery: extracted {} bytes before corruption: {}",
                        output.len(),
                        e
                    );
                    return Ok(output);
                }

                // Strategy 2: Try raw deflate (no zlib wrapper)
                // Some PDFs have corrupt zlib headers but valid deflate data
                log::info!("Zlib decode failed, trying raw deflate");
                output.clear();
                let mut deflate_decoder =
                    DeflateDecoder::new(input).take(u64::MAX);

                match deflate_decoder.read_to_end(&mut output) {
                    Ok(_) => {
                        log::info!("Raw deflate recovery succeeded: {} bytes", output.len());
                        Ok(output)
                    },
                    Err(deflate_err) => {
                        if !output.is_empty()
                            && (is_truncation(&deflate_err) || looks_like_real_stream(&output))
                        {
                            log::warn!(
                                "Raw deflate partial recovery: extracted {} bytes before error",
                                output.len()
                            );
                            return Ok(output);
                        }

                        // Strategy 3: Try skipping zlib header (2 bytes) and reading deflate
                        if input.len() > 2 {
                            log::info!(
                                "Trying deflate after skipping potential corrupt zlib header"
                            );
                            output.clear();
                            let mut deflate_decoder =
                                DeflateDecoder::new(&input[2..]).take(u64::MAX);

                            match deflate_decoder.read_to_end(&mut output) {
                                Ok(_) => {
                                    log::info!(
                                        "Deflate with header skip succeeded: {} bytes",
                                        output.len()
                                    );
                                    return Ok(output);
                                },
                                Err(_) => {
                                    if !output.is_empty() && looks_like_real_stream(&output) {
                                        log::warn!(
                                            "Deflate with header skip partial recovery: {} bytes",
                                            output.len()
                                        );
                                        return Ok(output);
                                    }
                                },
                            }
                        }

                        // Strategy 4: Try fixing corrupt zlib header byte
                        // If first byte has invalid compression method, replace with 0x78 (standard deflate)
                        if input.len() >= 2 {
                            let first_byte = input[0];
                            let compression_method = first_byte & 0x0F;
                            if compression_method != 8 {
                                log::info!(
                                    "Detected invalid compression method {} in header byte 0x{:02x}, trying with corrected header",
                                    compression_method,
                                    first_byte
                                );
                                // Create new buffer with corrected header
                                let mut corrected = input.to_vec();
                                // Replace CM bits (0-3) with 8 (deflate), keep CINFO bits (4-7)
                                corrected[0] = (first_byte & 0xF0) | 0x08;

                                output.clear();
                                let mut decoder = ZlibDecoder::new(&corrected[..])
                                    .take(u64::MAX);
                                match decoder.read_to_end(&mut output) {
                                    Ok(_) if !output.is_empty() => {
                                        log::info!(
                                            "Header correction recovery succeeded: {} bytes",
                                            output.len()
                                        );
                                        return Ok(output);
                                    },
                                    Err(_)
                                        if !output.is_empty()
                                            && looks_like_real_stream(&output) =>
                                    {
                                        log::warn!(
                                            "Header correction partial recovery: {} bytes",
                                            output.len()
                                        );
                                        return Ok(output);
                                    },
                                    _ => {
                                        log::info!("Header correction failed");
                                    },
                                }
                            }
                        }

                        // Strategy 5: Brute-force scan for valid deflate data
                        // Try starting deflate decompression from offsets 0-20
                        // BUT validate the output contains valid PDF operators
                        log::info!("Trying brute-force scan for valid deflate data");
                        let max_offset = std::cmp::min(20, input.len());
                        for offset in 0..max_offset {
                            if offset == 0 || offset == 2 {
                                continue; // Already tried these
                            }

                            output.clear();
                            let mut deflate_decoder = DeflateDecoder::new(&input[offset..])
                                .take(u64::MAX);

                            match deflate_decoder.read_to_end(&mut output) {
                                Ok(_) if !output.is_empty() => {
                                    // Validate output quality - check for PDF operators
                                    let decoded_str = String::from_utf8_lossy(&output);
                                    let has_pdf_operators = decoded_str.contains("BT")
                                        || decoded_str.contains("ET")
                                        || decoded_str.contains("Tj")
                                        || decoded_str.contains("TJ")
                                        || decoded_str.contains("Tm")
                                        || decoded_str.contains("Td");

                                    if has_pdf_operators {
                                        log::info!(
                                            "Brute-force deflate recovery succeeded at offset {}: {} bytes (validated PDF content)",
                                            offset,
                                            output.len()
                                        );
                                        return Ok(output);
                                    } else {
                                        log::info!(
                                            "Brute-force at offset {} produced {} bytes but no valid PDF operators - trying next offset",
                                            offset,
                                            output.len()
                                        );
                                        continue;
                                    }
                                },
                                Err(_) if !output.is_empty() => {
                                    // Validate partial recovery too
                                    let decoded_str = String::from_utf8_lossy(&output);
                                    let has_pdf_operators = decoded_str.contains("BT")
                                        || decoded_str.contains("ET")
                                        || decoded_str.contains("Tj")
                                        || decoded_str.contains("TJ")
                                        || decoded_str.contains("Tm")
                                        || decoded_str.contains("Td");

                                    if has_pdf_operators {
                                        log::warn!(
                                            "Brute-force partial recovery at offset {}: {} bytes (validated PDF content)",
                                            offset,
                                            output.len()
                                        );
                                        return Ok(output);
                                    } else {
                                        log::info!(
                                            "Partial recovery at offset {} but no valid PDF operators - trying next offset",
                                            offset
                                        );
                                        continue;
                                    }
                                },
                                _ => continue,
                            }
                        }

                        // SPEC COMPLIANCE FIX: Removed strategies 8-9 that violated PDF spec
                        //
                        // Previous strategies 8-9 would return raw uncompressed data for streams
                        // labeled as /FlateDecode. This violates PDF Spec ISO 32000-1:2008,
                        // Section 7.3.8.2 which states that if a stream has /Filter /FlateDecode,
                        // it MUST be compressed with the FlateDecode algorithm.
                        //
                        // Returning raw data creates security risks:
                        // 1. Malicious PDFs could bypass compression validation
                        // 2. Type confusion attacks (treating compressed data as raw)
                        // 3. Inconsistent behavior across PDF processors
                        //
                        // Correct behavior: If all decompression strategies fail, return an error.
                        // The stream is either corrupted or malicious, and should not be processed.

                        log::error!(
                            "All FlateDecode recovery strategies failed. Zlib: {}, Deflate: {}",
                            e,
                            deflate_err
                        );
                        log::error!(
                            "Stream labeled as FlateDecode but cannot be decompressed - this violates PDF spec"
                        );

                        Err(Error::Decode(format!(
                            "FlateDecode decompression failed: stream is labeled as compressed but all decompression attempts failed. \
                            This violates PDF Spec ISO 32000-1:2008, Section 7.3.8.2. \
                            Zlib error: {}, Deflate error: {}. Compressed size: {} bytes.",
                            e,
                            deflate_err,
                            input.len()
                        )))
                    },
                }
            },
        }
    }

pub(super) fn ascii85(input: &[u8]) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        let mut acc: u32 = 0;
        let mut count = 0;

        for &byte in input {
            match byte {
                b'~' => break, // End marker '~>'
                b'z' => {
                    // Special case: 'z' represents 4 zero bytes
                    if count != 0 {
                        return Err(Error::Decode(
                            "ASCII85Decode: 'z' must not appear in the middle of a group"
                                .to_string(),
                        ));
                    }
                    output.extend_from_slice(&[0, 0, 0, 0]);
                },
                b'!'..=b'u' => {
                    // Valid ASCII85 character (33-117)
                    acc = acc
                        .checked_mul(85)
                        .and_then(|v| v.checked_add((byte - b'!') as u32))
                        .ok_or_else(|| {
                            Error::Decode("ASCII85Decode: overflow in decoding".to_string())
                        })?;
                    count += 1;

                    if count == 5 {
                        // Got a complete group - output 4 bytes
                        output.extend_from_slice(&acc.to_be_bytes());
                        acc = 0;
                        count = 0;
                    }
                },
                _ if byte.is_ascii_whitespace() => {}, // Skip whitespace
                _ => {
                    return Err(Error::Decode(format!(
                        "ASCII85Decode: invalid character '{}'",
                        byte as char
                    )));
                },
            }
        }

        // Handle trailing bytes (incomplete group)
        if count > 0 {
            if count == 1 {
                return Err(Error::Decode(
                    "ASCII85Decode: incomplete group (need at least 2 characters)".to_string(),
                ));
            }

            // Pad with 'u' (84 = 117 - 33) to complete the group
            for _ in count..5 {
                acc = acc
                    .checked_mul(85)
                    .and_then(|v| v.checked_add(84))
                    .ok_or_else(|| {
                        Error::Decode("ASCII85Decode: overflow in padding".to_string())
                    })?;
            }

            // Output count-1 bytes (first N-1 bytes are valid)
            let bytes = acc.to_be_bytes();
            output.extend_from_slice(&bytes[..count - 1]);
        }

        Ok(output)
    }

pub(super) fn ascii_hex(input: &[u8]) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        let mut chars = input
            .iter()
            .filter(|&&c| !c.is_ascii_whitespace() && c != b'>')
            .peekable();

        while let Some(&high) = chars.next() {
            // If odd length, pad with '0'
            let low = chars.next().copied().unwrap_or(b'0');

            // Convert hex digits to byte
            let high_nibble = hex_digit_to_value(high).ok_or_else(|| {
                Error::Decode(format!("ASCIIHexDecode: invalid hex digit '{}'", high as char))
            })?;

            let low_nibble = hex_digit_to_value(low).ok_or_else(|| {
                Error::Decode(format!("ASCIIHexDecode: invalid hex digit '{}'", low as char))
            })?;

            let byte = (high_nibble << 4) | low_nibble;
            output.push(byte);
        }

        Ok(output)
    }

fn hex_digit_to_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}


pub(super) fn runlength(input: &[u8]) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        let mut i = 0;

        while i < input.len() {
            let length = input[i];
            i += 1;

            match length {
                0..=127 => {
                    // Copy next length+1 bytes literally
                    let count = length as usize + 1;

                    if i + count > input.len() {
                        return Err(Error::Decode(format!(
                            "RunLengthDecode: not enough data for literal run (need {}, have {})",
                            count,
                            input.len() - i
                        )));
                    }

                    output.extend_from_slice(&input[i..i + count]);
                    i += count;
                },
                128 => {
                    // EOD marker - no-op, but we'll break to end decoding
                    break;
                },
                129..=255 => {
                    // Repeat next byte 257-length times
                    let count = 257 - length as usize;

                    if i >= input.len() {
                        return Err(Error::Decode(
                            "RunLengthDecode: missing byte for run".to_string(),
                        ));
                    }

                    let byte = input[i];
                    i += 1;
                    output.resize(output.len() + count, byte);
                },
            }
        }

        Ok(output)
    }

pub(super) fn lzw(input: &[u8], early_change: bool) -> Result<Vec<u8>> {
    use weezl::{decode::Decoder, BitOrder};

    let mut decoder = if early_change {
        Decoder::with_tiff_size_switch(BitOrder::Msb, 8)
    } else {
        Decoder::new(BitOrder::Msb, 8)
    };
    decoder
        .decode(input)
        .map_err(|e| Error::Decode(format!("LZWDecode error: {:?}", e)))
}


use crate::decoders::DecodeParams;

pub(super) fn decode_predictor(data: &[u8], params: &DecodeParams) -> Result<Vec<u8>> {
    match params.predictor {
        1 => {
            // No predictor
            Ok(data.to_vec())
        },
        2 => {
            // TIFF Predictor 2
            decode_tiff_predictor(data, params)
        },
        10..=15 => {
            // PNG predictors
            decode_png_predictor(data, params)
        },
        _ => Err(Error::Decode(format!("Unsupported predictor: {}", params.predictor))),
    }
}

/// Decode TIFF Predictor 2.
///
/// TIFF Predictor 2 encodes the difference between adjacent samples in the same row.
fn decode_tiff_predictor(data: &[u8], params: &DecodeParams) -> Result<Vec<u8>> {
    let bytes_per_row = params.pixel_bytes_per_row();
    let colors = params.colors;

    if !data.len().is_multiple_of(bytes_per_row) {
        return Err(Error::Decode(format!(
            "Data length {} is not a multiple of row size {}",
            data.len(),
            bytes_per_row
        )));
    }

    let mut output = Vec::with_capacity(data.len());

    for row_data in data.chunks(bytes_per_row) {
        // First pixel in row is unchanged
        for i in 0..colors {
            output.push(row_data[i]);
        }

        // Subsequent pixels: add left neighbor
        for i in colors..row_data.len() {
            let left = output[output.len() - colors];
            output.push(row_data[i].wrapping_add(left));
        }
    }

    Ok(output)
}

/// Decode PNG predictors (10-15).
///
/// PNG predictors can vary per row (when using predictor 15).
/// Each row starts with a predictor tag byte indicating which algorithm to use.
fn decode_png_predictor(data: &[u8], params: &DecodeParams) -> Result<Vec<u8>> {
    let bytes_per_row = params.bytes_per_row(); // Includes predictor tag byte
    let pixel_bytes = params.pixel_bytes_per_row();

    if !data.len().is_multiple_of(bytes_per_row) {
        return Err(Error::Decode(format!(
            "Data length {} is not a multiple of row size {}",
            data.len(),
            bytes_per_row
        )));
    }

    let row_count = data.len() / bytes_per_row;
    let mut output = Vec::with_capacity(row_count * pixel_bytes);
    // The one departure from the old code: its byte distance was the colour
    // count, wrong at 16 bits and below 8 (ISO 32000-1 §7.4.4.4), and a
    // reference must not keep a bug the decoder no longer has.
    let bpp = params.bytes_per_pixel();

    for row_idx in 0..row_count {
        let row_start = row_idx * bytes_per_row;
        let row_data = &data[row_start..row_start + bytes_per_row];

        // PDF 32000-1:2008 §7.4.4.4: when PNG predictors (10-15) are used on
        // encoding, every row of the filtered data carries an explicit tag
        // byte identifying the per-row algorithm. For decoding, that per-row
        // tag is authoritative regardless of whether /Predictor is 10, 11,
        // 12, 13, 14, or 15 — the numeric /Predictor only signals encoder
        // intent. Honouring the declared predictor instead of the per-row
        // tag byte produces cascade noise on producers that emit tag 0
        // (None) under a non-10 /Predictor value, which the spec permits.
        let predictor_tag = row_data[0];

        let encoded_pixels = &row_data[1..]; // Skip predictor tag

        // Decode based on predictor type
        match predictor_tag {
            0 => {
                // None: copy as-is
                output.extend_from_slice(encoded_pixels);
            },
            1 => {
                // Sub: each byte is difference from left neighbor
                decode_png_sub(encoded_pixels, &mut output, bpp);
            },
            2 => {
                // Up: each byte is difference from above neighbor
                decode_png_up(encoded_pixels, &mut output, row_idx, pixel_bytes);
            },
            3 => {
                // Average: each byte is difference from average of left and above
                decode_png_average(encoded_pixels, &mut output, row_idx, pixel_bytes, bpp);
            },
            4 => {
                // Paeth: uses Paeth predictor function
                decode_png_paeth(encoded_pixels, &mut output, row_idx, pixel_bytes, bpp);
            },
            _ => {
                return Err(Error::Decode(format!("Invalid PNG predictor tag: {}", predictor_tag)));
            },
        }
    }

    Ok(output)
}

/// PNG Sub predictor: each byte is the difference from the left neighbor.
fn decode_png_sub(encoded: &[u8], output: &mut Vec<u8>, bpp: usize) {
    let start_pos = output.len();

    for (i, &byte) in encoded.iter().enumerate() {
        let left = if i >= bpp {
            output[start_pos + i - bpp]
        } else {
            0
        };
        output.push(byte.wrapping_add(left));
    }
}

/// PNG Up predictor: each byte is the difference from the byte above.
fn decode_png_up(encoded: &[u8], output: &mut Vec<u8>, row_idx: usize, pixel_bytes: usize) {
    for (i, &byte) in encoded.iter().enumerate() {
        let up = if row_idx > 0 {
            output[(row_idx - 1) * pixel_bytes + i]
        } else {
            0
        };
        output.push(byte.wrapping_add(up));
    }
}

/// PNG Average predictor: each byte is the difference from the average of left and above.
fn decode_png_average(
    encoded: &[u8],
    output: &mut Vec<u8>,
    row_idx: usize,
    pixel_bytes: usize,
    bpp: usize,
) {
    let start_pos = output.len();

    for (i, &byte) in encoded.iter().enumerate() {
        let left = if i >= bpp {
            output[start_pos + i - bpp] as u16
        } else {
            0
        };

        let up = if row_idx > 0 {
            output[(row_idx - 1) * pixel_bytes + i] as u16
        } else {
            0
        };

        let avg = ((left + up) / 2) as u8;
        output.push(byte.wrapping_add(avg));
    }
}

/// PNG Paeth predictor: uses the Paeth filter function.
fn decode_png_paeth(
    encoded: &[u8],
    output: &mut Vec<u8>,
    row_idx: usize,
    pixel_bytes: usize,
    bpp: usize,
) {
    let start_pos = output.len();

    for (i, &byte) in encoded.iter().enumerate() {
        let left = if i >= bpp {
            output[start_pos + i - bpp] as i16
        } else {
            0
        };

        let up = if row_idx > 0 {
            output[(row_idx - 1) * pixel_bytes + i] as i16
        } else {
            0
        };

        let up_left = if row_idx > 0 && i >= bpp {
            output[(row_idx - 1) * pixel_bytes + i - bpp] as i16
        } else {
            0
        };

        let paeth = paeth_predictor(left, up, up_left) as u8;
        output.push(byte.wrapping_add(paeth));
    }
}

/// Paeth predictor function from PNG specification.
fn paeth_predictor(a: i16, b: i16, c: i16) -> i16 {
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();

    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

