//! Encoders the reducer writes with: each takes 8-bit sample rows one at a
//! time, and has a form over a held buffer; nothing here reads a PDF.

use std::cell::RefCell;
use std::convert::Infallible;
use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::error::{Error, Result};
use jpeg_encoder::{rgb_to_ycbcr, Encoder, ImageBuffer, JpegColorType, SamplingFactor};

use super::policy::Subsampling;

/// Flate output with the PNG predictor parameters a reader needs.
pub struct FlateEncoded {
    /// The zlib stream (each row prefixed by its PNG filter byte before compression).
    pub bytes: Vec<u8>,
    /// Always 15 (`/Predictor 15`: PNG filter byte per row).
    pub predictor: u8,
    /// `/Colors`.
    pub colors: u8,
    /// `/Columns`.
    pub columns: u32,
}

fn expect_buffer_len(
    samples: &[u8],
    width: u32,
    height: u32,
    channels: u32,
    what: &str,
) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(Error::Image(format!("{what}: width and height must be non-zero")));
    }
    let expected = width as usize * height as usize * channels as usize;
    if samples.len() != expected {
        return Err(Error::Image(format!(
            "{what}: buffer holds {} bytes, expected {expected} ({width}x{height}x{channels})",
            samples.len()
        )));
    }
    Ok(())
}

/// A `Result<T, Infallible>` never carries an error; this makes that visible
/// at the call site instead of reaching for `.unwrap()`.
fn infallible<T>(result: std::result::Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let (a, b, c) = (a as i32, b as i32, c as i32);
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    if pa <= pb && pa <= pc {
        a as u8
    } else if pb <= pc {
        b as u8
    } else {
        c as u8
    }
}

/// The five PNG filters (PNG spec §9), applied byte-wise with `bpp` the
/// distance to the left neighbour that shares the same channel.
fn filter_row(filter: u8, row: &[u8], prior: &[u8], bpp: usize, out: &mut [u8]) {
    for i in 0..row.len() {
        let a = if i >= bpp { row[i - bpp] } else { 0 };
        let b = prior[i];
        let c = if i >= bpp { prior[i - bpp] } else { 0 };
        out[i] = match filter {
            0 => row[i],
            1 => row[i].wrapping_sub(a),
            2 => row[i].wrapping_sub(b),
            3 => row[i].wrapping_sub(((a as u16 + b as u16) / 2) as u8),
            4 => row[i].wrapping_sub(paeth_predictor(a, b, c)),
            _ => unreachable!("only filters 0-4 exist"),
        };
    }
}

/// Sum of the row's bytes read as signed residuals — the libpng heuristic
/// for picking the filter that compresses best.
fn residual_score(row: &[u8]) -> u64 {
    row.iter().map(|&b| (b as i8).unsigned_abs() as u64).sum()
}

/// Picks the PNG filter with the smallest residual sum for one row and
/// writes filter byte + filtered row into `out`.
pub(crate) fn best_filtered_row(row: &[u8], prior: &[u8], bpp: usize, out: &mut Vec<u8>) {
    let mut candidate = vec![0u8; row.len()];
    let mut best_filter = 0u8;
    let mut best_score = u64::MAX;
    let mut best_row = vec![0u8; row.len()];
    for filter in 0..=4u8 {
        filter_row(filter, row, prior, bpp, &mut candidate);
        let score = residual_score(&candidate);
        if score < best_score {
            best_score = score;
            best_filter = filter;
            best_row.copy_from_slice(&candidate);
        }
    }
    out.push(best_filter);
    out.extend_from_slice(&best_row);
}

/// Flate with PNG predictors, 8 bits per component, rows pushed one at a
/// time: what is held is the previous row and the compressed output.
pub struct FlateRows<W: Write = Vec<u8>> {
    zlib: ZlibEncoder<W>,
    prior: Vec<u8>,
    filtered: Vec<u8>,
    channels: u8,
    width: u32,
}

impl<W: Write> FlateRows<W> {
    /// An encoder whose compressed rows stream straight into `w`, so an
    /// image being written is never held encoded whole.
    pub fn to_writer(w: W, width: u32, channels: u8) -> Result<Self> {
        if !matches!(channels, 1 | 3 | 4) {
            return Err(Error::Image(format!(
                "encode_flate_predicted: channels must be 1, 3 or 4, got {channels}"
            )));
        }
        if width == 0 {
            return Err(Error::Image("encode_flate_predicted: width must be non-zero".to_string()));
        }
        Ok(Self {
            zlib: ZlibEncoder::new(w, Compression::best()),
            prior: vec![0u8; width as usize * channels as usize],
            filtered: Vec::new(),
            channels,
            width,
        })
    }

    /// The next row, `width * channels` bytes.
    pub fn push(&mut self, row: &[u8]) -> Result<()> {
        if row.len() != self.prior.len() {
            return Err(Error::Image(format!(
                "encode_flate_predicted: row holds {} bytes, expected {}",
                row.len(),
                self.prior.len()
            )));
        }
        self.filtered.clear();
        best_filtered_row(row, &self.prior, self.channels as usize, &mut self.filtered);
        self.prior.copy_from_slice(row);
        self.zlib
            .write_all(&self.filtered)
            .map_err(|e| Error::Image(format!("encode_flate_predicted: zlib write failed: {e}")))
    }

    /// Closes the stream and gives the writer back.
    pub fn finish_writer(self) -> Result<W> {
        self.zlib
            .finish()
            .map_err(|e| Error::Image(format!("encode_flate_predicted: zlib finish failed: {e}")))
    }
}

impl FlateRows<Vec<u8>> {
    /// An encoder for rows of `width` pixels of `channels` 1, 3 or 4 samples.
    pub fn new(width: u32, channels: u8) -> Result<Self> {
        Self::to_writer(Vec::new(), width, channels)
    }

    /// The finished stream.
    pub fn finish(self) -> Result<FlateEncoded> {
        let (colors, columns) = (self.channels, self.width);
        let bytes = self.finish_writer()?;
        Ok(FlateEncoded {
            bytes,
            predictor: 15,
            colors,
            columns,
        })
    }
}

/// Flate with PNG predictors, 8 bits per component, `channels` 1, 3 or 4.
pub fn encode_flate_predicted(
    samples: &[u8],
    width: u32,
    height: u32,
    channels: u8,
) -> Result<FlateEncoded> {
    let mut rows = FlateRows::new(width, channels)?;
    expect_buffer_len(samples, width, height, channels as u32, "encode_flate_predicted")?;
    for row in samples.chunks_exact(width as usize * channels as usize) {
        rows.push(row)?;
    }
    rows.finish()
}

/// The rows `next_row` yields, as the encoder asks for them: in order, the
/// last one again for the lines that pad the final MCU row.
struct RowFeed<'a> {
    width: u16,
    height: u16,
    channels: u8,
    state: RefCell<FeedState<'a>>,
    /// The first failure of `next_row`, which the encoder cannot carry.
    failure: &'a RefCell<Option<Error>>,
}

struct FeedState<'a> {
    next: u32,
    row: Vec<u8>,
    pull: &'a mut dyn FnMut(&mut [u8]) -> Result<()>,
}

impl ImageBuffer for RowFeed<'_> {
    fn get_jpeg_color_type(&self) -> JpegColorType {
        if self.channels == 1 {
            JpegColorType::Luma
        } else {
            JpegColorType::Ycbcr
        }
    }

    fn width(&self) -> u16 {
        self.width
    }

    fn height(&self) -> u16 {
        self.height
    }

    fn fill_buffers(&self, y: u16, buffers: &mut [Vec<u8>; 4]) {
        let mut guard = self.state.borrow_mut();
        let state = &mut *guard;
        while state.next <= u32::from(y) {
            if self.failure.borrow().is_none() {
                if let Err(e) = (state.pull)(&mut state.row) {
                    *self.failure.borrow_mut() = Some(e);
                }
            }
            state.next += 1;
        }
        if self.channels == 1 {
            buffers[0].extend_from_slice(&state.row);
        } else {
            for px in state.row.chunks_exact(3) {
                let (luma, cb, cr) = rgb_to_ycbcr(px[0], px[1], px[2]);
                buffers[0].push(luma);
                buffers[1].push(cb);
                buffers[2].push(cr);
            }
        }
    }
}

/// Baseline JPEG; `channels` 1 (L8) or 3 (Rgb8); `quality` 1..=100. The rows
/// are asked of `next_row` one at a time, `width * channels` bytes each, so
/// the encoder holds a band of MCU rows and its output, never the image.
/// Huffman tables are the standard ones: optimised tables need every
/// coefficient of the image before the first byte is written.
pub fn encode_jpeg_rows(
    width: u32,
    height: u32,
    channels: u8,
    quality: u8,
    subsampling: Subsampling,
    next_row: &mut dyn FnMut(&mut [u8]) -> Result<()>,
) -> Result<Vec<u8>> {
    if !matches!(channels, 1 | 3) {
        return Err(Error::Image(format!("encode_jpeg: channels must be 1 or 3, got {channels}")));
    }
    if !(1..=100).contains(&quality) {
        return Err(Error::Image(format!("encode_jpeg: quality must be 1..=100, got {quality}")));
    }
    if width == 0 || height == 0 {
        return Err(Error::Image("encode_jpeg: width and height must be non-zero".to_string()));
    }
    // Baseline JPEG carries dimensions in 16 bits.
    let (w16, h16) = match (u16::try_from(width), u16::try_from(height)) {
        (Ok(w), Ok(h)) => (w, h),
        _ => return Err(Error::Image(format!("encode_jpeg: {width}x{height} exceeds 65535"))),
    };
    let failure = RefCell::new(None);
    let feed = RowFeed {
        width: w16,
        height: h16,
        channels,
        state: RefCell::new(FeedState {
            next: 0,
            row: vec![0u8; width as usize * channels as usize],
            pull: next_row,
        }),
        failure: &failure,
    };
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, quality);
    encoder.set_sampling_factor(match subsampling {
        Subsampling::Full => SamplingFactor::F_1_1,
        Subsampling::Half => SamplingFactor::F_2_2,
    });
    let encoded = encoder.encode_image(feed);
    if let Some(e) = failure.into_inner() {
        return Err(e);
    }
    encoded.map_err(|e| Error::Image(format!("encode_jpeg: {e}")))?;
    Ok(bytes)
}

/// [`encode_jpeg_rows`] of a held buffer.
pub fn encode_jpeg(
    samples: &[u8],
    width: u32,
    height: u32,
    channels: u8,
    quality: u8,
    subsampling: Subsampling,
) -> Result<Vec<u8>> {
    if !matches!(channels, 1 | 3) {
        return Err(Error::Image(format!("encode_jpeg: channels must be 1 or 3, got {channels}")));
    }
    expect_buffer_len(samples, width, height, channels as u32, "encode_jpeg")?;
    let mut rows = samples.chunks_exact(width as usize * channels as usize);
    encode_jpeg_rows(width, height, channels, quality, subsampling, &mut |out| {
        out.copy_from_slice(rows.next().ok_or_else(|| Error::Image("encode_jpeg: no more rows".to_string()))?);
        Ok(())
    })
}

/// CCITT Group 4 (K = −1), a line at a time, of rows holding 0 (black) or 255
/// (white) per pixel, for a dictionary saying `/BlackIs1 false`: what is held
/// is the previous line and the encoded output.
pub struct CcittRows {
    encoder: fax::encoder::Encoder<fax::VecWriter>,
    width: u32,
}

impl CcittRows {
    /// An encoder for lines of `width` pixels.
    pub fn new(width: u32) -> Result<Self> {
        if width == 0 {
            return Err(Error::Image("encode_ccitt_g4: width must be non-zero".to_string()));
        }
        Ok(Self {
            encoder: fax::encoder::Encoder::new(fax::VecWriter::new()),
            width,
        })
    }

    /// The next line, `width` bytes.
    pub fn push(&mut self, row: &[u8]) -> Result<()> {
        if row.len() != self.width as usize {
            return Err(Error::Image(format!(
                "encode_ccitt_g4: line holds {} bytes, expected {}",
                row.len(),
                self.width
            )));
        }
        let pels = row.iter().map(|&v| {
            if v == 0 {
                fax::Color::Black
            } else {
                fax::Color::White
            }
        });
        self.encoder
            .encode_line(pels, self.width)
            .map_err(|e| Error::Image(format!("encode_ccitt_g4: line encode failed: {e:?}")))
    }
}

impl CcittRows {
    /// The finished stream.
    pub fn finish(self) -> Vec<u8> {
        infallible(self.encoder.finish()).finish()
    }
}

/// CCITT Group 4 (K = −1) of a buffer holding 0 (black) or 255 (white) per pixel,
/// for a dictionary saying `/BlackIs1 false`.
pub fn encode_ccitt_g4(gray: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    expect_buffer_len(gray, width, height, 1, "encode_ccitt_g4")?;
    let mut rows = CcittRows::new(width)?;
    for row in gray.chunks_exact(width as usize) {
        rows.push(row)?;
    }
    Ok(rows.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoders::{decode_predictor, CcittParams, DecodeParams};
    use crate::extractors::ccitt_bilevel::{bilevel_to_grayscale, decompress_ccitt};
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    fn gradient_rgb(width: u32, height: u32) -> Vec<u8> {
        let mut samples = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                samples.push((x * 16) as u8);
                samples.push((y * 32) as u8);
                samples.push(((x + y) * 8) as u8);
            }
        }
        samples
    }

    fn inflate(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        ZlibDecoder::new(bytes)
            .read_to_end(&mut out)
            .expect("valid zlib stream");
        out
    }

    #[test]
    fn flate_predicted_round_trips_through_the_crate_decoder() {
        let samples = gradient_rgb(16, 8);
        let encoded = encode_flate_predicted(&samples, 16, 8, 3).expect("encodes");
        assert_eq!(encoded.predictor, 15);
        assert_eq!(encoded.colors, 3);
        assert_eq!(encoded.columns, 16);

        let inflated = inflate(&encoded.bytes);
        let decoded = decode_predictor(
            &inflated,
            &DecodeParams {
                predictor: 15,
                columns: 16,
                colors: 3,
                bits_per_component: 8,
                early_change: true,
            },
        )
        .expect("valid predictor stream");
        assert_eq!(decoded, samples);
    }

    #[test]
    fn flate_predicted_is_smaller_than_plain_flate_on_a_gradient() {
        let samples = gradient_rgb(64, 64);
        let predicted = encode_flate_predicted(&samples, 64, 64, 3).expect("encodes");

        let mut plain = ZlibEncoder::new(Vec::new(), Compression::default());
        plain.write_all(&samples).expect("write");
        let plain_bytes = plain.finish().expect("finish");

        assert!(predicted.bytes.len() < plain_bytes.len());
    }

    fn psnr(a: &[u8], b: &[u8]) -> f64 {
        let mse: f64 = a
            .iter()
            .zip(b)
            .map(|(&x, &y)| {
                let d = x as f64 - y as f64;
                d * d
            })
            .sum::<f64>()
            / a.len() as f64;
        if mse == 0.0 {
            return f64::INFINITY;
        }
        10.0 * (255.0f64.powi(2) / mse).log10()
    }

    #[test]
    fn jpeg_decodes_back_to_the_same_size_and_close_pixels() {
        let width = 32u32;
        let height = 32u32;
        let mut samples = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            for x in 0..width {
                samples.push(((x + y) * 4) as u8);
            }
        }
        let bytes = encode_jpeg(&samples, width, height, 1, 90, Subsampling::Half).expect("encodes");
        let decoded = image::load_from_memory(&bytes)
            .expect("valid jpeg")
            .into_luma8();
        assert_eq!(decoded.width(), width);
        assert_eq!(decoded.height(), height);
        assert!(psnr(&samples, decoded.as_raw()) >= 35.0);
    }

    #[test]
    fn jpeg_rows_are_asked_once_each_at_a_size_that_is_no_multiple_of_an_mcu() {
        let (w, h) = (37u32, 21u32);
        let mut asked = 0u32;
        let bytes = encode_jpeg_rows(w, h, 3, 85, Subsampling::Half, &mut |row| {
            for (i, b) in row.iter_mut().enumerate() {
                *b = ((asked * 7 + i as u32) % 251) as u8;
            }
            asked += 1;
            Ok(())
        })
        .expect("encodes");
        assert_eq!(asked, h);
        let decoded = image::load_from_memory(&bytes).expect("valid jpeg");
        assert_eq!((decoded.width(), decoded.height()), (w, h));
    }

    #[test]
    fn a_row_source_that_fails_ends_the_jpeg_with_its_error() {
        let mut asked = 0;
        let result = encode_jpeg_rows(16, 16, 1, 75, Subsampling::Full, &mut |_| {
            asked += 1;
            if asked == 5 {
                Err(Error::Image("no more rows".to_string()))
            } else {
                Ok(())
            }
        });
        assert!(matches!(result, Err(Error::Image(m)) if m == "no more rows"));
    }

    #[test]
    fn jpeg_rejects_four_channels() {
        let samples = vec![0u8; 4 * 4 * 4];
        assert!(encode_jpeg(&samples, 4, 4, 4, 75, Subsampling::Half).is_err());
    }

    #[test]
    fn ccitt_g4_round_trips_through_the_crate_decoder() {
        let width = 40u32;
        let height = 12u32;
        let mut gray = vec![0u8; (width * height) as usize];
        for y in 0..height as usize {
            for x in 0..width as usize {
                gray[y * width as usize + x] = if y < 6 {
                    0
                } else if x % 5 == 0 {
                    0
                } else {
                    255
                };
            }
        }

        let bytes = encode_ccitt_g4(&gray, width, height).expect("encodes");
        let params = CcittParams {
            columns: width,
            rows: Some(height),
            k: -1,
            ..Default::default()
        };
        let decoded = decompress_ccitt(&bytes, &params).expect("valid ccitt stream");
        let regray = bilevel_to_grayscale(&decoded, width, height);
        assert_eq!(regray, gray);
    }

    #[test]
    fn every_encoder_rejects_a_short_buffer() {
        assert!(encode_flate_predicted(&[0u8; 5], 4, 4, 3).is_err());
        assert!(encode_jpeg(&[0u8; 5], 4, 4, 1, 75, Subsampling::Half).is_err());
        assert!(encode_ccitt_g4(&[0u8; 5], 4, 4).is_err());
    }

    #[test]
    fn full_chroma_costs_more_bytes_than_half_and_both_decode() {
        // Hard colour edges: the case where 4:2:0 smears and 4:4:4 earns its bytes.
        let (w, h) = (32u32, 32u32);
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let red = (x / 4 + y / 4) % 2 == 0;
                rgb.extend_from_slice(if red { &[220, 20, 20] } else { &[20, 20, 220] });
            }
        }
        let half = encode_jpeg(&rgb, w, h, 3, 85, Subsampling::Half).expect("half");
        let full = encode_jpeg(&rgb, w, h, 3, 85, Subsampling::Full).expect("full");
        assert!(full.len() > half.len(), "full {} vs half {}", full.len(), half.len());
        for bytes in [&half, &full] {
            let img = image::load_from_memory(bytes).expect("decodes");
            assert_eq!((img.width(), img.height()), (w, h));
        }
    }
}
