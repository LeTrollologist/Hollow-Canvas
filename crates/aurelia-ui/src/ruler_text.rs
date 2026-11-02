//! Ruler numerals, laid out with real type.
//!
//! The ticks and bands are rasterised by `aurelia-render` (crisp 1 px lines), but
//! the numbers are drawn by egui on top of the frame so they are antialiased and
//! use the same face as the rest of the chrome. This module owns only the pure
//! layout decision of *which* ticks get a number and where it starts.

use aurelia_render::software::{RulerTick, TickLevel};

/// Gap between the tick and the start of its number, in pixels.
pub const LABEL_INSET: f32 = 4.0;
/// Minimum clear space between two numbers, in pixels.
pub const LABEL_GAP: f32 = 7.0;

/// One number that survived the layout pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelPlacement {
    /// Canvas coordinate the number describes.
    pub value: f32,
    /// Tick position along the strip, in pixels.
    pub screen: f32,
    /// Extent of the rendered number along the strip, in pixels.
    pub extent: f32,
}

/// Chooses labelled ticks for a strip `[0, strip_hi)`.
///
/// `measure` returns the rendered length of a number along the strip, so the
/// layout uses real glyph metrics instead of guessing a fixed width.
pub fn place_labels(
    ticks: &[RulerTick],
    doc_len: f32,
    strip_hi: f32,
    measure: impl Fn(f32) -> f32,
) -> Vec<LabelPlacement> {
    let mut out: Vec<LabelPlacement> = Vec::new();
    let mut next_free = f32::NEG_INFINITY;

    for tick in ticks {
        if tick.level != TickLevel::Major || tick.value > doc_len + 0.5 {
            continue;
        }
        let start = tick.screen as f32 + LABEL_INSET;
        let extent = measure(tick.value);
        if !extent.is_finite() || extent <= 0.0 {
            continue;
        }
        if start + extent > strip_hi || start < next_free {
            continue;
        }
        out.push(LabelPlacement {
            value: tick.value,
            screen: tick.screen as f32,
            extent,
        });
        next_free = start + extent + LABEL_GAP;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticks(values: &[(f32, f32, TickLevel)]) -> Vec<RulerTick> {
        values
            .iter()
            .map(|&(value, screen, level)| RulerTick {
                value,
                screen: screen as isize,
                level,
            })
            .collect()
    }

    fn major_grid(count: usize, step: f32) -> Vec<RulerTick> {
        (0..=count)
            .map(|i| RulerTick {
                value: i as f32 * step,
                screen: (i as f32 * step * 0.5) as isize,
                level: TickLevel::Major,
            })
            .collect()
    }

    #[test]
    fn labels_never_overlap_and_keep_the_declared_gap() {
        let ticks = major_grid(40, 100.0);
        let labels = place_labels(&ticks, 4000.0, 4000.0, |_| 24.0);
        assert!(labels.len() > 1);
        for pair in labels.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let a_end = a.screen + LABEL_INSET + a.extent;
            assert!(
                b.screen + LABEL_INSET >= a_end + LABEL_GAP - 0.01,
                "labels {} and {} are {} px apart",
                a.value,
                b.value,
                (b.screen + LABEL_INSET) - a_end
            );
        }
    }

    #[test]
    fn labels_stay_inside_the_canvas_and_the_strip() {
        let ticks = major_grid(40, 100.0);
        for (doc_len, strip_hi) in [(4000.0_f32, 4000.0_f32), (1200.0, 3000.0), (4000.0, 700.0)] {
            let labels = place_labels(&ticks, doc_len, strip_hi, |_| 30.0);
            for label in &labels {
                assert!(
                    label.value >= 0.0 && label.value <= doc_len + 0.5,
                    "label {} outside a {doc_len} px canvas",
                    label.value
                );
                assert!(
                    label.screen + LABEL_INSET + label.extent <= strip_hi + 0.01,
                    "label {} runs past the {strip_hi} px strip",
                    label.value
                );
            }
        }
    }

    #[test]
    fn a_wide_number_pushes_the_next_label_out() {
        // "1000" measured as 40 px must delay the follower more than "40" at 10 px.
        let ticks = ticks(&[(0.0, 0.0, TickLevel::Major), (10.0, 20.0, TickLevel::Major)]);
        let narrow = place_labels(&ticks, 100.0, 400.0, |v| if v == 0.0 { 10.0 } else { 10.0 });
        assert_eq!(narrow.len(), 2);

        let wide = place_labels(&ticks, 100.0, 400.0, |v| if v == 0.0 { 40.0 } else { 10.0 });
        // 0 + 4 + 40 + 7 = 51 > 24, so the 10 label at screen 20 cannot be drawn.
        assert_eq!(wide.len(), 1);
        assert_eq!(wide[0].value, 0.0);
    }

    #[test]
    fn minor_ticks_never_receive_labels() {
        let ticks = ticks(&[
            (0.0, 0.0, TickLevel::Major),
            (50.0, 40.0, TickLevel::Mid),
            (100.0, 80.0, TickLevel::Minor),
            (200.0, 160.0, TickLevel::Major),
        ]);
        let labels = place_labels(&ticks, 400.0, 800.0, |_| 20.0);
        assert_eq!(labels.len(), 2);
        assert!(labels.iter().all(|l| l.value == 0.0 || l.value == 200.0));
    }

    #[test]
    fn degenerate_measurements_are_rejected() {
        let ticks = major_grid(4, 100.0);
        assert!(place_labels(&ticks, 400.0, 800.0, |_| 0.0).is_empty());
        assert!(place_labels(&ticks, 400.0, 800.0, |_| f32::NAN).is_empty());
        assert!(place_labels(&[], 400.0, 800.0, |_| 20.0).is_empty());
    }
}
