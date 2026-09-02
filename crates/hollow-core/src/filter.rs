use crate::selection::SelectionMask;

/// Converts RGB (0..255) to HSL (H: 0..360, S: 0..1, L: 0..1)
pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;

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
    if s.abs() < 1e-5 {
        let val = (l * 255.0).clamp(0.0, 255.0).round() as u8;
        return (val, val, val);
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

    let r = (hue_to_rgb(hk + 1.0 / 3.0) * 255.0).clamp(0.0, 255.0).round() as u8;
    let g = (hue_to_rgb(hk) * 255.0).clamp(0.0, 255.0).round() as u8;
    let b = (hue_to_rgb(hk - 1.0 / 3.0) * 255.0).clamp(0.0, 255.0).round() as u8;

    (r, g, b)
}

/// Adjusts Hue, Saturation, and Lightness of pixels in-place
pub fn adjust_hsl(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    hue_shift: f32,        // -180.0 to +180.0 degrees
    saturation_scale: f32, // 0.0 to 3.0 (1.0 = normal)
    lightness_shift: f32,  // -1.0 to +1.0 (0.0 = normal)
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
        let r = pixels[idx];
        let g = pixels[idx + 1];
        let b = pixels[idx + 2];
        let a = pixels[idx + 3];

        if a == 0 {
            continue;
        }

        let (h, s, l) = rgb_to_hsl(r, g, b);
        let new_h = (h + hue_shift + 360.0) % 360.0;
        let new_s = (s * saturation_scale).clamp(0.0, 1.0);
        let new_l = (l + lightness_shift).clamp(0.0, 1.0);

        let (nr, ng, nb) = hsl_to_rgb(new_h, new_s, new_l);
        pixels[idx] = nr;
        pixels[idx + 1] = ng;
        pixels[idx + 2] = nb;
    }
}

/// Adjusts Brightness (-100..=100) and Contrast (-100..=100)
pub fn adjust_brightness_contrast(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    brightness: f32, // -100.0 to +100.0
    contrast: f32,   // -100.0 to +100.0
    selection: Option<&SelectionMask>,
) {
    // Contrast factor formula
    let c_factor = (259.0 * (contrast + 255.0)) / (255.0 * (259.0 - contrast));
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
        let a = pixels[idx + 3];
        if a == 0 {
            continue;
        }

        for c in 0..3 {
            let orig = pixels[idx + c] as f32;
            let with_brightness = orig + brightness * 2.55;
            let with_contrast = c_factor * (with_brightness - 128.0) + 128.0;
            pixels[idx + c] = with_contrast.clamp(0.0, 255.0).round() as u8;
        }
    }
}

/// Color Balance: adjusts Red, Green, and Blue bias for shadows, midtones, and highlights
pub fn adjust_color_balance(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    cyan_red: f32,      // -100.0 (Cyan) to +100.0 (Red)
    magenta_green: f32, // -100.0 (Magenta) to +100.0 (Green)
    yellow_blue: f32,   // -100.0 (Yellow) to +100.0 (Blue)
    selection: Option<&SelectionMask>,
) {
    let total_pixels = (width * height) as usize;
    let r_bias = cyan_red * 1.28;
    let g_bias = magenta_green * 1.28;
    let b_bias = yellow_blue * 1.28;

    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let a = pixels[idx + 3];
        if a == 0 {
            continue;
        }

        let r = (pixels[idx] as f32 + r_bias).clamp(0.0, 255.0).round() as u8;
        let g = (pixels[idx + 1] as f32 + g_bias).clamp(0.0, 255.0).round() as u8;
        let b = (pixels[idx + 2] as f32 + b_bias).clamp(0.0, 255.0).round() as u8;

        pixels[idx] = r;
        pixels[idx + 1] = g;
        pixels[idx + 2] = b;
    }
}

/// Inverts RGB channels
pub fn filter_invert(pixels: &mut [u8], width: u32, height: u32, selection: Option<&SelectionMask>) {
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
        let a = pixels[idx + 3];
        if a == 0 {
            continue;
        }

        pixels[idx] = 255 - pixels[idx];
        pixels[idx + 1] = 255 - pixels[idx + 1];
        pixels[idx + 2] = 255 - pixels[idx + 2];
    }
}

/// Posterizes color channels to discrete tonal levels (2 to 32)
pub fn filter_posterize(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    levels: u32,
    selection: Option<&SelectionMask>,
) {
    let levels = levels.clamp(2, 32) as f32;
    let step = 255.0 / (levels - 1.0);
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
        let a = pixels[idx + 3];
        if a == 0 {
            continue;
        }

        for c in 0..3 {
            let val = pixels[idx + c] as f32;
            let quantized = ((val / step).round() * step).clamp(0.0, 255.0) as u8;
            pixels[idx + c] = quantized;
        }
    }
}

/// Converts layer to perceptual grayscale
pub fn filter_grayscale(pixels: &mut [u8], width: u32, height: u32, selection: Option<&SelectionMask>) {
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
        let a = pixels[idx + 3];
        if a == 0 {
            continue;
        }

        let r = pixels[idx] as f32;
        let g = pixels[idx + 1] as f32;
        let b = pixels[idx + 2] as f32;
        let lum = (0.2126 * r + 0.7152 * g + 0.0722 * b).round() as u8;

        pixels[idx] = lum;
        pixels[idx + 1] = lum;
        pixels[idx + 2] = lum;
    }
}

/// Applies vintage sepia photographic tone
pub fn filter_sepia(pixels: &mut [u8], width: u32, height: u32, selection: Option<&SelectionMask>) {
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
        let a = pixels[idx + 3];
        if a == 0 {
            continue;
        }

        let r = pixels[idx] as f32;
        let g = pixels[idx + 1] as f32;
        let b = pixels[idx + 2] as f32;

        let tr = (0.393 * r + 0.769 * g + 0.189 * b).clamp(0.0, 255.0) as u8;
        let tg = (0.349 * r + 0.686 * g + 0.168 * b).clamp(0.0, 255.0) as u8;
        let tb = (0.272 * r + 0.534 * g + 0.131 * b).clamp(0.0, 255.0) as u8;

        pixels[idx] = tr;
        pixels[idx + 1] = tg;
        pixels[idx + 2] = tb;
    }
}

/// Binary Black & White Thresholding
pub fn filter_threshold(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    threshold: u8,
    selection: Option<&SelectionMask>,
) {
    let total_pixels = (width * height) as usize;
    let thresh_f = threshold as f32;

    for i in 0..total_pixels {
        let x = (i as u32) % width;
        let y = (i as u32) / width;

        if let Some(mask) = selection {
            if !mask.is_selected(x, y) {
                continue;
            }
        }

        let idx = i * 4;
        let a = pixels[idx + 3];
        if a == 0 {
            continue;
        }

        let r = pixels[idx] as f32;
        let g = pixels[idx + 1] as f32;
        let b = pixels[idx + 2] as f32;
        let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;

        let val = if lum >= thresh_f { 255 } else { 0 };
        pixels[idx] = val;
        pixels[idx + 1] = val;
        pixels[idx + 2] = val;
    }
}

/// High-performance Separable 1D Gaussian Blur with configurable radius (1..=48)
pub fn filter_gaussian_blur(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    radius: f32,
    selection: Option<&SelectionMask>,
) {
    if radius <= 0.2 || width == 0 || height == 0 {
        return;
    }

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
        let a = pixels[idx + 3] as f32 / 255.0;
        premul[idx] = pixels[idx] as f32 * a;
        premul[idx + 1] = pixels[idx + 1] as f32 * a;
        premul[idx + 2] = pixels[idx + 2] as f32 * a;
        premul[idx + 3] = pixels[idx + 3] as f32;
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
            let out_a = acc_a.clamp(0.0, 255.0);
            let dst_idx = ((y * w + x) * 4) as usize;
            if out_a > 0.5 {
                let inv_a = 255.0 / out_a;
                pixels[dst_idx] = (acc_r * inv_a).clamp(0.0, 255.0).round() as u8;
                pixels[dst_idx + 1] = (acc_g * inv_a).clamp(0.0, 255.0).round() as u8;
                pixels[dst_idx + 2] = (acc_b * inv_a).clamp(0.0, 255.0).round() as u8;
            } else {
                pixels[dst_idx] = 0;
                pixels[dst_idx + 1] = 0;
                pixels[dst_idx + 2] = 0;
            }
            pixels[dst_idx + 3] = out_a.round() as u8;
        }
    }
}

/// Sharpens image using a 3x3 convolution kernel
pub fn filter_sharpen(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    amount: f32, // 0.1 to 3.0
    selection: Option<&SelectionMask>,
) {
    if amount <= 0.01 || width < 3 || height < 3 {
        return;
    }

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
            if src[idx + 3] == 0 {
                continue;
            }

            for c in 0..3 {
                let mut acc = 0.0_f32;
                let c_idx = idx + c;

                // Center
                acc += src[c_idx] as f32 * center;

                // 4-neighbors
                let top = ((y.saturating_sub(1) * w + x) * 4) as usize + c;
                let bot = (((y + 1).min(h - 1) * w + x) * 4) as usize + c;
                let left = ((y * w + x.saturating_sub(1)) * 4) as usize + c;
                let right = ((y * w + (x + 1).min(w - 1)) * 4) as usize + c;

                acc += src[top] as f32 * neighbor;
                acc += src[bot] as f32 * neighbor;
                acc += src[left] as f32 * neighbor;
                acc += src[right] as f32 * neighbor;

                pixels[c_idx] = acc.clamp(0.0, 255.0).round() as u8;
            }
        }
    }
}

/// Unsharp Mask: enhances high-frequency contrast edges with radius and threshold gating
pub fn filter_unsharp_mask(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    radius: f32,    // 1.0 to 10.0
    amount: f32,    // 0.1 to 3.0
    threshold: f32, // 0.0 to 30.0
    selection: Option<&SelectionMask>,
) {
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
        if pixels[idx + 3] == 0 {
            continue;
        }

        for c in 0..3 {
            let orig = pixels[idx + c] as f32;
            let blur = blurred[idx + c] as f32;
            let diff = orig - blur;

            if diff.abs() >= threshold {
                let sharpened = orig + diff * amount;
                pixels[idx + c] = sharpened.clamp(0.0, 255.0).round() as u8;
            }
        }
    }
}

/// Procedural Film Grain & Analog Noise Generator
pub fn filter_film_grain(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    intensity: f32, // 0.0 to 1.0
    is_colored: bool,
    selection: Option<&SelectionMask>,
) {
    if intensity <= 0.001 {
        return;
    }

    let total = (width * height) as usize;
    let max_noise = intensity * 75.0;

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
        let a = pixels[idx + 3];
        if a == 0 {
            continue;
        }

        if is_colored {
            for c in 0..3 {
                let n = next_rand() * max_noise;
                let val = (pixels[idx + c] as f32 + n).clamp(0.0, 255.0) as u8;
                pixels[idx + c] = val;
            }
        } else {
            let n = next_rand() * max_noise;
            for c in 0..3 {
                let val = (pixels[idx + c] as f32 + n).clamp(0.0, 255.0) as u8;
                pixels[idx + c] = val;
            }
        }
    }
}

/// Vignette Filter: Darkens edges radiating outward from image center
pub fn filter_vignette(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    radius: f32,   // 0.1 to 1.5
    softness: f32, // 0.1 to 1.0
    darkness: f32, // 0.0 to 1.0
    selection: Option<&SelectionMask>,
) {
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
        let a = pixels[idx + 3];
        if a == 0 {
            continue;
        }

        let dx = x as f32 - cx;
        let dy = y as f32 - cy;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist > r_inner {
            let factor = ((dist - r_inner) / (r_outer - r_inner).max(1.0)).clamp(0.0, 1.0);
            let shade = 1.0 - factor * darkness;

            pixels[idx] = (pixels[idx] as f32 * shade).clamp(0.0, 255.0) as u8;
            pixels[idx + 1] = (pixels[idx + 1] as f32 * shade).clamp(0.0, 255.0) as u8;
            pixels[idx + 2] = (pixels[idx + 2] as f32 * shade).clamp(0.0, 255.0) as u8;
        }
    }
}

/// Chromatic Aberration / Lens Prism Split (Red & Blue displacement)
pub fn filter_chromatic_aberration(
    pixels: &mut [u8],
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
