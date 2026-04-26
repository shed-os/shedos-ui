//! Easing functions for effect animations.
//!
//! Each function takes `t: 0.0..=1.0` (normalized progress) and
//! returns an eased value in the same range. Mirrors the standard
//! Robert Penner easing set; effects pick one based on the feel
//! they want (ease-out for "settling", ease-in for "accelerating").

#[inline]
pub fn linear(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

#[inline]
pub fn ease_in_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

#[inline]
pub fn ease_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(2)
}

#[inline]
pub fn ease_in_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

#[inline]
pub fn ease_in_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

#[inline]
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

#[inline]
pub fn ease_out_quart(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(4)
}

#[inline]
pub fn ease_out_expo(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        1.0
    } else {
        1.0 - 2.0_f32.powf(-10.0 * t)
    }
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_endpoints() {
        assert_eq!(linear(0.0), 0.0);
        assert_eq!(linear(1.0), 1.0);
    }

    #[test]
    fn easings_clamp() {
        // Out-of-range inputs stay in [0, 1].
        for f in [
            ease_in_quad,
            ease_out_quad,
            ease_in_out_quad,
            ease_in_cubic,
            ease_out_cubic,
            ease_out_quart,
            ease_out_expo,
            linear,
        ] {
            assert!((0.0..=1.0).contains(&f(-1.0)), "f(-1.0) escapes [0,1]");
            assert!((0.0..=1.0).contains(&f(2.0)), "f(2.0) escapes [0,1]");
        }
    }

    #[test]
    fn easings_pass_through_endpoints() {
        for f in [
            ease_in_quad,
            ease_out_quad,
            ease_in_out_quad,
            ease_in_cubic,
            ease_out_cubic,
            ease_out_quart,
            ease_out_expo,
            linear,
        ] {
            assert!((f(0.0) - 0.0).abs() < 1e-4, "f(0.0) = {} not 0", f(0.0));
            assert!((f(1.0) - 1.0).abs() < 1e-4, "f(1.0) = {} not 1", f(1.0));
        }
    }

    #[test]
    fn ease_out_settles_above_linear() {
        // For ease-out curves, midpoint should be >= 0.5 (early progress).
        assert!(ease_out_quad(0.5) >= 0.5);
        assert!(ease_out_cubic(0.5) >= 0.5);
        assert!(ease_out_quart(0.5) >= 0.5);
    }

    #[test]
    fn lerp_basic() {
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-6);
        assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);
    }
}
