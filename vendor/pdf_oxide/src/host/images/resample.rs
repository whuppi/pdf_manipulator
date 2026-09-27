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
