//! Oklch colour maths adapted from
//! [culori](https://github.com/Evercoder/culori).
//!
//! Animated colours are interpolated in Oklch, not in sRGB, and are brought back into the sRGB
//! gamut by reducing chroma rather than clipping channels.
//! For example, mixing `#00ff00` with
//! `#ff00ff` in Oklch and clipping gives `rgb(255, 97, 0)`, while reducing chroma gives
//! `rgb(255, 157, 84)`, which is what the source actually paints.
//!
//! This module therefore reproduces, in order:
//!
//! 1. culori's sRGB transfer function and the Oklab matrices (Björn Ottosson's constants,
//!    spelled to full precision as in culori's `oklab/convertLrgbToOklab.js`).
//! 2. `interpolate([a, b], "oklch")` — linear on `l`, `c` and `h`, with the **shorter** hue
//!    arc (`fixupHueShorter`), `t` clamped to `[0, 1]`, and any channel that is missing at one
//!    end held constant at the other end's value.
//! 3. `clampChroma(color, "oklch", "rgb")` — a bisection on chroma with a resolution of
//!    `(0.4 - 0) / 2^13`, falling back to channel clipping when even `c = 0` is out of gamut.
//! 4. `formatRgb` — each channel clamped to `[0, 1]`, multiplied by 255 and rounded, so the
//!    string this produces is byte-for-byte the one the source hands to the browser.
//!
//! ## Why this is `f64`, and why the constants are written out in full
//!
//! For an exactly neutral colour such as `#141414` the Oklab `a` and `b` are the difference of
//! nearly equal matrix terms — cancellation noise around `1e-8`, not a meaningful chroma. culori
//! still assigns a hue from `atan2(b, a)`, so a grey's hue is *arbitrary*, and the shorter-arc
//! fixup carries that arbitrary value into the interpolation. Two implementations that agree to
//! six decimal places disagree completely about a grey's hue, and a quarter of the way between two
//! near-neutral colours that is worth a whole 8-bit step per channel.
//!
//! Matching culori here therefore means matching its arithmetic, not merely its algorithm: the
//! computations run in `f64` (JavaScript numbers) with the constants at full precision and the
//! same left-to-right accumulation order. The fixtures in `mix_oklch_fixtures.txt` were generated
//! by running culori 4.0.2 itself over 156 colour pairs — several of them neutral — and
//! [`tests::matches_culori`] requires every one to match exactly.
//!
//! Note that `rgba(...)` is a CSS string an SVG `fill` does not reliably accept. Components
//! should use [`mix_oklch`] (which yields an opaque [`Rgba`]) plus [`to_hex`], and express
//! partial opacity with the SVG `opacity`/`fill-opacity` attribute instead of [`with_alpha`].

/// A colour in sRGB, channels nominally in `0..=1` and possibly outside the gamut.
///
/// `alpha` is `None` for an opaque colour, mirroring culori, where an absent alpha and
/// `alpha == 1` both serialise as opaque `rgb(...)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    /// Red channel, `0..=1` in gamut.
    pub r: f64,
    /// Green channel, `0..=1` in gamut.
    pub g: f64,
    /// Blue channel, `0..=1` in gamut.
    pub b: f64,
    /// Alpha, `0..=1`; `None` means opaque.
    pub alpha: Option<f64>,
}

/// A colour in Oklch: perceptual lightness, chroma and hue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    /// Lightness, `0..=1`.
    pub l: f64,
    /// Chroma, nominally `0..=0.4`.
    pub c: f64,
    /// Hue in degrees. `None` for an achromatic colour, which is what culori produces when the
    /// computed chroma is exactly zero.
    pub h: Option<f64>,
    /// Alpha, `0..=1`; `None` means opaque.
    pub alpha: Option<f64>,
}

/// The upper bound of the Oklch chroma range culori declares, used for the gamut bisection.
const OKLCH_CHROMA_MAX: f64 = 0.4;

/// culori's `normalizeHue`: wrap into `[0, 360)`.
fn normalize_hue(hue: f64) -> f64 {
    let wrapped = hue % 360.0;
    if wrapped < 0.0 {
        wrapped + 360.0
    } else {
        wrapped
    }
}

/// culori's `fixup`: clamp to `[0, 1]` (treating NaN as 0) and round to 8 bits.
///
/// `Math.round` is half-away-from-zero **towards positive infinity**, so `(x + 0.5).floor()`
/// is the matching operation for the non-negative values here.
fn fixup(value: f64) -> u8 {
    let clamped = if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    };
    (clamped * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8
}

/// Round to two decimals and drop trailing zeros, like culori's `round(2)` formatter.
fn two_decimals(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let mut text = format!("{rounded:.2}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

/// sRGB → linear sRGB, with culori's sign-preserving transfer function.
///
/// culori writes the sign as `(Math.sign(c) || 1)`, which only differs from `signum` at zero —
/// and zero takes the linear branch, so the two agree everywhere this is called.
fn srgb_to_linear(c: f64) -> f64 {
    let abs = c.abs();
    if abs <= 0.040_45 {
        c / 12.92
    } else {
        c.signum() * ((abs + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear sRGB → sRGB, the inverse of [`srgb_to_linear`].
fn linear_to_srgb(c: f64) -> f64 {
    let abs = c.abs();
    if abs > 0.003_130_8 {
        c.signum() * (1.055 * abs.powf(1.0 / 2.4) - 0.055)
    } else {
        c * 12.92
    }
}

/// Linear sRGB → Oklab (culori's `convertLrgbToOklab`), constants and order verbatim.
fn linear_to_oklab(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let l = (0.412_221_469_470_763 * r + 0.536_332_537_261_734_8 * g + 0.051_445_993_267_502_2 * b)
        .cbrt();
    let m =
        (0.211_903_495_817_825_2 * r + 0.680_699_550_645_234_4 * g + 0.107_396_953_536_940_6 * b)
            .cbrt();
    let s =
        (0.088_302_459_190_056_4 * r + 0.281_718_839_136_121_5 * g + 0.629_978_701_673_822_2 * b)
            .cbrt();
    (
        0.210_454_268_309_314 * l + 0.793_617_774_702_305_4 * m - 0.004_072_043_011_619_3 * s,
        1.977_998_532_431_168_4 * l - 2.428_592_242_048_58 * m + 0.450_593_709_617_411 * s,
        0.025_904_042_465_547_8 * l + 0.782_771_712_457_529_6 * m - 0.808_675_754_923_077_4 * s,
    )
}

/// Oklab → linear sRGB (culori's `convertOklabToLrgb`), constants and order verbatim.
fn oklab_to_linear(l: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let p = (l + 0.396_337_777_376_174_9 * a + 0.215_803_757_309_913_6 * b).powi(3);
    let m = (l - 0.105_561_345_815_658_6 * a - 0.063_854_172_825_813_3 * b).powi(3);
    let s = (l - 0.089_484_177_529_811_9 * a - 1.291_485_548_019_409_2 * b).powi(3);
    (
        4.076_741_636_075_957 * p - 3.307_711_539_258_061_6 * m + 0.230_969_903_182_104_4 * s,
        -1.268_437_973_285_031_7 * p + 2.609_757_349_287_688_7 * m - 0.341_319_376_002_657_3 * s,
        -0.004_196_076_138_675_6 * p - 0.703_418_617_935_936_2 * m + 1.707_614_694_074_611_7 * s,
    )
}

/// sRGB → Oklch, exactly as culori's `rgb → lrgb → oklab → oklch` chain does it.
///
/// ## The neutral special case
///
/// culori's `convertRgbToOklab` zeroes the Oklab `a` and `b` channels when the sRGB input is
/// exactly neutral (`r === g === b`):
///
/// ```js
/// if (rgb.r === rgb.b && rgb.b === rgb.g) { res.a = res.b = 0; }
/// ```
///
/// Without it a grey's `a` and `b` are the cancellation noise of two nearly equal matrix terms —
/// around `1e-16` — which is not zero, so `atan2` hands the colour an arbitrary hue. The shorter
/// arc then carries that arbitrary hue into every interpolation, and mixing a grey with anything
/// swings through a colour nobody asked for. With it, a grey has no hue at all, so the hue comes
/// from the other endpoint and white-to-blue stays blue.
///
/// The hue is `None` when the chroma is exactly zero, which is how culori signals an achromatic
/// colour.
#[must_use]
pub fn rgb_to_oklch(color: Rgba) -> Oklch {
    let (l, mut a, mut b) = linear_to_oklab(
        srgb_to_linear(color.r),
        srgb_to_linear(color.g),
        srgb_to_linear(color.b),
    );
    if color.r == color.g && color.g == color.b {
        a = 0.0;
        b = 0.0;
    }
    let c = (a * a + b * b).sqrt();
    Oklch {
        l,
        c,
        h: if c != 0.0 {
            Some(normalize_hue(b.atan2(a).to_degrees()))
        } else {
            None
        },
        alpha: color.alpha,
    }
}

/// Oklch → sRGB, exactly as culori's `oklch → oklab → lrgb → rgb` chain does it.
#[must_use]
pub fn oklch_to_rgb(color: Oklch) -> Rgba {
    let h = color.h.unwrap_or(0.0);
    let a = if color.c != 0.0 {
        color.c * (h / 180.0 * std::f64::consts::PI).cos()
    } else {
        0.0
    };
    let b = if color.c != 0.0 {
        color.c * (h / 180.0 * std::f64::consts::PI).sin()
    } else {
        0.0
    };
    let (r, g, bl) = oklab_to_linear(color.l, a, b);
    Rgba {
        r: linear_to_srgb(r),
        g: linear_to_srgb(g),
        b: linear_to_srgb(bl),
        alpha: color.alpha,
    }
}

/// Whether every channel of an sRGB colour is inside `[0, 1]` (culori's `displayable`).
fn displayable(color: Rgba) -> bool {
    (0.0..=1.0).contains(&color.r)
        && (0.0..=1.0).contains(&color.g)
        && (0.0..=1.0).contains(&color.b)
}

/// Clip the channels of an sRGB colour into `[0, 1]` (culori's `fixup_rgb`).
fn clip_rgb(color: Rgba) -> Rgba {
    Rgba {
        r: color.r.clamp(0.0, 1.0),
        g: color.g.clamp(0.0, 1.0),
        b: color.b.clamp(0.0, 1.0),
        alpha: color.alpha,
    }
}

/// culori's `clampChroma(color, "oklch", "rgb")`: bring a colour into the sRGB gamut by
/// finding the greatest chroma that fits, keeping lightness and hue.
///
/// The bisection runs until the interval is narrower than `0.4 / 2^13`, which is the
/// resolution culori derives from the Oklch chroma range.
#[must_use]
pub fn clamp_chroma(color: Oklch) -> Oklch {
    if displayable(oklch_to_rgb(color)) {
        return color;
    }

    // Try chroma = 0 first. If even grey is outside sRGB, clip channels instead.
    let achromatic = Oklch { c: 0.0, ..color };
    if !displayable(oklch_to_rgb(achromatic)) {
        let clipped = clip_rgb(oklch_to_rgb(achromatic));
        return rgb_to_oklch(clipped);
    }

    let end_limit = color.c;
    let resolution = OKLCH_CHROMA_MAX / 8192.0;

    let mut start = 0.0_f64;
    let mut end = end_limit;
    let mut last_good = achromatic.c;
    let mut candidate = achromatic;

    while end - start > resolution {
        candidate.c = start + (end - start) * 0.5;
        if displayable(oklch_to_rgb(candidate)) {
            last_good = candidate.c;
            start = candidate.c;
        } else {
            end = candidate.c;
        }
    }

    if displayable(oklch_to_rgb(candidate)) {
        candidate
    } else {
        Oklch {
            c: last_good,
            ..candidate
        }
    }
}

/// Interpolate two Oklch colours the way culori's `interpolate([a, b], "oklch")` does.
///
/// `t` is clamped to `[0, 1]`. Hue takes the shorter arc; a hue missing at one end is held at
/// the other end's value, and alpha is lerped treating a missing alpha as `1`.
fn interpolate_oklch(a: Oklch, b: Oklch, t: f64) -> Oklch {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 {
        return a;
    }

    let lerp = |x: f64, y: f64| x + t * (y - x);

    // culori's fixupHueShorter over the two hues, then a linear interpolation of the fixed pair.
    let hue = match (a.h, b.h) {
        (None, None) => None,
        (None, Some(hb)) => Some(normalize_hue(hb)),
        (Some(ha), None) => Some(normalize_hue(ha)),
        (Some(ha), Some(hb)) => {
            let first = normalize_hue(ha);
            let delta = normalize_hue(hb) - first;
            let delta = if delta.abs() <= 180.0 {
                delta
            } else {
                delta - 360.0 * delta.signum()
            };
            Some(lerp(first, first + delta))
        }
    };

    let alpha = match (a.alpha, b.alpha) {
        (None, None) => None,
        (x, y) => Some(lerp(x.unwrap_or(1.0), y.unwrap_or(1.0))),
    };

    Oklch {
        l: lerp(a.l, b.l),
        c: lerp(a.c, b.c),
        h: hue,
        alpha,
    }
}

/// Parse the colour syntaxes used by the component collection.
///
/// Supports `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` (with or without the `#`), the
/// `rgb()`/`rgba()` forms with numbers or percentages, `oklch()` with numbers or a percentage
/// lightness, and a small set of CSS colour names. Anything else — including `var(--token)`,
/// which cannot resolve outside a browser — degrades to opaque black, matching culori's guard.
#[must_use]
pub fn parse_color(text: &str) -> Rgba {
    const BLACK: Rgba = Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        alpha: None,
    };

    let s = text.trim();
    let lower = s.to_ascii_lowercase();

    if let Some(rgb) = parse_hex(&lower) {
        return rgb;
    }
    if lower.starts_with("rgb(") || lower.starts_with("rgba(") {
        return parse_rgb_function(&lower).unwrap_or(BLACK);
    }
    if lower.starts_with("oklch(") {
        return parse_oklch_function(&lower).unwrap_or(BLACK);
    }
    match lower.as_str() {
        "transparent" => Rgba {
            alpha: Some(0.0),
            ..BLACK
        },
        "white" => Rgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            alpha: None,
        },
        "red" => Rgba {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            alpha: None,
        },
        "green" => Rgba {
            r: 0.0,
            g: 128.0 / 255.0,
            b: 0.0,
            alpha: None,
        },
        "blue" => Rgba {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            alpha: None,
        },
        "yellow" => Rgba {
            r: 1.0,
            g: 1.0,
            b: 0.0,
            alpha: None,
        },
        "cyan" | "aqua" => Rgba {
            r: 0.0,
            g: 1.0,
            b: 1.0,
            alpha: None,
        },
        "magenta" | "fuchsia" => Rgba {
            r: 1.0,
            g: 0.0,
            b: 1.0,
            alpha: None,
        },
        "gray" | "grey" => Rgba {
            r: 128.0 / 255.0,
            g: 128.0 / 255.0,
            b: 128.0 / 255.0,
            alpha: None,
        },
        _ => BLACK,
    }
}

/// Parse a hex colour of 3, 4, 6 or 8 digits, with or without a leading `#`.
fn parse_hex(text: &str) -> Option<Rgba> {
    let body = text.strip_prefix('#').unwrap_or(text);
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let nibble = |index: usize| -> f64 {
        let c = body.as_bytes()[index] as char;
        f64::from(c.to_digit(16).unwrap_or(0))
    };
    // A doubled nibble is how CSS expands the short forms: `#c93` is `#cc9933`.
    let doubled = |index: usize| -> f64 { (nibble(index) * 16.0 + nibble(index)) / 255.0 };
    let pair = |index: usize| -> f64 { (nibble(index) * 16.0 + nibble(index + 1)) / 255.0 };
    match body.len() {
        3 => Some(Rgba {
            r: doubled(0),
            g: doubled(1),
            b: doubled(2),
            alpha: None,
        }),
        4 => Some(Rgba {
            r: doubled(0),
            g: doubled(1),
            b: doubled(2),
            alpha: Some(doubled(3)),
        }),
        6 => Some(Rgba {
            r: pair(0),
            g: pair(2),
            b: pair(4),
            alpha: None,
        }),
        8 => Some(Rgba {
            r: pair(0),
            g: pair(2),
            b: pair(4),
            alpha: Some(pair(6)),
        }),
        _ => None,
    }
}

/// Split the arguments of a CSS colour function, separating an optional `/ alpha` part.
fn function_args(text: &str) -> Option<(Vec<String>, Option<String>)> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close < open {
        return None;
    }
    let inner = &text[open + 1..close];
    let (head, tail) = match inner.split_once('/') {
        Some((head, tail)) => (head, Some(tail.to_owned())),
        None => (inner, None),
    };
    // `rgb(1, 2, 3)` and `rgb(1 2 3)` are both valid; the legacy comma form can also carry a
    // fourth alpha argument, which is handled by the caller.
    let parts: Vec<String> = head
        .split([',', ' '])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    Some((parts, tail))
}

/// A CSS numeric component: a plain number, or a percentage divided by 100.
fn number_or_percent(text: &str) -> Option<f64> {
    let text = text.trim().trim_end_matches("deg");
    match text.strip_suffix('%') {
        Some(number) => number.trim().parse::<f64>().ok().map(|v| v / 100.0),
        None => text.parse::<f64>().ok(),
    }
}

/// `rgb(r, g, b)` / `rgba(r, g, b, a)`: numbers are `0..=255`, percentages are `0..=100%`.
fn parse_rgb_function(text: &str) -> Option<Rgba> {
    let (parts, slash_alpha) = function_args(text)?;
    let channel = |index: usize| -> Option<f64> {
        let raw = parts.get(index)?;
        let value = number_or_percent(raw)?;
        Some(if raw.trim().ends_with('%') {
            value
        } else {
            value / 255.0
        })
    };
    let r = channel(0)?;
    let g = channel(1)?;
    let b = channel(2)?;
    let alpha = match slash_alpha {
        Some(alpha_text) => number_or_percent(&alpha_text),
        None => parts.get(3).and_then(|raw| number_or_percent(raw)),
    };
    Some(Rgba {
        r,
        g,
        b,
        alpha: alpha.map(|a| a.clamp(0.0, 1.0)),
    })
}

/// `oklch(l c h)` / `oklch(l% c h / a)`.
fn parse_oklch_function(text: &str) -> Option<Rgba> {
    let (parts, slash_alpha) = function_args(text)?;
    let l = number_or_percent(parts.first()?)?;
    let c = number_or_percent(parts.get(1)?)?;
    let h = parts
        .get(2)
        .and_then(|v| number_or_percent(v))
        .unwrap_or(0.0);
    let alpha = slash_alpha
        .as_deref()
        .and_then(number_or_percent)
        .map(|a| a.clamp(0.0, 1.0));
    Some(oklch_to_rgb(Oklch {
        l,
        c,
        h: Some(h),
        alpha,
    }))
}

/// Interpolate two colours in Oklch and bring the result back
/// into the sRGB gamut by reducing chroma.
///
/// `t` is taken as `f32` because callers' progress values are `f32`, and widened internally.
#[must_use]
pub fn mix_oklch(a: &str, b: &str, t: f32) -> Rgba {
    let mixed = interpolate_oklch(
        rgb_to_oklch(parse_color(a)),
        rgb_to_oklch(parse_color(b)),
        f64::from(t),
    );
    oklch_to_rgb(clamp_chroma(mixed))
}

/// culori's `formatRgb`: `rgb(r, g, b)`, or `rgba(r, g, b, a)` when the alpha is not opaque.
#[must_use]
pub fn format_rgb(color: Rgba) -> String {
    let (r, g, b) = (fixup(color.r), fixup(color.g), fixup(color.b));
    match color.alpha {
        None => format!("rgb({r}, {g}, {b})"),
        Some(alpha) if alpha >= 1.0 => format!("rgb({r}, {g}, {b})"),
        Some(alpha) => {
            let alpha = alpha.clamp(0.0, 1.0);
            format!("rgba({r}, {g}, {b}, {})", two_decimals(alpha))
        }
    }
}

/// An `#rrggbb` string for a colour, rounded to 8 bits the same way [`format_rgb`] rounds.
///
/// SVG fills are safer as hex than as `rgba(...)`, so this is what components should paint with.
#[must_use]
pub fn to_hex(color: Rgba) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        fixup(color.r),
        fixup(color.g),
        fixup(color.b)
    )
}

/// Apply an alpha value to a parsed colour and return its hex colour.
///
/// The colour is parsed and returned unchanged; the caller applies `alpha` through the SVG
/// `opacity`/`fill-opacity` attribute, because `rgba()` is not dependable in SVG `fill`.
/// This mirrors culori in leaving the colour itself untouched (no gamut mapping).
#[must_use]
pub fn with_alpha(color: &str, alpha: f32) -> String {
    let parsed = parse_color(color);
    to_hex(Rgba {
        alpha: Some(f64::from(alpha)),
        ..parsed
    })
}

/// The alpha of a colour, defaulting to `1` for opaque colours (culori's `parseColor` contract).
#[must_use]
pub fn alpha_of(color: &str) -> f64 {
    parse_color(color).alpha.unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixtures were generated by running culori 4.0.2's own `mixOklch` over 156 pairs.
    /// One tab-separated row per case: `a`, `b`, `t`, expected `formatRgb` output.
    const FIXTURES: &str = include_str!("mix_oklch_fixtures.txt");

    fn fixtures() -> Vec<(String, String, f32, String)> {
        FIXTURES
            .lines()
            .skip(1)
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let mut fields = line.split('\t');
                let a = fields.next().expect("fixture a").to_owned();
                let b = fields.next().expect("fixture b").to_owned();
                let t: f32 = fields
                    .next()
                    .expect("fixture t")
                    .parse()
                    .expect("fixture t is a number");
                let out = fields.next().expect("fixture out").to_owned();
                (a, b, t, out)
            })
            .collect()
    }

    #[test]
    fn matches_culori() {
        let fixtures = fixtures();
        assert!(
            fixtures.len() > 100,
            "expected the full fixture set, parsed {}",
            fixtures.len()
        );
        let mut failures = Vec::new();
        for (a, b, t, expected) in fixtures {
            let actual = format_rgb(mix_oklch(&a, &b, t));
            if actual != expected {
                failures.push(format!("mixOklch({a}, {b}, {t}): {actual} != {expected}"));
            }
        }
        assert!(
            failures.is_empty(),
            "{} of the culori fixtures differ:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn parses_the_hex_forms() {
        assert_eq!(to_hex(parse_color("#ffffff")), "#ffffff");
        assert_eq!(to_hex(parse_color("#000000")), "#000000");
        assert_eq!(to_hex(parse_color("#3072db")), "#3072db");
        // The short form doubles each nibble.
        assert_eq!(to_hex(parse_color("#c93")), "#cc9933");
        assert_eq!(to_hex(parse_color("3072db")), "#3072db");
        assert!((alpha_of("#ff000080") - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn unresolvable_colours_degrade_to_black_without_panicking() {
        // `var(--token)` cannot resolve outside a browser; culori returns black.
        assert_eq!(to_hex(parse_color("var(--primary)")), "#000000");
        assert_eq!(to_hex(parse_color("not a colour")), "#000000");
        assert_eq!(to_hex(parse_color("")), "#000000");
    }

    #[test]
    fn parsing_an_endpoint_is_a_fixed_point_of_oklch() {
        // A round trip through Oklch must land back on the same 8-bit colour.
        for hex in [
            "#3072db", "#faf9f6", "#141414", "#d92d20", "#6e6a63", "#00ff00",
        ] {
            let rgb = parse_color(hex);
            assert_eq!(
                to_hex(oklch_to_rgb(rgb_to_oklch(rgb))),
                hex,
                "round trip {hex}"
            );
        }
    }

    #[test]
    fn mixing_is_clamped_to_the_unit_interval() {
        // culori clamps `t`, so out-of-range inputs behave like the endpoints.
        assert_eq!(
            format_rgb(mix_oklch("#3072db", "#ffffff", -1.0)),
            format_rgb(parse_color("#3072db"))
        );
        assert_eq!(
            format_rgb(mix_oklch("#3072db", "#ffffff", 2.0)),
            format_rgb(parse_color("#ffffff"))
        );
    }

    #[test]
    fn hue_takes_the_shorter_arc() {
        // Red -> blue is a 235 degree hue step; the shorter arc passes through magenta at
        // roughly `rgb(178, 0, 184)` rather than through green.
        let middle = mix_oklch("#ff0000", "#0000ff", 0.5);
        assert_eq!(format_rgb(middle), "rgb(178, 0, 184)");
    }

    #[test]
    fn gamut_mapping_reduces_chroma_rather_than_clipping() {
        // Clipping the raw Oklch mix would give rgb(255, 97, 0); chroma reduction gives a
        // lighter orange, which is what the source paints.
        let mixed = mix_oklch("#00ff00", "#ff00ff", 0.5);
        assert_eq!(format_rgb(mixed), "rgb(255, 157, 84)");
    }

    #[test]
    fn with_alpha_keeps_the_colour_and_reports_alpha() {
        assert_eq!(with_alpha("#3072db", 0.16), "#3072db");
        assert!((alpha_of("rgba(48, 114, 219, 0.16)") - 0.16).abs() < 1e-6);
        assert!((alpha_of("#3072db") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn oklch_function_input_matches_the_equivalent_hex() {
        let from_function = parse_color("oklch(0.5674465 0.17437622 259.50405)");
        assert_eq!(to_hex(from_function), "#3072db");
    }
}
