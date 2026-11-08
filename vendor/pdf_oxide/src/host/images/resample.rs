//! Downsampling for the reducer. Pure functions over 8-bit sample buffers.

use image::{imageops, GrayImage, ImageBuffer, RgbImage, RgbaImage};

use crate::error::{Error, Result};

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

fn expect_target(target_w: u32, target_h: u32, what: &str) -> Result<()> {
    if target_w == 0 || target_h == 0 {
        return Err(Error::Image(format!("{what}: target width and height must be non-zero")));
    }
    Ok(())
}

/// Bilinear (Triangle) resize of an interleaved buffer; `channels` 1, 3 or 4.
///
/// 4 channels goes through an RGBA buffer with the fourth lane carrying K;
/// the filter treats lanes independently, so the result is a valid CMYK
/// buffer. Triangle downscales scans and line art without the ringing
/// Lanczos would add or the aliasing Nearest would add.
pub fn resample_continuous(
    samples: &[u8],
    width: u32,
    height: u32,
    channels: u8,
    target_w: u32,
    target_h: u32,
) -> Result<Vec<u8>> {
    if !matches!(channels, 1 | 3 | 4) {
        return Err(Error::Image(format!(
            "resample_continuous: channels must be 1, 3 or 4, got {channels}"
        )));
    }
    expect_buffer_len(samples, width, height, channels as u32, "resample_continuous")?;
    expect_target(target_w, target_h, "resample_continuous")?;

    let out = match channels {
        1 => {
            let img: GrayImage = ImageBuffer::from_raw(width, height, samples.to_vec())
                .expect("length checked above");
            imageops::resize(&img, target_w, target_h, imageops::FilterType::Triangle).into_raw()
        },
        3 => {
            let img: RgbImage = ImageBuffer::from_raw(width, height, samples.to_vec())
                .expect("length checked above");
            imageops::resize(&img, target_w, target_h, imageops::FilterType::Triangle).into_raw()
        },
        4 => {
            let img: RgbaImage = ImageBuffer::from_raw(width, height, samples.to_vec())
                .expect("length checked above");
            imageops::resize(&img, target_w, target_h, imageops::FilterType::Triangle).into_raw()
        },
        _ => unreachable!("channels validated above"),
    };
    Ok(out)
}

/// Area-average then threshold at 128; input and output hold 0 or 255 per pixel.
///
/// Averaging before the threshold keeps a downsampled scan of text readable;
/// a thresholded nearest-neighbour pick drops strokes.
pub fn resample_bilevel(
    gray: &[u8],
    width: u32,
    height: u32,
    target_w: u32,
    target_h: u32,
) -> Result<Vec<u8>> {
    let averaged = resample_continuous(gray, width, height, 1, target_w, target_h)?;
    Ok(averaged
        .into_iter()
        .map(|v| if v >= 128 { 255 } else { 0 })
        .collect())
}

/// The source samples one destination sample is a weighted sum of.
struct Taps {
    start: u32,
    weights: Vec<f32>,
}

impl Taps {
    /// The triangle filter for destination sample `index` of `dst`, from a
    /// line of `src` samples. A reduction widens the filter to the scale
    /// factor so that every source sample counts; an enlargement uses the
    /// plain two-sample triangle, which is linear interpolation. Weights sum
    /// to one.
    fn triangle(index: u32, src: u32, dst: u32) -> Self {
        let scale = f64::from(src) / f64::from(dst);
        let support = scale.max(1.0);
        let centre = (f64::from(index) + 0.5) * scale;
        let start = ((centre - support).floor().max(0.0) as u32).min(src - 1);
        let end = ((centre + support).ceil().max(0.0) as u32).min(src).max(start + 1);
        let mut weights: Vec<f32> = (start..end)
            .map(|j| (1.0 - ((f64::from(j) + 0.5 - centre) / support).abs()).max(0.0) as f32)
            .collect();
        let sum: f32 = weights.iter().sum();
        if sum > 0.0 {
            for w in &mut weights {
                *w /= sum;
            }
        } else {
            // Not reachable for a filter that covers the centre; the nearest
            // sample stands in rather than a line of zeros.
            let nearest = (centre.floor() as i64).clamp(i64::from(start), i64::from(end) - 1);
            for (j, w) in (start..end).zip(weights.iter_mut()) {
                *w = if i64::from(j) == nearest { 1.0 } else { 0.0 };
            }
        }
        Self { start, weights }
    }

    fn end(&self) -> u32 {
        self.start + self.weights.len() as u32
    }
}

/// A separable triangle resampler of premultiplied RGBA8 that works one
/// source row at a time and yields destination rows in order.
///
/// The horizontal pass runs on each source row as it arrives; a destination
/// row is the vertical weighted sum of the few horizontally resampled rows
/// its filter spans, which are all the resampler holds. Memory is
/// `(2 * scale + 2)` rows of the destination width, not the image. Every
/// destination sample is a fixed function of the source samples under its
/// filter, so the rows come out the same however they are asked for.
pub struct RowResampler {
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    columns: Vec<Taps>,
    rows: Vec<Taps>,
    /// Horizontally resampled rows `(source index, destination-width f32 RGBA)`.
    ring: std::collections::VecDeque<(u32, Vec<f32>)>,
    /// Source rows given so far, resampled or skipped.
    fed: u32,
    /// Row buffers that left the ring, kept to be filled again.
    spare: Vec<Vec<f32>>,
}

impl RowResampler {
    /// A resampler from `src_w x src_h` to `dst_w x dst_h`; none is zero.
    pub fn new(src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Self {
        Self {
            src_w,
            src_h,
            dst_w,
            dst_h,
            columns: (0..dst_w).map(|i| Taps::triangle(i, src_w, dst_w)).collect(),
            rows: (0..dst_h).map(|i| Taps::triangle(i, src_h, dst_h)).collect(),
            ring: std::collections::VecDeque::new(),
            fed: 0,
            spare: Vec::new(),
        }
    }

    /// The destination size.
    pub fn dst_size(&self) -> (u32, u32) {
        (self.dst_w, self.dst_h)
    }

    /// The index of the next source row to give.
    pub fn next_source_row(&self) -> u32 {
        self.fed
    }

    /// The source rows `start..end` that destination row `dst_row` is made of.
    pub fn source_rows_of(&self, dst_row: u32) -> (u32, u32) {
        let taps = &self.rows[dst_row as usize];
        (taps.start, taps.end())
    }

    /// Passes over source row `next_source_row()` without using it: the rows
    /// before the first destination row wanted are never resampled.
    pub fn skip(&mut self) {
        self.fed += 1;
    }

    /// Gives source row `next_source_row()`, premultiplied RGBA8 of the
    /// source width.
    pub fn feed(&mut self, row: &[u8]) {
        debug_assert_eq!(row.len(), self.src_w as usize * 4);
        let mut out = self.spare.pop().unwrap_or_default();
        out.clear();
        out.resize(self.dst_w as usize * 4, 0.0);
        for (x, taps) in self.columns.iter().enumerate() {
            let mut acc = [0f32; 4];
            for (k, w) in taps.weights.iter().enumerate() {
                let at = (taps.start as usize + k) * 4;
                for c in 0..4 {
                    acc[c] += w * f32::from(row[at + c]);
                }
            }
            out[x * 4..x * 4 + 4].copy_from_slice(&acc);
        }
        self.ring.push_back((self.fed, out));
        self.fed += 1;
    }

    /// Writes destination row `dst_row` as premultiplied RGBA8. Every source
    /// row up to `source_rows_of(dst_row).1` must have been given, and none
    /// of the rows it spans skipped.
    pub fn take(&mut self, dst_row: u32, out: &mut [u8]) {
        debug_assert_eq!(out.len(), self.dst_w as usize * 4);
        let sums = self.weighted(dst_row);
        for (pixel, dst) in sums.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
            let a = (pixel[3] + 0.5).clamp(0.0, 255.0) as u8;
            for c in 0..3 {
                // A premultiplied colour never exceeds its alpha.
                dst[c] = ((pixel[c] + 0.5).clamp(0.0, 255.0) as u8).min(a);
            }
            dst[3] = a;
        }
    }

    /// [`Self::take`] for four independent samples a pixel (CMYK): the lanes
    /// are rounded as they are, with no colour held under an alpha.
    pub fn take_lanes(&mut self, dst_row: u32, out: &mut [u8]) {
        debug_assert_eq!(out.len(), self.dst_w as usize * 4);
        for (sum, dst) in self.weighted(dst_row).iter().zip(out.iter_mut()) {
            *dst = (sum + 0.5).clamp(0.0, 255.0) as u8;
        }
    }

    /// Destination row `dst_row` as the weighted sum of the rows it spans;
    /// rows before them leave the ring.
    fn weighted(&mut self, dst_row: u32) -> Vec<f32> {
        let (start, end) = self.source_rows_of(dst_row);
        while self.ring.front().is_some_and(|(index, _)| *index < start) {
            if let Some((_, buffer)) = self.ring.pop_front() {
                self.spare.push(buffer);
            }
        }
        let weights = &self.rows[dst_row as usize].weights;
        let mut sums = vec![0f32; self.dst_w as usize * 4];
        for (index, line) in &self.ring {
            if *index < start || *index >= end {
                continue;
            }
            let w = weights[(*index - start) as usize];
            for (sum, v) in sums.iter_mut().zip(line) {
                *sum += w * v;
            }
        }
        sums
    }

    /// Forgets every row given, to start again from source row 0.
    pub fn reset(&mut self) {
        while let Some((_, buffer)) = self.ring.pop_front() {
            self.spare.push(buffer);
        }
        self.fed = 0;
    }

    /// Bytes of horizontally resampled rows held.
    pub fn retained_bytes(&self) -> usize {
        self.ring.iter().map(|(_, row)| row.capacity() * 4).sum::<usize>()
            + self.spare.iter().map(|row| row.capacity() * 4).sum::<usize>()
    }

    /// The source height, for a caller that checks it against its rows.
    pub fn src_height(&self) -> u32 {
        self.src_h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray_gradient(width: u32, height: u32) -> Vec<u8> {
        (0..height)
            .flat_map(|y| (0..width).map(move |x| ((x + y) % 256) as u8))
            .collect()
    }

    #[test]
    fn continuous_resample_keeps_the_mean_within_one_level() {
        let width = 64u32;
        let height = 64u32;
        let samples = gray_gradient(width, height);
        let mean_in = samples.iter().map(|&v| v as f64).sum::<f64>() / samples.len() as f64;

        let out = resample_continuous(&samples, width, height, 1, 16, 16).expect("resamples");
        assert_eq!(out.len(), 256);
        let mean_out = out.iter().map(|&v| v as f64).sum::<f64>() / out.len() as f64;
        assert!((mean_in - mean_out).abs() <= 1.0, "mean_in={mean_in} mean_out={mean_out}");
    }

    #[test]
    fn continuous_resample_handles_four_channels_independently() {
        let width = 8u32;
        let height = 8u32;
        let lanes = [10u8, 20, 30, 40];
        let samples: Vec<u8> = (0..width * height).flat_map(|_| lanes).collect();

        let out = resample_continuous(&samples, width, height, 4, 4, 4).expect("resamples");
        for pixel in out.chunks_exact(4) {
            assert_eq!(pixel, &lanes);
        }
    }

    #[test]
    fn bilevel_resample_keeps_a_one_output_pixel_rule_at_four_to_one() {
        let width = 64u32;
        let height = 64u32;
        let mut gray = vec![255u8; (width * height) as usize];
        for y in 0..height as usize {
            for x in 28..32usize {
                gray[y * width as usize + x] = 0;
            }
        }

        let out = resample_bilevel(&gray, width, height, 16, 16).expect("resamples");
        for row in out.chunks_exact(16) {
            assert!(row.iter().any(|&v| v == 0), "row {row:?} has no black pixel");
        }
    }

    #[test]
    fn bilevel_resample_outputs_only_zero_and_255() {
        let width = 33u32;
        let height = 17u32;
        let mut state: u32 = 0x2545F491;
        let mut gray = vec![0u8; (width * height) as usize];
        for pixel in gray.iter_mut() {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            *pixel = if (state >> 16) & 1 == 0 { 0 } else { 255 };
        }

        let out = resample_bilevel(&gray, width, height, 9, 5).expect("resamples");
        for &v in &out {
            assert!(v == 0 || v == 255, "unexpected value {v}");
        }
    }

    #[test]
    fn resamplers_reject_bad_dimensions() {
        let samples = vec![255u8; 16];
        assert!(resample_continuous(&samples, 4, 4, 1, 0, 4).is_err());
        assert!(resample_continuous(&samples, 4, 4, 1, 4, 0).is_err());
        assert!(resample_continuous(&[0u8; 5], 4, 4, 1, 2, 2).is_err());
        assert!(resample_bilevel(&samples, 4, 4, 0, 4).is_err());
        assert!(resample_bilevel(&[0u8; 5], 4, 4, 2, 2).is_err());
    }
}
