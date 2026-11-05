//! Filters and adjustment passes.
//!
//! Every operation here is generic over the **storage channel**, so it runs natively on an
//! `Srgb16` layer as well as an `Srgb8` one. The pattern matches the compositor and the
//! rasterizer: the maths is written once, dispatch happens once per operation (see the layer
//! entry points at the bottom), and no pixel loop ever asks what format it is writing.
//!
//! Two rules make that safe for existing output:
//!
//! * **Arithmetic stays in the channel's own scale.** `to_scaled_f32` is `0..=255` for `u8`
//!   and `0..=65535` for `u16`, so a `u8` filter computes exactly the numbers it always did and
//!   a `u16` filter computes the same formulas at its own precision.
//! * **The rounding rule travels with the filter.** Some filters round (`store_rounded`), some
//!   have always truncated (`store_truncated` - a float `as u8`). Unifying them would move
//!   bytes, so both exist and each filter keeps its own.
//!
//! `eight_bit_filter_output_is_unchanged` pins the result of every filter, captured before this
//! module was generalised.

use crate::layer::{Layer, LayerBuffer};
use crate::pixel::Channel;
use crate::selection::SelectionMask;

// ── Colour-space helpers ────────────────────────────────────────────────────────────────────

/// Converts RGB (0..255) to HSL (H: 0..360, S: 0..1, L: 0..1)
pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    rgb_to_hsl_f32([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0])
}

/// Converts normalised RGB to HSL (H: 0..360, S: 0..1, L: 0..1).
///
/// The working-precision form of [`rgb_to_hsl`]: identical arithmetic, without the trip
/// through 8-bit channels. One implementation serves both, so a 16-bit adjustment cannot
/// drift away from the 8-bit one by being written twice.
pub fn rgb_to_hsl_f32(rgb: [f32; 3]) -> (f32, f32, f32) {
    let [rf, gf, bf] = rgb;

    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;

    let l = (max + min) * 0.5;

    if delta.abs() < 1e-5 {
        return (0.0, 0.0, l);
    }

    let s = if l > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };

    let mut h = if (max - rf).abs() < 1e-5 {
        (gf - bf) / delta + (if gf < bf { 6.0 } else { 0.0 })
    } else if (max - gf).abs() < 1e-5 {
        (bf - rf) / delta + 2.0
    } else {
        (rf - gf) / delta + 4.0
    };

    h *= 60.0;
    (h, s, l)
}

/// Converts HSL (H: 0..360, S: 0..1, L: 0..1) to RGB (0..255)
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let [r, g, b] = hsl_to_rgb_f32(h, s, l);
    (
        (r * 255.0).clamp(0.0, 255.0).round() as u8,
        (g * 255.0).clamp(0.0, 255.0).round() as u8,
        (b * 255.0).clamp(0.0, 255.0).round() as u8,
    )
}

/// Converts HSL (H: 0..360, S: 0..1, L: 0..1) to normalised RGB.
///
/// The working-precision form of [`hsl_to_rgb`]. The 8-bit wrapper multiplies and rounds the
/// result, which is the only difference between them, so quantisation happens once, at the
/// boundary, instead of inside the maths.
pub fn hsl_to_rgb_f32(h: f32, s: f32, l: f32) -> [f32; 3] {
    if s.abs() < 1e-5 {
        let val = l.clamp(0.0, 1.0);
        return [val, val, val];
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    let hk = ((h % 360.0) + 360.0) % 360.0 / 360.0;

    let hue_to_rgb = |t: f32| -> f32 {
        let mut tc = t;
        if tc < 0.0 {
            tc += 1.0;
        }
        if tc > 1.0 {
            tc -= 1.0;
        }
        if tc < 1.0 / 6.0 {
            p + (q - p) * 6.0 * tc
        } else if tc < 0.5 {
            q
        } else if tc < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - tc) * 6.0
        } else {
            p
        }
    };

    [
        hue_to_rgb(hk + 1.0 / 3.0).clamp(0.0, 1.0),
        hue_to_rgb(hk).clamp(0.0, 1.0),
        hue_to_rgb(hk - 1.0 / 3.0).clamp(0.0, 1.0),
    ]
}

// ── Storage access ──────────────────────────────────────────────────────────────────────────

/// The channel's maximum in its own scale: `255` for `u8`, `65535` for `u16`.
#[inline]
fn max_scaled<C: Channel>() -> f32 {
    C::MAX_SCALED as f32
}

/// `255`-relative constants (brush amounts, thresholds) expressed in the channel's own scale.
#[inline]
fn per_255<C: Channel>(value: f32) -> f32 {
    value * (max_scaled::<C>() / 255.0)
}

#[inline]
fn read_scaled<C: Channel>(pixels: &[C], idx: usize) -> [f32; 4] {
    [
        pixels[idx].to_scaled_f32(),
        pixels[idx + 1].to_scaled_f32(),
        pixels[idx + 2].to_scaled_f32(),
        pixels[idx + 3].to_scaled_f32(),
    ]
}

/// Writes a pixel with the round-to-nearest rule (the majority of the filters).
#[inline]
fn store_rounded<C: Channel>(pixels: &mut [C], idx: usize, pixel: [f32; 4]) {
    pixels[idx] = C::from_scaled_f32(pixel[0]);
    pixels[idx + 1] = C::from_scaled_f32(pixel[1]);
    pixels[idx + 2] = C::from_scaled_f32(pixel[2]);
    pixels[idx + 3] = C::from_scaled_f32(pixel[3]);
}

/// Writes a pixel with the truncation rule (sepia, posterize, grain, vignette).
#[inline]
fn store_truncated<C: Channel>(pixels: &mut [C], idx: usize, pixel: [f32; 4]) {
    pixels[idx] = C::from_scaled_f32_trunc(pixel[0]);
    pixels[idx + 1] = C::from_scaled_f32_trunc(pixel[1]);
    pixels[idx + 2] = C::from_scaled_f32_trunc(pixel[2]);
    pixels[idx + 3] = C::from_scaled_f32_trunc(pixel[3]);
}

// ── Adjustment passes ──────────────────────────────────────────────────────────────────────

/// Adjusts Hue, Saturation, and Lightness of pixels in-place
pub fn adjust_hsl<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    hue_shift: f32,        // -180.0 to +180.0 degrees
    saturation_scale: f32, // 0.0 to 3.0 (1.0 = normal)
    lightness_shift: f32,  // -1.0 to +1.0 (0.0 = normal)
    selection: Option<&SelectionMask>,
) {
    let mx = max_scaled::<C>();
    let total_pixels = (width * height) as usize;
    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let px = read_scaled(pixels, idx);
        if px[3] == 0.0 {
            continue;
        }

        let (h, s, l) = rgb_to_hsl_f32([px[0] / mx, px[1] / mx, px[2] / mx]);
        let new_h = (h + hue_shift + 360.0) % 360.0;
        let new_s = (s * saturation_scale).clamp(0.0, 1.0);
        let new_l = (l + lightness_shift).clamp(0.0, 1.0);

        let rgb = hsl_to_rgb_f32(new_h, new_s, new_l);
        pixels[idx] = C::from_scaled_f32(rgb[0] * mx);
        pixels[idx + 1] = C::from_scaled_f32(rgb[1] * mx);
        pixels[idx + 2] = C::from_scaled_f32(rgb[2] * mx);
    }
}

/// Adjusts Brightness (-100..=100) and Contrast (-100..=100)
pub fn adjust_brightness_contrast<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    brightness: f32, // -100.0 to +100.0
    contrast: f32,   // -100.0 to +100.0
    selection: Option<&SelectionMask>,
) {
    // Contrast factor formula
    let c_factor = (259.0 * (contrast + 255.0)) / (255.0 * (259.0 - contrast));
    // 128 on the 8-bit scale, not half the maximum: 255/2 is 127.5 and would move every byte.
    let midpoint = per_255::<C>(128.0);
    let brightness_bias = per_255::<C>(brightness * 2.55);
    let total_pixels = (width * height) as usize;

    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let mut px = read_scaled(pixels, idx);
        if px[3] == 0.0 {
            continue;
        }

        for channel in 0..3 {
            px[channel] = (c_factor * (px[channel] + brightness_bias - midpoint) + midpoint)
                .clamp(0.0, max_scaled::<C>());
        }
        store_rounded(pixels, idx, px);
    }
}

/// Color Balance: adjusts Red, Green, and Blue bias for shadows, midtones, and highlights
pub fn adjust_color_balance<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    cyan_red: f32,      // -100.0 (Cyan) to +100.0 (Red)
    magenta_green: f32, // -100.0 (Magenta) to +100.0 (Green)
    yellow_blue: f32,   // -100.0 (Yellow) to +100.0 (Blue)
    selection: Option<&SelectionMask>,
) {
    let total_pixels = (width * height) as usize;
    let r_bias = per_255::<C>(cyan_red * 1.28);
    let g_bias = per_255::<C>(magenta_green * 1.28);
    let b_bias = per_255::<C>(yellow_blue * 1.28);
    let mx = max_scaled::<C>();

    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let px = read_scaled(pixels, idx);
        if px[3] == 0.0 {
            continue;
        }

        pixels[idx] = C::from_scaled_f32((px[0] + r_bias).clamp(0.0, mx));
        pixels[idx + 1] = C::from_scaled_f32((px[1] + g_bias).clamp(0.0, mx));
        pixels[idx + 2] = C::from_scaled_f32((px[2] + b_bias).clamp(0.0, mx));
    }
}

// ── Filters ────────────────────────────────────────────────────────────────────────────────

/// Inverts RGB channels
pub fn filter_invert<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    selection: Option<&SelectionMask>,
) {
    let mx = max_scaled::<C>();
    let total_pixels = (width * height) as usize;
    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let px = read_scaled(pixels, idx);
        if px[3] == 0.0 {
            continue;
        }

        pixels[idx] = C::from_scaled_f32(mx - px[0]);
        pixels[idx + 1] = C::from_scaled_f32(mx - px[1]);
        pixels[idx + 2] = C::from_scaled_f32(mx - px[2]);
    }
}

/// Posterizes color channels to discrete tonal levels (2 to 32)
pub fn filter_posterize<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    levels: u32,
    selection: Option<&SelectionMask>,
) {
    let mx = max_scaled::<C>();
    let levels = levels.clamp(2, 32) as f32;
    let step = mx / (levels - 1.0);
    let total_pixels = (width * height) as usize;

    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let px = read_scaled(pixels, idx);
        if px[3] == 0.0 {
            continue;
        }

        // Truncating, as it always was.
        for channel in 0..3 {
            let quantized = (px[channel] / step).round() * step;
            pixels[idx + channel] = C::from_scaled_f32_trunc(quantized.clamp(0.0, mx));
        }
    }
}

/// Converts layer to perceptual grayscale
pub fn filter_grayscale<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    selection: Option<&SelectionMask>,
) {
    let total_pixels = (width * height) as usize;
    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let px = read_scaled(pixels, idx);
        if px[3] == 0.0 {
            continue;
        }

        let lum = C::from_scaled_f32(0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2]);
        pixels[idx] = lum;
        pixels[idx + 1] = lum;
        pixels[idx + 2] = lum;
    }
}

/// Applies vintage sepia photographic tone
pub fn filter_sepia<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    selection: Option<&SelectionMask>,
) {
    let mx = max_scaled::<C>();
    let total_pixels = (width * height) as usize;
    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let px = read_scaled(pixels, idx);
        if px[3] == 0.0 {
            continue;
        }

        let (r, g, b) = (px[0], px[1], px[2]);
        // Truncating, as it always was.
        pixels[idx] = C::from_scaled_f32_trunc(
            (0.393 * r + 0.769 * g + 0.189 * b).clamp(0.0, mx),
        );
        pixels[idx + 1] = C::from_scaled_f32_trunc(
            (0.349 * r + 0.686 * g + 0.168 * b).clamp(0.0, mx),
        );
        pixels[idx + 2] = C::from_scaled_f32_trunc(
            (0.272 * r + 0.534 * g + 0.131 * b).clamp(0.0, mx),
        );
    }
}

/// Binary Black & White Thresholding
pub fn filter_threshold<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    threshold: u8,
    selection: Option<&SelectionMask>,
) {
    let total_pixels = (width * height) as usize;
    // The cutoff is an 8-bit quantity, so it is lifted into the channel's scale.
    let thresh_f = per_255::<C>(threshold as f32);
    let mx = max_scaled::<C>();

    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let px = read_scaled(pixels, idx);
        if px[3] == 0.0 {
            continue;
        }

        let lum = 0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2];
        let value = C::from_scaled_f32(if lum >= thresh_f { mx } else { 0.0 });
        pixels[idx] = value;
        pixels[idx + 1] = value;
        pixels[idx + 2] = value;
    }
}

/// High-performance Separable 1D Gaussian Blur with configurable radius (1..=48)
pub fn filter_gaussian_blur<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    radius: f32,
    selection: Option<&SelectionMask>,
) {
    if radius <= 0.2 || width == 0 || height == 0 {
        return;
    }

    let mx = max_scaled::<C>();
    let r = radius.clamp(1.0, 48.0);
    let kernel_radius = r.ceil() as i32;
    let sigma = r * 0.5;
    let two_sigma_sq = 2.0 * sigma * sigma;

    // Generate 1D Gaussian Kernel
    let mut kernel = Vec::with_capacity((kernel_radius * 2 + 1) as usize);
    let mut sum = 0.0_f32;
    for x in -kernel_radius..=kernel_radius {
        let g = (-((x * x) as f32) / two_sigma_sq).exp();
        kernel.push(g);
        sum += g;
    }
    for val in &mut kernel {
        *val /= sum;
    }

    let w = width as i32;
    let h = height as i32;
    // Convert to premultiplied alpha for correct blur behavior
    let total_px = (w * h) as usize;
    let mut premul = vec![0.0_f32; total_px * 4];
    for i in 0..total_px {
        let idx = i * 4;
        let a = pixels[idx + 3].to_scaled_f32() / mx;
        premul[idx] = pixels[idx].to_scaled_f32() * a;
        premul[idx + 1] = pixels[idx + 1].to_scaled_f32() * a;
        premul[idx + 2] = pixels[idx + 2].to_scaled_f32() * a;
        premul[idx + 3] = pixels[idx + 3].to_scaled_f32();
    }

    let mut temp_f = vec![0.0_f32; total_px * 4];

    // Horizontal Pass
    for y in 0..h {
        let row_start = (y * w * 4) as usize;
        for x in 0..w {
            let mut acc_r = 0.0_f32;
            let mut acc_g = 0.0_f32;
            let mut acc_b = 0.0_f32;
            let mut acc_a = 0.0_f32;

            for (k_idx, kx) in (-kernel_radius..=kernel_radius).enumerate() {
                let sample_x = (x + kx).clamp(0, w - 1);
                let idx = row_start + (sample_x * 4) as usize;
                let weight = kernel[k_idx];

                acc_r += premul[idx] * weight;
                acc_g += premul[idx + 1] * weight;
                acc_b += premul[idx + 2] * weight;
                acc_a += premul[idx + 3] * weight;
            }

            let dst_idx = row_start + (x * 4) as usize;
            temp_f[dst_idx] = acc_r;
            temp_f[dst_idx + 1] = acc_g;
            temp_f[dst_idx + 2] = acc_b;
            temp_f[dst_idx + 3] = acc_a;
        }
    }

    // Vertical Pass + unpremultiply
    for x in 0..w {
        for y in 0..h {
            if let Some(mask) = selection {
                if !mask.is_selected(x as u32, y as u32) {
                    continue;
                }
            }

            let mut acc_r = 0.0_f32;
            let mut acc_g = 0.0_f32;
            let mut acc_b = 0.0_f32;
            let mut acc_a = 0.0_f32;

            for (k_idx, ky) in (-kernel_radius..=kernel_radius).enumerate() {
                let sample_y = (y + ky).clamp(0, h - 1);
                let idx = ((sample_y * w + x) * 4) as usize;
                let weight = kernel[k_idx];

                acc_r += temp_f[idx] * weight;
                acc_g += temp_f[idx + 1] * weight;
                acc_b += temp_f[idx + 2] * weight;
                acc_a += temp_f[idx + 3] * weight;
            }

            // Unpremultiply alpha
            let out_a = acc_a.clamp(0.0, mx);
            let dst_idx = ((y * w + x) * 4) as usize;
            if out_a > 0.5 {
                let inv_a = mx / out_a;
                pixels[dst_idx] = C::from_scaled_f32((acc_r * inv_a).clamp(0.0, mx));
                pixels[dst_idx + 1] = C::from_scaled_f32((acc_g * inv_a).clamp(0.0, mx));
                pixels[dst_idx + 2] = C::from_scaled_f32((acc_b * inv_a).clamp(0.0, mx));
            } else {
                pixels[dst_idx] = C::ZERO;
                pixels[dst_idx + 1] = C::ZERO;
                pixels[dst_idx + 2] = C::ZERO;
            }
            pixels[dst_idx + 3] = C::from_scaled_f32(out_a);
        }
    }
}

/// Sharpens image using a 3x3 convolution kernel
pub fn filter_sharpen<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    amount: f32, // 0.1 to 3.0
    selection: Option<&SelectionMask>,
) {
    if amount <= 0.01 || width < 3 || height < 3 {
        return;
    }

    let mx = max_scaled::<C>();
    let w = width as i32;
    let h = height as i32;
    let src = pixels.to_vec();

    let a = amount.clamp(0.1, 3.0);
    // Sharpen kernel with dynamic strength
    let center = 1.0 + 4.0 * a;
    let neighbor = -a;

    for y in 0..h {
        for x in 0..w {
            if let Some(mask) = selection {
                if !mask.is_selected(x as u32, y as u32) {
                    continue;
                }
            }

            let idx = ((y * w + x) * 4) as usize;
            if src[idx + 3].to_scaled_f32() == 0.0 {
                continue;
            }

            for c in 0..3 {
                let mut acc = 0.0_f32;
                let c_idx = idx + c;

                // Center
                acc += src[c_idx].to_scaled_f32() * center;

                // 4-neighbors. `saturating_sub` on a signed integer floors at `i32::MIN`, not
                // at zero, so it produced a negative index and panicked on the top row and the
                // left column; `max(0)` is the clamp this always meant.
                let top = (((y - 1).max(0) * w + x) * 4) as usize + c;
                let bot = (((y + 1).min(h - 1) * w + x) * 4) as usize + c;
                let left = ((y * w + (x - 1).max(0)) * 4) as usize + c;
                let right = ((y * w + (x + 1).min(w - 1)) * 4) as usize + c;

                acc += src[top].to_scaled_f32() * neighbor;
                acc += src[bot].to_scaled_f32() * neighbor;
                acc += src[left].to_scaled_f32() * neighbor;
                acc += src[right].to_scaled_f32() * neighbor;

                pixels[c_idx] = C::from_scaled_f32(acc.clamp(0.0, mx));
            }
        }
    }
}

/// Unsharp Mask: enhances high-frequency contrast edges with radius and threshold gating
pub fn filter_unsharp_mask<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    radius: f32,    // 1.0 to 10.0
    amount: f32,    // 0.1 to 3.0
    threshold: f32, // 0.0 to 30.0
    selection: Option<&SelectionMask>,
) {
    let mx = max_scaled::<C>();
    let threshold = per_255::<C>(threshold);

    let mut blurred = pixels.to_vec();
    filter_gaussian_blur(&mut blurred, width, height, radius, selection);

    let total = (width * height) as usize;
    for i in 0..total {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        if pixels[idx + 3].to_scaled_f32() == 0.0 {
            continue;
        }

        for c in 0..3 {
            let orig = pixels[idx + c].to_scaled_f32();
            let blur = blurred[idx + c].to_scaled_f32();
            let diff = orig - blur;

            if diff.abs() >= threshold {
                let sharpened = orig + diff * amount;
                pixels[idx + c] = C::from_scaled_f32(sharpened.clamp(0.0, mx));
            }
        }
    }
}

/// Procedural Film Grain & Analog Noise Generator
pub fn filter_film_grain<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    intensity: f32, // 0.0 to 1.0
    is_colored: bool,
    selection: Option<&SelectionMask>,
) {
    if intensity <= 0.001 {
        return;
    }

    let mx = max_scaled::<C>();
    let total = (width * height) as usize;
    let max_noise = per_255::<C>(intensity * 75.0);

    // Fast Xorshift PRNG for authentic film grain
    let mut seed = 0x193a52f9u32;
    let mut next_rand = || -> f32 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        ((seed & 0xFFFF) as f32 / 32768.0) - 1.0 // -1.0 to +1.0
    };

    for i in 0..total {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        if pixels[idx + 3].to_scaled_f32() == 0.0 {
            continue;
        }

        // Truncating, as it always was.
        if is_colored {
            for c in 0..3 {
                let n = next_rand() * max_noise;
                pixels[idx + c] =
                    C::from_scaled_f32_trunc((pixels[idx + c].to_scaled_f32() + n).clamp(0.0, mx));
            }
        } else {
            let n = next_rand() * max_noise;
            for c in 0..3 {
                pixels[idx + c] =
                    C::from_scaled_f32_trunc((pixels[idx + c].to_scaled_f32() + n).clamp(0.0, mx));
            }
        }
    }
}

/// Vignette Filter: Darkens edges radiating outward from image center
pub fn filter_vignette<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    radius: f32,   // 0.1 to 1.5
    softness: f32, // 0.1 to 1.0
    darkness: f32, // 0.0 to 1.0
    selection: Option<&SelectionMask>,
) {
    let mx = max_scaled::<C>();
    let cx = width as f32 * 0.5;
    let cy = height as f32 * 0.5;
    let max_dist = (cx * cx + cy * cy).sqrt();

    let r_inner = radius * max_dist * (1.0 - softness * 0.6);
    let r_outer = radius * max_dist;

    let total = (width * height) as usize;
    for i in 0..total {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        if pixels[idx + 3].to_scaled_f32() == 0.0 {
            continue;
        }

        let dx = x as f32 - cx;
        let dy = y as f32 - cy;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist > r_inner {
            let factor = ((dist - r_inner) / (r_outer - r_inner).max(1.0)).clamp(0.0, 1.0);
            let shade = 1.0 - factor * darkness;

            // Truncating, as it always was.
            pixels[idx] = C::from_scaled_f32_trunc((pixels[idx].to_scaled_f32() * shade).clamp(0.0, mx));
            pixels[idx + 1] = C::from_scaled_f32_trunc((pixels[idx + 1].to_scaled_f32() * shade).clamp(0.0, mx));
            pixels[idx + 2] = C::from_scaled_f32_trunc((pixels[idx + 2].to_scaled_f32() * shade).clamp(0.0, mx));
        }
    }
}

/// Chromatic Aberration / Lens Prism Split (Red & Blue displacement)
pub fn filter_chromatic_aberration<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    shift_px: f32, // 1.0 to 20.0 px
    angle_deg: f32,
    selection: Option<&SelectionMask>,
) {
    if shift_px <= 0.2 || width == 0 || height == 0 {
        return;
    }

    let rad = angle_deg.to_radians();
    let dx = (rad.cos() * shift_px).round() as i32;
    let dy = (rad.sin() * shift_px).round() as i32;

    let src = pixels.to_vec();
    let w = width as i32;
    let h = height as i32;

    for y in 0..h {
        for x in 0..w {
            if let Some(mask) = selection {
                if !mask.is_selected(x as u32, y as u32) {
                    continue;
                }
            }

            let idx = ((y * w + x) * 4) as usize;

            // Red shifted in +direction (sample from -offset to move channel positively)
            let rx = (x - dx).clamp(0, w - 1);
            let ry = (y - dy).clamp(0, h - 1);
            let r_idx = ((ry * w + rx) * 4) as usize;

            // Blue shifted in -direction (sample from +offset to move channel negatively)
            let bx = (x + dx).clamp(0, w - 1);
            let by = (y + dy).clamp(0, h - 1);
            let b_idx = ((by * w + bx) * 4) as usize;

            pixels[idx] = src[r_idx];
            // Green remains centered
            pixels[idx + 2] = src[b_idx + 2];
        }
    }
}

/// Levels: remaps the black/white input range and applies a midtone gamma.
///
/// `black` and `white` are normalized input levels (0.0..=1.0), `gamma` is the
/// midtone exponent (1.0 = neutral, <1.0 brightens, >1.0 darkens).
pub fn filter_levels<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    black: f32,
    white: f32,
    gamma: f32,
    selection: Option<&SelectionMask>,
) {
    let mx = max_scaled::<C>();
    let black = black.clamp(0.0, 0.99);
    let white = white.clamp(black + 0.01, 1.0);
    let gamma = gamma.clamp(0.05, 10.0);
    let range = white - black;
    let inv_gamma = 1.0 / gamma;
    let total_pixels = (width * height) as usize;

    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let px = read_scaled(pixels, idx);
        if px[3] == 0.0 {
            continue;
        }

        for c in 0..3 {
            let normalized = (px[c] / mx - black) / range;
            let shaped = normalized.clamp(0.0, 1.0).powf(inv_gamma);
            pixels[idx + c] = C::from_scaled_f32((shaped * mx).round().clamp(0.0, mx));
        }
    }
}

/// Colorize: replaces chroma with a single hue/saturation while preserving luminance.
pub fn filter_colorize<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    hue: f32,
    saturation: f32,
    selection: Option<&SelectionMask>,
) {
    let mx = max_scaled::<C>();
    let saturation = saturation.clamp(0.0, 1.0);
    let total_pixels = (width * height) as usize;

    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let px = read_scaled(pixels, idx);
        if px[3] == 0.0 {
            continue;
        }

        let lum = (0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2]) / mx;
        let rgb = hsl_to_rgb_f32(hue, saturation, lum);
        pixels[idx] = C::from_scaled_f32(rgb[0] * mx);
        pixels[idx + 1] = C::from_scaled_f32(rgb[1] * mx);
        pixels[idx + 2] = C::from_scaled_f32(rgb[2] * mx);
    }
}

/// Pixelate: averages each `block`x`block` cell and writes the flat colour back.
pub fn filter_pixelate<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    block: u32,
    selection: Option<&SelectionMask>,
) {
    let block = block.clamp(2, 256);
    if width == 0 || height == 0 {
        return;
    }
    let src = pixels.to_vec();
    let bw = width as usize;
    let bh = height as usize;

    for by in (0..bh).step_by(block as usize) {
        for bx in (0..bw).step_by(block as usize) {
            let x1 = (bx + block as usize).min(bw);
            let y1 = (by + block as usize).min(bh);
            // u64 so a 16-bit cell (65535 x 65536 samples) cannot overflow.
            let mut acc = [0u64; 4];
            let mut count = 0u64;

            for y in by..y1 {
                for x in bx..x1 {
                    let idx = (y * bw + x) * 4;
                    for c in 0..4 {
                        acc[c] += src[idx + c].to_scaled_f32() as u64;
                    }
                    count += 1;
                }
            }
            if count == 0 {
                continue;
            }

            // Integer division: truncating, as it always was.
            let avg = [
                C::from_scaled_u64(acc[0] / count),
                C::from_scaled_u64(acc[1] / count),
                C::from_scaled_u64(acc[2] / count),
                C::from_scaled_u64(acc[3] / count),
            ];

            for y in by..y1 {
                for x in bx..x1 {
                    if let Some(mask) = selection {
                        if !mask.is_selected(x as u32, y as u32) {
                            continue;
                        }
                    }
                    let idx = (y * bw + x) * 4;
                    pixels[idx..idx + 4].copy_from_slice(&avg);
                }
            }
        }
    }
}

/// Motion blur: box-averages samples along a line of `distance` pixels at `angle_deg`.
pub fn filter_motion_blur<C: Channel>(
    pixels: &mut [C],
    width: u32,
    height: u32,
    distance: u32,
    angle_deg: f32,
    selection: Option<&SelectionMask>,
) {
    if distance == 0 || width == 0 || height == 0 {
        return;
    }
    let dist = distance.clamp(1, 128) as i32;
    let src = pixels.to_vec();
    let w = width as i32;
    let h = height as i32;
    let rad = angle_deg.to_radians();
    let dx = rad.cos();
    let dy = rad.sin();
    let samples = dist * 2 + 1;

    for y in 0..h {
        for x in 0..w {
            if let Some(mask) = selection {
                if !mask.is_selected(x as u32, y as u32) {
                    continue;
                }
            }
            let dst_idx = ((y * w + x) * 4) as usize;
            if src[dst_idx + 3].to_scaled_f32() == 0.0 {
                continue;
            }

            let mut acc = [0u64; 4];
            for s in -dist..=dist {
                let sx = (x + (dx * s as f32).round() as i32).clamp(0, w - 1);
                let sy = (y + (dy * s as f32).round() as i32).clamp(0, h - 1);
                let s_idx = ((sy * w + sx) * 4) as usize;
                for c in 0..4 {
                    acc[c] += src[s_idx + c].to_scaled_f32() as u64;
                }
            }
            for c in 0..4 {
                // Integer division: truncating, as it always was.
                pixels[dst_idx + c] = C::from_scaled_u64(acc[c] / samples as u64);
            }
        }
    }
}

// ── Layer entry points ─────────────────────────────────────────────────────────────────────

/// Generates a layer-level entry point per filter: it dispatches **once** on the buffer and
/// calls the generic body, so no call site writes its own `match` and no pixel loop sees a
/// format.
macro_rules! layer_entry_points {
    ($(
        $(#[$meta:meta])*
        $entry:ident ( $target:ident ) ( $( $arg:ident : $ty:ty ),* $(,)? );
    )*) => {
        $(
            $(#[$meta])*
            #[allow(clippy::too_many_arguments)]
            pub fn $entry(
                layer: &mut Layer,
                $($arg: $ty,)*
                selection: Option<&SelectionMask>,
            ) {
                let (width, height) = (layer.width, layer.height);
                match layer.buffer_mut() {
                    LayerBuffer::Srgb8(pixels) => {
                        $target(pixels, width, height, $($arg,)* selection)
                    }
                    LayerBuffer::Srgb16(pixels) => {
                        $target(pixels, width, height, $($arg,)* selection)
                    }
                }
            }
        )*
    };
}

layer_entry_points! {
    /// Applies invert to a layer, whichever storage format it holds.
    filter_invert_layer(filter_invert)();
    /// Applies posterize to a layer.
    filter_posterize_layer(filter_posterize)(levels: u32);
    /// Applies grayscale to a layer.
    filter_grayscale_layer(filter_grayscale)();
    /// Applies sepia to a layer.
    filter_sepia_layer(filter_sepia)();
    /// Applies a binary threshold to a layer.
    filter_threshold_layer(filter_threshold)(threshold: u8);
    /// Applies a Gaussian blur to a layer.
    filter_gaussian_blur_layer(filter_gaussian_blur)(radius: f32);
    /// Applies sharpen to a layer.
    filter_sharpen_layer(filter_sharpen)(amount: f32);
    /// Applies an unsharp mask to a layer.
    filter_unsharp_mask_layer(filter_unsharp_mask)(radius: f32, amount: f32, threshold: f32);
    /// Applies film grain to a layer.
    filter_film_grain_layer(filter_film_grain)(intensity: f32, is_colored: bool);
    /// Applies a vignette to a layer.
    filter_vignette_layer(filter_vignette)(radius: f32, softness: f32, darkness: f32);
    /// Applies chromatic aberration to a layer.
    filter_chromatic_aberration_layer(filter_chromatic_aberration)(shift_px: f32, angle_deg: f32);
    /// Applies a levels remap to a layer.
    filter_levels_layer(filter_levels)(black: f32, white: f32, gamma: f32);
    /// Applies colorize to a layer.
    filter_colorize_layer(filter_colorize)(hue: f32, saturation: f32);
    /// Applies pixelate to a layer.
    filter_pixelate_layer(filter_pixelate)(block: u32);
    /// Applies motion blur to a layer.
    filter_motion_blur_layer(filter_motion_blur)(distance: u32, angle_deg: f32);
    /// Applies the HSL adjustment to a layer.
    adjust_hsl_layer(adjust_hsl)(hue_shift: f32, saturation_scale: f32, lightness_shift: f32);
    /// Applies the brightness/contrast adjustment to a layer.
    adjust_brightness_contrast_layer(adjust_brightness_contrast)(brightness: f32, contrast: f32);
    /// Applies the colour-balance adjustment to a layer.
    adjust_color_balance_layer(adjust_color_balance)(cyan_red: f32, magenta_green: f32, yellow_blue: f32);
}
