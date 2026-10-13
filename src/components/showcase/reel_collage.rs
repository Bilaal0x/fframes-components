//! A photo reel closes, its caption swaps, and a collage blooms into view.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{CubicBezier, clamp01, interpolate, interpolate_multi, normalised_speed};
use crate::ui::{Mode, UiTheme, blur, text};

// ─── Geometry, in reference pixels ───────────────────────────────────────────

/// The reference recording's width. Every length in this scene is a fraction of it.
pub const REF_W: f32 = 712.0;

/// Caption type size at zoom 1, in reference px (`FONT_SIZE`).
pub const FONT_SIZE: f32 = 19.22;

/// The captions' tracking, in em (`LETTER_SPACING = "-0.068em"`).
pub const LETTER_SPACING_EM: f32 = -0.068;

/// The captions' weight (`fontWeight: 500`).
pub const CAPTION_WEIGHT: u16 = 500;

/// The reel's window width at zoom 1, in reference px.
pub const BOX_W: f32 = 282.08;

/// Caption baselines at the open end, relative to the page centre: `[top, bottom]`.
pub const CAPTION_OPEN: [f32; 2] = [-110.07, 125.53];

/// Caption baselines once the reel has shut: `[top, bottom]`.
pub const CAPTION_SHUT: [f32; 2] = [-5.44, 13.75];

/// How far left of centre the swapped pair starts, in reference px (`SWAP_DX`).
pub const SWAP_DX: f32 = -1.9;

/// Where the swapped lines come to rest, in reference px: `[[x, y]; top, bottom]`.
///
/// `x` is the ink centre and `y` the baseline.
pub const SWAP_REST: [[f32; 2]; 2] = [[-71.0, -94.6], [41.0, 104.6]];

/// The frame the copy swaps on (`SWAP_AT`). Half-way through the split.
pub const SWAP_AT: f32 = 46.0;

/// The frame each reel shot cuts in on (`CUTS`).
pub const CUTS: [f32; 8] = [0.0, 3.0, 6.0, 10.0, 14.0, 19.0, 25.0, 29.0];

/// One reel shot's fitted framing, in reference px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    /// The shot's own aspect ratio; the window's height is its width over this.
    pub aspect: f32,
    /// A hair of extra width the fit measured per cut.
    pub k: f32,
    /// The shot's centre, x, offset from the page centre in reference px.
    pub dx: f32,
    /// The shot's centre, y, offset from the page centre in reference px.
    pub dy: f32,
}

/// The eight shots' fitted framing, in cut order (`SHOTS`).
pub const SHOTS: [Shot; 8] = [
    Shot {
        aspect: 1.505,
        k: 1.0004,
        dx: 0.47,
        dy: 0.1,
    },
    Shot {
        aspect: 1.4788,
        k: 0.9982,
        dx: 0.78,
        dy: 0.09,
    },
    Shot {
        aspect: 1.4832,
        k: 1.0038,
        dx: 1.67,
        dy: 1.16,
    },
    Shot {
        aspect: 1.4561,
        k: 1.0009,
        dx: 1.59,
        dy: 0.93,
    },
    Shot {
        aspect: 1.4573,
        k: 0.9999,
        dx: 1.8,
        dy: 0.84,
    },
    Shot {
        aspect: 1.4505,
        k: 0.9988,
        dx: 1.92,
        dy: 1.3,
    },
    Shot {
        aspect: 1.4465,
        k: 0.9984,
        dx: 1.8,
        dy: 1.14,
    },
    Shot {
        aspect: 1.4465,
        k: 1.0,
        dx: 1.3,
        dy: -0.1,
    },
];

/// One collage slot's resting geometry, in reference px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollageSlot {
    /// The slot's centre, x, relative to the page centre.
    pub x: f32,
    /// The slot's centre, y, relative to the page centre.
    pub y: f32,
    /// The slot's width.
    pub w: f32,
    /// The slot's height.
    pub h: f32,
    /// The scale the picture blooms from, up to `1`.
    pub from: f32,
}

/// The six collage slots, listed bottom to top: left, centre, top right, bottom left, bottom
/// right, and the centre-right one that sits over it.
pub const COLLAGE: [CollageSlot; 6] = [
    CollageSlot {
        x: -195.5,
        y: -38.5,
        w: 105.0,
        h: 117.0,
        from: 0.616,
    },
    CollageSlot {
        x: -66.5,
        y: -1.0,
        w: 141.0,
        h: 96.0,
        from: 0.425,
    },
    CollageSlot {
        x: 106.5,
        y: -91.5,
        w: 161.0,
        h: 107.0,
        from: 0.554,
    },
    CollageSlot {
        x: -153.0,
        y: 108.0,
        w: 150.0,
        h: 102.0,
        from: 0.644,
    },
    CollageSlot {
        x: 200.0,
        y: 97.5,
        w: 142.0,
        h: 97.0,
        from: 0.61,
    },
    CollageSlot {
        x: 81.5,
        y: 27.5,
        w: 139.0,
        h: 97.0,
        from: 0.601,
    },
];

/// A blurred blob's fitted geometry, in reference px.
///
/// It is a rounded rectangle of half-extents `(hw, hh)` and corner radius `r`, centred `(dx, dy)`
/// from the page centre, painted in one flat colour and blurred by `blur`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blob {
    /// The blob's centre, x, relative to the page centre.
    pub dx: f32,
    /// The blob's centre, y, relative to the page centre.
    pub dy: f32,
    /// Half the blob's width, before blur.
    pub hw: f32,
    /// Half the blob's height, before blur.
    pub hh: f32,
    /// The blob's corner radius.
    pub r: f32,
    /// The CSS `blur()` radius.
    pub blur: f32,
}

/// The mint glow the whole scene sits on (`GLOW`).
pub const GLOW: Blob = Blob {
    dx: 10.9,
    dy: 6.4,
    hw: 261.6,
    hh: 147.0,
    r: 128.4,
    blur: 36.2,
};

/// The peach flush that rises around the edges as the collage blooms in (`FLUSH`).
pub const FLUSH: Blob = Blob {
    dx: 4.4,
    dy: -4.2,
    hw: 301.8,
    hh: 151.5,
    r: 2.0,
    blur: 58.6,
};

// ─── Timeline windows (frames @ 30fps) ───────────────────────────────────────

/// Caption zoom window: the type grows 17% across the reel.
pub const TEXT_ZOOM: [f32; 2] = [7.59, 37.0];

/// Reel zoom window: the frame runs a little ahead of the type.
pub const BOX_ZOOM: [f32; 2] = [0.42, 54.55];

/// Eyelid window: how open the reel is, `1 → 0`.
pub const REEL_OPEN: [f32; 2] = [18.71, 37.47];

/// Captions window: how far apart the two lines still are, `1 → 0`.
pub const CAPTIONS_OPEN: [f32; 2] = [18.4, 38.0];

/// The first swapped line's window: its split, `0 → 1`.
pub const SPLIT_TOP: [f32; 2] = [43.69, 69.24];

/// The second swapped line's window: its split, `0 → 1`. It leaves first.
pub const SPLIT_BOTTOM: [f32; 2] = [40.33, 69.32];

/// The collage's pan window: `57 → 0` reference px.
pub const COLLAGE_PAN: [f32; 2] = [46.66, 75.15];

/// The collage's bloom window: `0 → 1`.
pub const COLLAGE_BLOOM: [f32; 2] = [48.92, 73.18];

/// The collage's fade-in window, for every slot but the bottom-right one.
pub const COLLAGE_FADE: [f32; 2] = [50.14, 62.3];

/// The fade-in window of the bottom-right collage picture, which trails the rest.
pub const COLLAGE_FADE_TRAILING: [f32; 2] = [50.74, 65.08];

/// The flush's keyframes: out, full, held, gone.
pub const FLUSH_KEYFRAMES: [f32; 4] = [48.0, 56.0, 57.5, 73.5];

/// Every frame on which a fitted move finishes, for [`settle_frame`].
pub const MOTION_ENDS: [f32; 11] = [
    TEXT_ZOOM[1],
    BOX_ZOOM[1],
    REEL_OPEN[1],
    CAPTIONS_OPEN[1],
    SPLIT_TOP[1],
    SPLIT_BOTTOM[1],
    COLLAGE_PAN[1],
    COLLAGE_BLOOM[1],
    COLLAGE_FADE[1],
    COLLAGE_FADE_TRAILING[1],
    FLUSH_KEYFRAMES[3],
];

// ─── Fitted curves ───────────────────────────────────────────────────────────

/// Caption zoom's cubic-bezier.
const EASE_TEXT_ZOOM: CubicBezier = CubicBezier::new(0.9062, 0.1924, 0.4104, 1.0142);
/// Reel zoom's cubic-bezier.
const EASE_BOX_ZOOM: CubicBezier = CubicBezier::new(0.8548, 0.3903, 0.0817, 1.1691);
/// Eyelid's cubic-bezier.
const EASE_REEL_OPEN: CubicBezier = CubicBezier::new(0.8703, 0.0, 0.078, 1.0);
/// Caption folding's cubic-bezier.
const EASE_CAPTIONS_OPEN: CubicBezier = CubicBezier::new(0.8766, 0.0, 0.0413, 1.0);
/// The first swapped line's cubic-bezier.
const EASE_SPLIT_TOP: CubicBezier = CubicBezier::new(0.9243, 0.1575, 0.1018, 0.781);
/// The second swapped line's cubic-bezier.
const EASE_SPLIT_BOTTOM: CubicBezier = CubicBezier::new(0.9897, 0.0, 0.1683, 0.7764);
/// The collage's pan cubic-bezier.
const EASE_COLLAGE_PAN: CubicBezier = CubicBezier::new(0.8112, 0.3372, 0.0414, 0.6655);
/// The collage's bloom cubic-bezier.
const EASE_COLLAGE_BLOOM: CubicBezier = CubicBezier::new(0.5174, -0.3347, 0.0854, 1.0);

// ─── The blur stand-in's ladder ──────────────────────────────────────────────

/// The layers the blur stand-in is drawn as, outermost first: `(offset, alpha)`.
///
/// `offset` is in standard deviations of the source's own blur — CSS `blur(r)` is a Gaussian of
/// `σ = r / 2` — so the rectangle is grown (or shrunk) by `offset * σ` on every side at that
/// layer. The alphas are `1 - Φ(z) / Φ(z + 0.5)` for `z = 2.5, 2.0, … -2.5`, which makes the
/// stack's coverage exactly the Gaussian's CDF at each layer's edge: `0.5` at the shape's own
/// edge, `Φ(-z)` at `z` standard deviations outside it, and `0.9938` deep inside — a step
/// approximation of a Gaussian blur, which is all an SVG stack can be.
pub const BLUR_LAYERS: [(f32, f32); 11] = [
    (2.5, 0.006_210),
    (2.0, 0.016_644),
    (1.5, 0.045_083),
    (1.0, 0.098_423),
    (0.5, 0.178_146),
    (0.0, 0.276_895),
    (-0.5, 0.382_925),
    (-1.0, 0.485_783),
    (-1.5, 0.578_916),
    (-2.0, 0.659_466),
    (-2.5, 0.727_049),
];

/// The stack's coverage deep inside the shape: `Π(1 - alpha)`, i.e. what the page shows through.
pub const BLUR_CORE_SHOW_THROUGH: f32 = 0.006_210;

// ─── Pure functions: the scene's own arithmetic ──────────────────────────────

/// The canvas scale factor: the source's own `u = width / REF_W`.
///
/// Exactly `2.6966` on a 1920-wide canvas. See the module docs — this is **not**
/// [`crate::ui::text::unit`], and using that would draw the scene at 0.56x.
#[must_use]
pub fn scene_unit(canvas_width: f32) -> f32 {
    canvas_width / REF_W
}

/// A fitted segment: `from → to` across `[start, end]` on a cubic-bezier.
///
/// The input progress is clamped at both ends and the easing is applied to it afterwards, which is
/// Remotion's own order — so a curve whose control points dip below zero still undershoots, and one
/// that overshoots still passes its target.
#[must_use]
pub fn seg(frame: f32, window: [f32; 2], out: [f32; 2], ease: CubicBezier) -> f32 {
    let span = window[1] - window[0];
    let progress = if span.abs() < f32::EPSILON {
        1.0
    } else {
        clamp01((frame - window[0]) / span)
    };
    out[0] + (out[1] - out[0]) * ease.ease(progress)
}

/// Caption zoom: the type grows 17% across the reel.
#[must_use]
pub fn text_zoom(frame: f32) -> f32 {
    seg(frame, TEXT_ZOOM, [1.0, 1.1741], EASE_TEXT_ZOOM)
}

/// Reel zoom: the window runs a little ahead of the type.
#[must_use]
pub fn box_zoom(frame: f32) -> f32 {
    seg(frame, BOX_ZOOM, [1.0, 1.1852], EASE_BOX_ZOOM)
}

/// How open the reel is, `1 → 0`. The eyelid.
#[must_use]
pub fn reel_open(frame: f32) -> f32 {
    seg(frame, REEL_OPEN, [1.0, 0.0], EASE_REEL_OPEN)
}

/// How far apart the captions still are, `1 → 0`. Lags the eyelid at the tail.
#[must_use]
pub fn captions_open(frame: f32) -> f32 {
    seg(frame, CAPTIONS_OPEN, [1.0, 0.0], EASE_CAPTIONS_OPEN)
}

/// The first swapped line's split, `0 → 1`.
#[must_use]
pub fn split_top(frame: f32) -> f32 {
    seg(frame, SPLIT_TOP, [0.0, 1.0], EASE_SPLIT_TOP)
}

/// The second swapped line's split, `0 → 1`. The lower line leaves first.
#[must_use]
pub fn split_bottom(frame: f32) -> f32 {
    seg(frame, SPLIT_BOTTOM, [0.0, 1.0], EASE_SPLIT_BOTTOM)
}

/// The collage's leftward drift, in reference px: `57 → 0`.
#[must_use]
pub fn collage_pan(frame: f32) -> f32 {
    seg(frame, COLLAGE_PAN, [57.0, 0.0], EASE_COLLAGE_PAN)
}

/// The collage's bloom, `0 → 1`: each picture travels from its own `from` scale to `1`.
#[must_use]
pub fn collage_bloom(frame: f32) -> f32 {
    seg(frame, COLLAGE_BLOOM, [0.0, 1.0], EASE_COLLAGE_BLOOM)
}

/// Collage fade-in: linear, on its own window per slot.
///
/// Slot 4 — the bottom-right picture — trails the rest, which is the source's own special case.
#[must_use]
pub fn collage_opacity(frame: f32, slot: usize) -> f32 {
    let window = if slot == 4 {
        COLLAGE_FADE_TRAILING
    } else {
        COLLAGE_FADE
    };
    interpolate(frame, window, [0.0, 1.0])
}

/// The flush's keyframed opacity: up with the bloom, gone once it has landed.
#[must_use]
pub fn flush_opacity(frame: f32) -> f32 {
    interpolate_multi(frame, &FLUSH_KEYFRAMES, &[0.0, 1.0, 1.0, 0.0])
}

/// The reel shot on screen at `frame`: the last cut at or before it, or the first.
#[must_use]
pub fn slide_at(frame: f32) -> usize {
    let mut index = 0;
    for (i, cut) in CUTS.iter().enumerate() {
        if frame >= *cut {
            index = i;
        }
    }
    index
}

/// The framing of shot `index`, falling back to the first as the source's `?? SHOTS[0]` does.
#[must_use]
pub fn shot(index: usize) -> Shot {
    SHOTS.get(index).copied().unwrap_or(SHOTS[0])
}

/// The first frame on which nothing moves any more.
///
/// The last fitted move ends on [`COLLAGE_PAN`]'s `75.15`, so frame 75 still travels a fraction of
/// a pixel and frame 76 is the first at rest — the value `registry/__measured__.json` records as
/// `settle.base` for this component's shipped default.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn settle_frame() -> usize {
    let last = MOTION_ENDS.iter().copied().fold(0.0_f32, f32::max);
    last.ceil() as usize
}

/// The reel's window and the picture behind it, in reference px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReelGeometry {
    /// The window's width, already zoomed.
    pub box_w: f32,
    /// The window's visible height: the picture's full height times how open the eyelid is.
    pub box_h: f32,
    /// The picture's full height, of which the window shows a centred band.
    pub full_h: f32,
    /// The window's centre, x, relative to the page centre.
    pub dx: f32,
    /// The window's centre, y, relative to the page centre.
    pub dy: f32,
}

impl ReelGeometry {
    /// The window at `frame`, for shot `index`, in reference px.
    #[must_use]
    pub fn at(frame: f32, index: usize) -> Self {
        let shot = shot(index);
        let box_w = BOX_W * shot.k * box_zoom(frame);
        let full_h = box_w / shot.aspect;
        Self {
            box_w,
            box_h: full_h * reel_open(frame),
            full_h,
            dx: shot.dx,
            dy: shot.dy,
        }
    }

    /// Whether the window is worth drawing: the source's own `boxH > 0.01`, in canvas px.
    #[must_use]
    pub fn is_visible(&self, unit: f32) -> bool {
        self.box_h * unit > 0.01
    }
}

/// One caption line's animated state at a frame, in reference px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptionPlacement {
    /// The ink centre, x, relative to the page centre.
    pub x: f32,
    /// The **baseline**, y, relative to the page centre.
    pub y: f32,
    /// The type's zoom, applied about the line's own baseline and ink centre.
    pub zoom: f32,
    /// Whether the copy has already swapped: `false` draws the open caption, `true` the swapped one.
    pub swapped: bool,
}

/// Where caption `line` (`0` top, `1` bottom) sits at `frame`, in reference px.
///
/// The open baseline and the shut one are mixed by how far apart the captions still are, the swap's
/// own rest position is mixed in by that line's split, and both are pivoted on the line's baseline
/// so the zoom never climbs the pixel grid.
#[must_use]
pub fn caption_placement(frame: f32, line: usize) -> CaptionPlacement {
    let index = if line == 0 { 0 } else { 1 };
    let zoom = text_zoom(frame);
    let apart = captions_open(frame);
    let swapped = frame >= SWAP_AT;
    let split = if index == 0 {
        split_top(frame)
    } else {
        split_bottom(frame)
    };

    let open_y = CAPTION_OPEN[index];
    let shut_y = CAPTION_SHUT[index];
    let baseline = zoom * (open_y * apart + shut_y * (1.0 - apart));
    let drift = if swapped {
        SWAP_DX * (1.0 - split)
    } else {
        0.0
    };
    CaptionPlacement {
        x: drift + SWAP_REST[index][0] * split,
        y: baseline + (SWAP_REST[index][1] - zoom * shut_y) * split,
        zoom,
        swapped,
    }
}

/// One collage slot's animated state at a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollageMotion {
    /// Slot `4` fades in on its own trailing window; the rest share one.
    pub opacity: f32,
    /// The scale the picture is drawn at, blooming from its own `from` to `1`.
    pub scale: f32,
    /// The whole collage's leftward drift, in reference px.
    pub pan: f32,
}

/// The collage's state at `frame`, for `slot`.
///
/// A slot outside [`COLLAGE`] blooms from `1`, which draws nothing moving.
#[must_use]
pub fn collage_motion(frame: f32, slot: usize) -> CollageMotion {
    let from = COLLAGE.get(slot).map_or(1.0, |s| s.from);
    let bloom = collage_bloom(frame);
    CollageMotion {
        opacity: collage_opacity(frame, slot),
        scale: from + (1.0 - from) * bloom,
        pan: collage_pan(frame),
    }
}

// ─── Pure functions: reference px → canvas px ────────────────────────────────

/// A rectangle on the canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// The rectangle's centre, x.
    #[must_use]
    pub fn centre_x(&self) -> f32 {
        self.x + self.w / 2.0
    }

    /// The rectangle's centre, y.
    #[must_use]
    pub fn centre_y(&self) -> f32 {
        self.y + self.h / 2.0
    }

    /// The blob layer `offset` reference px larger on every side, or `None` when that collapses.
    #[must_use]
    pub fn grown(&self, offset: f32) -> Option<Self> {
        let w = self.w + 2.0 * offset;
        let h = self.h + 2.0 * offset;
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        Some(Self {
            x: self.x - offset,
            y: self.y - offset,
            w,
            h,
        })
    }
}

/// The reel's visible window on the canvas, for a page centred on `centre`.
#[must_use]
pub fn reel_window(centre: (f32, f32), unit: f32, geometry: &ReelGeometry) -> Rect {
    let (centre_x, centre_y) = centre;
    Rect {
        x: centre_x + geometry.dx * unit - geometry.box_w * unit / 2.0,
        y: centre_y + geometry.dy * unit - geometry.box_h * unit / 2.0,
        w: geometry.box_w * unit,
        h: geometry.box_h * unit,
    }
}

/// The picture behind the reel's window, on the canvas.
///
/// It keeps its fully open height; only the window closes on it, so its top sits above the window
/// by `(full_h - box_h) / 2` — negative once the eyelid is shut.
#[must_use]
pub fn reel_picture(centre: (f32, f32), unit: f32, geometry: &ReelGeometry) -> Rect {
    let window = reel_window(centre, unit, geometry);
    Rect {
        x: window.x,
        y: window.centre_y() - geometry.full_h * unit / 2.0,
        w: window.w,
        h: geometry.full_h * unit,
    }
}

/// One collage slot's box on the canvas, before its bloom scale, for a page centred on `centre`.
#[must_use]
pub fn collage_box(centre: (f32, f32), unit: f32, slot: &CollageSlot, pan: f32) -> Rect {
    let (centre_x, centre_y) = centre;
    Rect {
        x: centre_x + (slot.x + pan - slot.w / 2.0) * unit,
        y: centre_y + (slot.y - slot.h / 2.0) * unit,
        w: slot.w * unit,
        h: slot.h * unit,
    }
}

/// The anchor one caption line is drawn at: the ink centre and the **baseline**.
#[must_use]
pub fn caption_anchor(centre: (f32, f32), unit: f32, placement: &CaptionPlacement) -> (f32, f32) {
    (centre.0 + placement.x * unit, centre.1 + placement.y * unit)
}

/// The blob's own rounded rectangle on the canvas, for a page centred on `centre`.
#[must_use]
pub fn blob_box(centre: (f32, f32), unit: f32, blob: &Blob) -> Rect {
    Rect {
        x: centre.0 + (blob.dx - blob.hw) * unit,
        y: centre.1 + (blob.dy - blob.hh) * unit,
        w: 2.0 * blob.hw * unit,
        h: 2.0 * blob.hh * unit,
    }
}

/// The blur's standard deviation in reference px: CSS `blur(r)` is a Gaussian of `r / 2`.
#[must_use]
pub fn blur_sigma(blob: &Blob) -> f32 {
    blob.blur / 2.0
}

/// The original abstract placeholder images embedded in `media/`, in the reel's cut order.
///
/// They are local media names, so the shipped render is deterministic, offline, and free of
/// third-party image licensing requirements.
pub const DEFAULT_SLIDES: [&str; 8] = [
    "placeholder-01.png",
    "placeholder-02.png",
    "placeholder-03.png",
    "placeholder-04.png",
    "placeholder-05.png",
    "placeholder-06.png",
    "placeholder-07.png",
    "placeholder-08.png",
];

/// The six embedded placeholder images used by the final collage, in [`COLLAGE`] slot order.
pub const DEFAULT_COLLAGE: [&str; 6] = [
    "placeholder-03.png",
    "placeholder-01.png",
    "placeholder-06.png",
    "placeholder-02.png",
    "placeholder-08.png",
    "placeholder-05.png",
];

/// The pool entry at `index`, wrapping as the source's `pool[index % pool.length]` does.
#[must_use]
pub fn pool_entry(pool: &[String], index: usize) -> Option<&str> {
    if pool.is_empty() {
        return None;
    }
    pool.get(index % pool.len()).map(String::as_str)
}

// ─── Configuration ───────────────────────────────────────────────────────────

/// Configuration for [`ReelCollage`].
#[derive(Debug, Clone, PartialEq)]
pub struct ReelCollageConfig {
    /// The caption above the reel.
    pub top_line: String,
    /// The caption below the reel.
    pub bottom_line: String,
    /// The first line after the swap — it drifts up and to the left.
    pub swap_top: String,
    /// The second line after the swap — it drifts down and to the right.
    pub swap_bottom: String,
    /// The reel, in cut order. Names resolve through the host media provider.
    pub slides: Vec<String>,
    /// The collage, six pictures in [`COLLAGE`] slot order. Names resolve through the host media
    /// provider.
    pub collage: Vec<String>,
    /// The page.
    pub background: String,
    /// The soft glow the whole scene sits on.
    pub glow_color: String,
    /// The flush that rises around the edges as the collage blooms in.
    pub flush_color: String,
    /// Caption ink. `None` takes the theme's `foreground`.
    pub text_color: Option<String>,
    /// Which shadcn palette to paint from.
    pub mode: Mode,
    /// A whole token set, replacing the palette for `mode`.
    pub theme: Option<UiTheme>,
    /// The face to paint in. `None` keeps the theme's face, then the crate's embedded face.
    pub font_family: Option<String>,
    /// Timeline multiplier. Non-positive and non-finite values are treated as `1`.
    pub speed: f32,
    /// Composition length in frames.
    pub duration: usize,
    /// Whether to draw the blur stand-in behind the glow and the flush.
    pub simulate_blur: bool,
}

impl Default for ReelCollageConfig {
    /// `config.ts`'s demo defaults, field for field, plus the things it does not carry: the two
    /// asset pools (`index.tsx`'s `DEFAULT_SLIDES` and `DEFAULT_COLLAGE`), `mode`, which the source
    /// resolves to the light palette, and `simulate_blur`,
    /// which this crate adds to every port that stands in for a CSS filter.
    fn default() -> Self {
        Self {
            top_line: "The videos".to_owned(),
            bottom_line: "That you ship".to_owned(),
            swap_top: "Especially on".to_owned(),
            swap_bottom: "Launch day".to_owned(),
            slides: DEFAULT_SLIDES
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            collage: DEFAULT_COLLAGE
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            background: "#f5f4f2".to_owned(),
            glow_color: "#dfe7e3".to_owned(),
            flush_color: "#fdc7bc".to_owned(),
            text_color: None,
            mode: Mode::Light,
            theme: None,
            font_family: None,
            speed: 1.0,
            duration: 96,
            simulate_blur: true,
        }
    }
}

// ─── The component ───────────────────────────────────────────────────────────

/// The **Reel Collage** component.
#[derive(Debug, Clone, Default)]
pub struct ReelCollage {
    /// Component options.
    pub config: ReelCollageConfig,
}

impl ReelCollage {
    /// Build with the given options.
    #[must_use]
    pub fn new(config: ReelCollageConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (the catalog needs this).
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "reel-collage",
            name: "Reel Collage",
            summary: "A photo reel shuts like an eyelid, the copy swaps and a collage blooms in",
            feature: "reel-collage",
        }
    }

    /// The theme this instance paints from.
    fn theme(&self) -> UiTheme {
        self.config
            .theme
            .clone()
            .unwrap_or_else(|| UiTheme::for_mode(self.config.mode))
    }
}

/// One blurred blob: a stack of the same rounded rectangle, growing and fading outward.
///
/// The alphas in [`BLUR_LAYERS`] make the stack's own coverage the Gaussian's CDF, so this reads as
/// a defocused shape rather than as a ring of copies. `simulate_blur: false` draws the shape alone.
fn blob_nodes(
    centre: (f32, f32),
    unit: f32,
    blob: &Blob,
    colour: &str,
    opacity: f32,
    simulate_blur: bool,
) -> Svgr<'static> {
    let shape = blob_box(centre, unit, blob);
    let radius = blob.r * unit;
    let sigma = blur_sigma(blob) * unit;
    let mut layers: Vec<Svgr<'static>> = Vec::new();

    if simulate_blur && blur::is_visible(blob.blur) {
        for (offset, alpha) in BLUR_LAYERS {
            let grown = offset * sigma;
            let Some(layer) = shape.grown(grown) else {
                continue;
            };
            let r = (radius + grown).max(0.0);
            layers.push(fframes::svgr!(
                <rect x={layer.x} y={layer.y} width={layer.w} height={layer.h}
                      rx={r} ry={r} fill={colour.to_owned()} opacity={alpha} />
            ));
        }
    } else {
        layers.push(fframes::svgr!(
            <rect x={shape.x} y={shape.y} width={shape.w} height={shape.h}
                  rx={radius} ry={radius} fill={colour.to_owned()} opacity={1.0} />
        ));
    }

    let layers = Svgr::from(layers);
    fframes::svgr!(<g opacity={opacity}>{layers}</g>)
}

impl Component for ReelCollage {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        // `config.ts` ships `previewBackdrop: { type: "color", value: "#f5f4f2" }`, which is also
        // the `background` prop's own default.
        Some("#f5f4f2")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let theme = self.theme();
        let family =
            text::resolve_face(config.font_family.as_ref(), theme.font_family.as_ref(), ctx);
        let ink = config
            .text_color
            .clone()
            .unwrap_or_else(|| theme.foreground.clone());

        let width = ctx.width();
        let height = ctx.height();
        let centre = (width / 2.0, height / 2.0);
        // The source's own `const u = width / REF_W`, *not* `ctx.vmin() / 7.2`.
        let unit = scene_unit(width);
        let frame = ctx.frame() * normalised_speed(config.speed);

        let mut defs: Vec<Svgr<'static>> = Vec::new();
        let mut nodes: Vec<Svgr<'static>> = Vec::new();

        // The stage. `config.ts` declares the backdrop, and the source paints it on the page.
        nodes.push(fframes::svgr!(
            <rect x={0.0} y={0.0} width={width} height={height} fill={config.background.clone()} />
        ));

        // ── The two blobs: the flush first, so the glow washes over it ──
        let flush = flush_opacity(frame);
        if flush > 0.0 {
            nodes.push(blob_nodes(
                centre,
                unit,
                &FLUSH,
                &config.flush_color,
                flush,
                config.simulate_blur,
            ));
        }
        nodes.push(blob_nodes(
            centre,
            unit,
            &GLOW,
            &config.glow_color,
            1.0,
            config.simulate_blur,
        ));

        // ── The reel: a centred window that shuts on a full-size picture ──
        let slide = slide_at(frame);
        let geometry = ReelGeometry::at(frame, slide);
        if geometry.is_visible(unit)
            && let Some(src) = pool_entry(&config.slides, slide)
        {
            let window = reel_window(centre, unit, &geometry);
            let picture = reel_picture(centre, unit, &geometry);
            let clip_id = "reel-collage-reel".to_owned();

            defs.push(fframes::svgr!(
                <clipPath id={clip_id.clone()}>
                    <rect x={window.x} y={window.y} width={window.w} height={window.h} />
                </clipPath>
            ));

            // The picture keeps its open height; only the window closes on it. The neutral rect is
            // deliberately underneath the image, so an unresolved custom name remains legible.
            let clip = format!("url(#{clip_id})");
            nodes.push(fframes::svgr!(
                <g clip-path={clip}>
                    <rect x={picture.x} y={picture.y} width={picture.w} height={picture.h}
                          fill="#242424" />
                    <image href={src.to_owned()} x={picture.x} y={picture.y}
                           width={picture.w} height={picture.h}
                           preserveAspectRatio="xMidYMid slice" />
                </g>
            ));
        }

        // ── The collage: six pictures, bottom to top in slot order ──
        for (slot, place) in COLLAGE.iter().enumerate() {
            let motion = collage_motion(frame, slot);
            if motion.opacity <= 0.0 {
                continue;
            }
            // An empty pool has no entry to draw, exactly as the source's `pool[i % 0]` yields
            // `undefined` and the slot renders nothing.
            let Some(src) = pool_entry(&config.collage, slot) else {
                continue;
            };
            let panel = collage_box(centre, unit, place, motion.pan);
            // CSS scales about the box's own centre, which is where the source's `transform:
            // scale()` leaves the slot, so the group pivots there.
            let transform = format!(
                "translate({:.5} {:.5}) scale({:.6})",
                panel.centre_x(),
                panel.centre_y(),
                motion.scale
            );
            nodes.push(fframes::svgr!(
                <g transform={transform} opacity={motion.opacity}>
                    <rect x={-panel.w / 2.0} y={-panel.h / 2.0} width={panel.w} height={panel.h}
                          fill="#242424" />
                    <image href={src.to_owned()} x={-panel.w / 2.0} y={-panel.h / 2.0}
                           width={panel.w} height={panel.h}
                           preserveAspectRatio="xMidYMid slice" />
                </g>
            ));
        }

        // ── The captions: SVG text, so `y` is the baseline and the zoom pivots on it ──
        for line in 0..2 {
            let place = caption_placement(frame, line);
            let content = match (place.swapped, line) {
                (false, 0) => &config.top_line,
                (false, _) => &config.bottom_line,
                (true, 0) => &config.swap_top,
                (true, _) => &config.swap_bottom,
            };
            if content.is_empty() {
                continue;
            }
            let (anchor_x, baseline) = caption_anchor(centre, unit, &place);
            // The group's scale is folded into the size and the tracking: scaling a run about its
            // own anchor is the same as measuring and drawing it at the scaled size.
            let size = FONT_SIZE * unit * place.zoom;
            let tracking = text::tracking_px_from_em(LETTER_SPACING_EM, size);
            nodes.push(text::text_node(
                ctx,
                content,
                anchor_x,
                baseline,
                family.clone(),
                size,
                CAPTION_WEIGHT,
                tracking,
                ink.clone(),
                1.0,
            ));
        }

        let defs = Svgr::from(defs);
        let nodes = Svgr::from(nodes);
        fframes::svgr!(
            <g>
                <defs>{defs}</defs>
                {nodes}
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canvas scale the tests use: the crate's default 1920x1080 canvas.
    const UNIT: f32 = 2.696_629_2;
    /// The page centre of that canvas.
    const CENTRE: (f32, f32) = (960.0, 540.0);

    fn close(a: f32, b: f32, tolerance: f32) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn the_scene_scales_by_width_not_by_the_family_unit() {
        // The source's own `u = width / REF_W`. On 1080p that is 2.6966, not the collection's
        // 1.5 design unit — the mistake that draws this scene at 0.56x.
        assert!(close(scene_unit(1920.0), 2.696_629_2, 1e-5));
        assert!(close(scene_unit(1280.0), 1.797_752_8, 1e-5));
        assert!(close(scene_unit(960.0), 1.348_314_6, 1e-5));
        assert!(close(scene_unit(1920.0) / scene_unit(1280.0), 1.5, 1e-5));
    }

    #[test]
    fn a_segment_eases_between_its_ends_and_clamps_outside_them() {
        let ease = EASE_COLLAGE_PAN;
        assert!(close(
            seg(0.0, [46.66, 75.15], [57.0, 0.0], ease),
            57.0,
            1e-4
        ));
        assert!(close(
            seg(46.66, [46.66, 75.15], [57.0, 0.0], ease),
            57.0,
            1e-4
        ));
        assert!(close(
            seg(75.15, [46.66, 75.15], [57.0, 0.0], ease),
            0.0,
            1e-4
        ));
        assert!(close(
            seg(1000.0, [46.66, 75.15], [57.0, 0.0], ease),
            0.0,
            1e-4
        ));
        // Half-way in time is *not* half-way in value: the curve is a slow start, a fast middle.
        let middle = seg(60.905, [46.66, 75.15], [57.0, 0.0], ease);
        assert!(middle < 57.0 && middle > 0.0);
        // A degenerate window is a step, not a division by zero.
        assert!(close(seg(0.0, [10.0, 10.0], [1.0, 0.0], ease), 0.0, 1e-4));
    }

    #[test]
    fn the_fitted_curves_start_and_end_on_the_sources_own_numbers() {
        assert!(close(text_zoom(0.0), 1.0, 1e-4));
        assert!(close(text_zoom(37.0), 1.1741, 1e-4));
        assert!(close(box_zoom(0.0), 1.0, 1e-4));
        assert!(close(box_zoom(54.55), 1.1852, 1e-4));
        assert!(close(reel_open(0.0), 1.0, 1e-4));
        assert!(close(reel_open(37.47), 0.0, 1e-4));
        assert!(close(captions_open(0.0), 1.0, 1e-4));
        assert!(close(captions_open(38.0), 0.0, 1e-4));
        assert!(close(split_top(43.69), 0.0, 1e-4));
        assert!(close(split_top(69.24), 1.0, 1e-4));
        assert!(close(split_bottom(40.33), 0.0, 1e-4));
        assert!(close(split_bottom(69.32), 1.0, 1e-4));
        assert!(close(collage_pan(46.66), 57.0, 1e-4));
        assert!(close(collage_pan(75.15), 0.0, 1e-4));
        assert!(close(collage_bloom(48.92), 0.0, 1e-4));
        assert!(close(collage_bloom(73.18), 1.0, 1e-4));
    }

    #[test]
    fn the_flush_keys_on_its_four_frames() {
        assert!(close(flush_opacity(0.0), 0.0, 1e-6));
        assert!(close(flush_opacity(48.0), 0.0, 1e-6));
        assert!(close(flush_opacity(52.0), 0.5, 1e-6));
        assert!(close(flush_opacity(56.0), 1.0, 1e-6));
        assert!(close(flush_opacity(57.5), 1.0, 1e-6));
        assert!(close(flush_opacity(65.5), 0.5, 1e-6));
        assert!(close(flush_opacity(73.5), 0.0, 1e-6));
        assert!(close(flush_opacity(200.0), 0.0, 1e-6));
    }

    #[test]
    fn the_bottom_right_picture_fades_in_last() {
        // The source's own special case: slot 4 trails the rest.
        for frame in [0.0, 50.0, 50.14, 55.0, 62.3, 70.0, 96.0] {
            if frame < COLLAGE_FADE_TRAILING[0] {
                assert!(
                    close(collage_opacity(frame, 4), 0.0, 1e-6),
                    "slot 4 at {frame}"
                );
            }
            assert!(
                collage_opacity(frame, 4) <= collage_opacity(frame, 0) + 1e-6,
                "slot 4 should never lead slot 0 (frame {frame})"
            );
        }
        assert!(close(
            collage_opacity(56.0, 4),
            (56.0 - 50.74) / (65.08 - 50.74),
            1e-5
        ));
        assert!(close(
            collage_opacity(56.0, 0),
            (56.0 - 50.14) / (62.3 - 50.14),
            1e-5
        ));
        assert!(close(collage_opacity(90.0, 0), 1.0, 1e-6));
        assert!(close(collage_opacity(0.0, 0), 0.0, 1e-6));
    }

    #[test]
    fn every_slot_blooms_from_its_own_scale_to_one() {
        let cold = collage_motion(0.0, 1);
        assert!(close(cold.opacity, 0.0, 1e-6));
        assert!(close(cold.scale, COLLAGE[1].from, 1e-6));
        assert!(close(cold.pan, 57.0, 1e-4));

        let landed = collage_motion(96.0, 1);
        assert!(close(landed.opacity, 1.0, 1e-6));
        assert!(close(landed.scale, 1.0, 1e-6));
        assert!(close(landed.pan, 0.0, 1e-4));

        // Part way through, the scale sits between its own start and 1.
        let mid = collage_motion(65.0, 5);
        assert!(
            mid.scale > COLLAGE[5].from && mid.scale < 1.0,
            "{}",
            mid.scale
        );
        // A slot outside the list has nothing to bloom from, and nothing to draw.
        assert!(close(collage_motion(65.0, 99).scale, 1.0, 1e-6));
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn the_bloom_undershoots_exactly_as_the_source_curve_does() {
        // `Easing.bezier(0.5174, -0.3347, 0.0854, 1)` dips below zero, and Remotion applies the
        // easing after clamping the *input*, so the excursion survives. It is small — and it lands
        // while the panels are still nearly transparent.
        let lowest = (49..=60)
            .map(|frame| collage_bloom(frame as f32))
            .fold(f32::MAX, f32::min);
        assert!(lowest < 0.0 && lowest > -0.06, "deepest bloom {lowest}");
        assert!(collage_bloom(52.0) < 0.0);
        // Once it is positive it never goes back.
        let rising = (56..=96)
            .map(|frame| collage_bloom(frame as f32))
            .collect::<Vec<_>>();
        for pair in rising.windows(2) {
            assert!(pair[1] >= pair[0] - 1e-6);
        }
    }

    #[test]
    fn the_reel_cuts_on_the_sources_own_frames() {
        assert_eq!(slide_at(-1.0), 0);
        assert_eq!(slide_at(0.0), 0);
        assert_eq!(slide_at(2.99), 0);
        assert_eq!(slide_at(3.0), 1);
        assert_eq!(slide_at(9.0), 2);
        assert_eq!(slide_at(10.0), 3);
        assert_eq!(slide_at(19.0), 5);
        assert_eq!(slide_at(29.0), 7);
        assert_eq!(slide_at(96.0), 7);
        assert_eq!(SHOTS.len(), CUTS.len());
        assert_eq!(shot(99), SHOTS[0]);
    }

    #[test]
    fn the_settle_frame_reproduces_the_registry_measurement() {
        // `registry/__measured__.json` records `settle.base = 76`; the last fitted move ends on
        // the collage's pan at 75.15, so frame 75 still travels and 76 is the first at rest.
        let last = MOTION_ENDS.iter().copied().fold(0.0_f32, f32::max);
        assert!(close(last, COLLAGE_PAN[1], 1e-6));
        assert!(last > 75.0 && last < 76.0);
        assert_eq!(settle_frame(), 76);
        assert_eq!(ReelCollage::default().duration(), 96);
    }

    #[test]
    fn the_reel_window_opens_on_the_shot_and_shuts_on_the_eyelid() {
        // Frame 0: the window is at its open size for shot 0, aspect 1.505.
        let open = ReelGeometry::at(0.0, 0);
        assert!(close(open.box_w, 282.08 * 1.0004, 1e-3));
        assert!(close(open.full_h, open.box_w / 1.505, 1e-3));
        assert!(close(open.box_h, open.full_h, 1e-3));
        assert!(open.is_visible(UNIT));

        // By the end of the eyelid the window is a hairline and is not drawn.
        let shut = ReelGeometry::at(37.47, 0);
        assert!(close(shut.box_h, 0.0, 1e-4));
        assert!(!shut.is_visible(UNIT));

        // The window zooms with the reel, ahead of the type.
        let zoomed = ReelGeometry::at(54.55, 7);
        assert!(close(zoomed.box_w, BOX_W * 1.0 * 1.1852, 1e-3));
    }

    #[test]
    fn the_drawn_reel_window_matches_the_demos_own_ink() {
        // Measured on `reference/free/reel-collage.mp4` frame 0 (file frame 0 = source
        // frame 0) with a loose threshold: the dark picture spans x 290..670 and y 144..396 in
        // demo px. The demo is 960 wide and this canvas is 1920, so demo px x 2 = canvas px.
        let geometry = ReelGeometry::at(0.0, 0);
        let window = reel_window(CENTRE, UNIT, &geometry);
        assert!(close(window.x, 580.8, 1.0), "left {}", window.x);
        assert!(close(window.w, 761.0, 1.0), "width {}", window.w);
        assert!(close(window.y, 287.5, 1.0), "top {}", window.y);
        assert!(close(window.h, 505.6, 1.0), "height {}", window.h);
        // The demo's own readings, converted: the dark picture's ink runs x 290..670 and
        // y 144..396 inclusive, i.e. edges at 289.5..670.5 and 143.5..396.5, doubled.
        assert!(close(window.x, 289.5 * 2.0, 2.0), "left against the demo");
        assert!(
            close(window.x + window.w, 670.5 * 2.0, 2.0),
            "right against the demo"
        );
        assert!(close(window.y, 143.5 * 2.0, 1.5), "top against the demo");
        assert!(
            close(window.y + window.h, 396.5 * 2.0, 1.5),
            "bottom against the demo"
        );

        // The picture behind it is the full open height, centred on the window.
        let picture = reel_picture(CENTRE, UNIT, &geometry);
        assert!(close(picture.centre_y(), window.centre_y(), 1e-6));
        assert!(close(picture.h, geometry.full_h * UNIT, 1e-6));
        assert!(close(picture.w, window.w, 1e-6));
    }

    #[test]
    fn the_drawn_caption_anchor_matches_the_demos_own_ink() {
        // Frame 0: nothing has zoomed or folded, so the top caption's baseline is the source's
        // `CAPTION_OPEN.top` and its ink centre is the page centre. Measured on demo frame 0:
        // the top caption's ink spans x 421..539 (centre 480 = the page centre) and its last ink
        // row is 121, i.e. the baseline at 121.6 — so 960 and 243.2 on this canvas.
        let place = caption_placement(0.0, 0);
        assert!(!place.swapped);
        assert!(close(place.x, 0.0, 1e-6));
        assert!(close(place.y, -110.07, 1e-6));
        assert!(close(place.zoom, 1.0, 1e-6));
        let (anchor_x, baseline) = caption_anchor(CENTRE, UNIT, &place);
        assert!(close(anchor_x, 960.0, 1e-3), "anchor x {anchor_x}");
        assert!(close(baseline, 243.2, 0.5), "baseline {baseline}");
        assert!(close(baseline, 121.6 * 2.0, 1.0));

        // The bottom line is the page centre plus `CAPTION_OPEN.bottom`.
        let (_, bottom) = caption_anchor(CENTRE, UNIT, &caption_placement(0.0, 1));
        assert!(close(bottom, 540.0 + 125.53 * UNIT, 1e-3));
        assert!(close(bottom, 439.3 * 2.0, 1.0));

        // The type is drawn at `FONT_SIZE * u * zoom`: 51.8 px here, the demo's 25.9 doubled.
        assert!(close(
            FONT_SIZE * UNIT * caption_placement(0.0, 0).zoom,
            51.83,
            0.05
        ));
    }

    #[test]
    fn the_captions_fold_then_swap_then_split() {
        // Before the fold both lines are fully apart, on the open baselines, at zoom 1.
        let early = caption_placement(5.0, 0);
        assert!(close(early.zoom, 1.0, 1e-6));
        assert!(close(early.y, -110.07, 1e-4));
        // The type has started to grow by frame 10 — its own window opens on 7.59.
        assert!(caption_placement(10.0, 0).zoom > 1.0);

        // Once the fold is over the lines share the shut baselines, scaled by the zoom.
        let folded = caption_placement(38.0, 0);
        let zoom = text_zoom(38.0);
        assert!(close(folded.y, zoom * CAPTION_SHUT[0], 1e-4));
        assert!(close(folded.y, -6.387, 0.01));

        // The copy swaps on frame 46, not before.
        assert!(!caption_placement(45.9, 0).swapped);
        assert!(caption_placement(46.0, 0).swapped);

        // And by the composition's end the lines have split to their rest positions.
        let top = caption_placement(96.0, 0);
        let bottom = caption_placement(96.0, 1);
        assert!(close(top.x, SWAP_REST[0][0], 1e-4));
        assert!(close(top.y, SWAP_REST[0][1], 1e-4));
        assert!(close(bottom.x, SWAP_REST[1][0], 1e-4));
        assert!(close(bottom.y, SWAP_REST[1][1], 1e-4));
        assert!(close(text_zoom(96.0), 1.1741, 1e-4));

        // The rest positions are the ink centre and the baseline, so the anchors land there.
        assert!(close(
            caption_anchor(CENTRE, UNIT, &top).0,
            960.0 - 71.0 * UNIT,
            1e-3
        ));
        assert!(close(
            caption_anchor(CENTRE, UNIT, &top).1,
            540.0 - 94.6 * UNIT,
            1e-3
        ));

        // The lower line leaves first: its split is keyed earlier.
        assert!(split_bottom(42.0) > 0.0);
        assert!(close(split_top(42.0), 0.0, 1e-6));
    }

    #[test]
    fn the_drawn_collage_box_matches_the_demos_own_ink() {
        // The settled frame (source 96, demo file frame 184): every panel is at its rest scale and
        // the pan has landed. Measured on the demo: the collage's own extremes are x 145..844 and
        // the bottom-left panel's underside is y 484.4 — i.e. exactly slots 0's left edge, 4's
        // right edge and 3's bottom, on this canvas 290..1688 and 968.8.
        let pan = collage_pan(96.0);
        let left = collage_box(CENTRE, UNIT, &COLLAGE[0], pan);
        assert!(close(left.x, 145.0 * 2.0, 3.0), "left {}", left.x);
        assert!(close(left.w, 105.0 * UNIT, 1e-3));
        assert!(close(left.h, 117.0 * UNIT, 1e-3));
        let right = collage_box(CENTRE, UNIT, &COLLAGE[4], pan);
        assert!(close(right.x + right.w, 844.5 * 2.0, 3.0));
        let bottom = collage_box(CENTRE, UNIT, &COLLAGE[3], pan);
        assert!(close(bottom.y + bottom.h, 484.5 * 2.0, 3.0));

        // A panel is scaled about its own centre, so a bloom never moves the centre.
        let centre = collage_box(CENTRE, UNIT, &COLLAGE[1], 0.0);
        assert!(close(centre.centre_x(), 960.0 + COLLAGE[1].x * UNIT, 1e-3));
        assert!(close(centre.centre_y(), 540.0 + COLLAGE[1].y * UNIT, 1e-3));
    }

    #[test]
    fn the_drawn_blob_is_the_sources_own_rounded_box() {
        let glow = blob_box(CENTRE, UNIT, &GLOW);
        assert!(close(glow.w, 2.0 * 261.6 * UNIT, 1e-3));
        assert!(close(glow.h, 2.0 * 147.0 * UNIT, 1e-3));
        assert!(close(glow.centre_x(), 960.0 + 10.9 * UNIT, 1e-3));
        assert!(close(glow.centre_y(), 540.0 + 6.4 * UNIT, 1e-3));
        assert!(close(blur_sigma(&GLOW), 18.1, 1e-4));

        // A layer grown outward is the box plus twice the offset on each axis.
        let layer = glow.grown(18.1).expect("a grown layer");
        assert!(close(layer.w, glow.w + 2.0 * 18.1, 1e-3));
        assert!(close(layer.centre_x(), glow.centre_x(), 1e-3));
        // A layer shrunk past its own size draws nothing.
        assert!(glow.grown(-glow.h / 2.0).is_none());
    }

    #[test]
    fn the_blur_ladder_reproduces_a_gaussian_edges_coverage() {
        let mut show_through = 1.0_f32;
        for (_, alpha) in BLUR_LAYERS {
            assert!(alpha > 0.0 && alpha < 1.0, "alpha {alpha}");
            show_through *= 1.0 - alpha;
        }
        // Deep inside the shape the stack is 99.4% opaque — a Gaussian's own deficit at 2.5σ.
        assert!(close(show_through, BLUR_CORE_SHOW_THROUGH, 1e-6));
        assert!(close(1.0 - show_through, 0.993_79, 1e-5));

        // The innermost copy carries the most ink and the outermost the least, and the ladder is
        // strictly monotone.
        assert!(BLUR_LAYERS[10].1 > BLUR_LAYERS[0].1);
        for pair in BLUR_LAYERS.windows(2) {
            assert!(
                pair[1].1 > pair[0].1,
                "{:?} should be above {:?}",
                pair[1],
                pair[0]
            );
            assert!(pair[1].0 < pair[0].0);
        }

        // Coverage at the shape's own edge is a half, which is the definition of the blur's edge.
        let mut edge_show_through = 1.0_f32;
        for (offset, alpha) in BLUR_LAYERS {
            if offset >= 0.0 {
                edge_show_through *= 1.0 - alpha;
            }
        }
        assert!(
            close(edge_show_through, 0.5, 1e-5),
            "edge coverage {}",
            1.0 - edge_show_through
        );
    }

    #[test]
    fn a_pool_wraps_and_an_empty_pool_draws_nothing() {
        let pool = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
        assert_eq!(pool_entry(&pool, 0), Some("a"));
        assert_eq!(pool_entry(&pool, 2), Some("c"));
        assert_eq!(pool_entry(&pool, 3), Some("a"));
        assert_eq!(pool_entry(&pool, 8), Some("c"));
        let empty: Vec<String> = Vec::new();
        assert_eq!(pool_entry(&empty, 0), None);
    }

    #[test]
    fn the_embedded_placeholder_images_are_the_shipped_defaults() {
        let config = ReelCollageConfig::default();
        assert_eq!(config.top_line, "The videos");
        assert_eq!(config.bottom_line, "That you ship");
        assert_eq!(config.swap_top, "Especially on");
        assert_eq!(config.swap_bottom, "Launch day");
        assert_eq!(
            config.slides,
            DEFAULT_SLIDES
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            config.collage,
            DEFAULT_COLLAGE
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>()
        );
        assert_eq!(config.background, "#f5f4f2");
        assert_eq!(config.glow_color, "#dfe7e3");
        assert_eq!(config.flush_color, "#fdc7bc");
        assert_eq!(config.text_color, None);
        assert_eq!(config.mode, Mode::Light);
        assert_eq!(config.theme, None);
        assert_eq!(config.font_family, None);
        assert!(close(config.speed, 1.0, 1e-6));
        assert_eq!(config.duration, 96);
        assert!(config.simulate_blur);
    }

    #[test]
    fn the_geometry_constants_are_the_sources_own() {
        assert!(close(REF_W, 712.0, 1e-6));
        assert!(close(FONT_SIZE, 19.22, 1e-6));
        assert!(close(LETTER_SPACING_EM, -0.068, 1e-6));
        assert!(close(BOX_W, 282.08, 1e-6));
        assert_eq!(CAPTION_OPEN, [-110.07, 125.53]);
        assert_eq!(CAPTION_SHUT, [-5.44, 13.75]);
        assert!(close(SWAP_DX, -1.9, 1e-6));
        assert_eq!(SWAP_REST, [[-71.0, -94.6], [41.0, 104.6]]);
        assert!(close(SWAP_AT, 46.0, 1e-6));
        assert_eq!(CUTS, [0.0, 3.0, 6.0, 10.0, 14.0, 19.0, 25.0, 29.0]);
        assert_eq!(SHOTS[0].aspect, 1.505);
        assert_eq!(SHOTS[7].dx, 1.3);
        assert_eq!(COLLAGE[1].from, 0.425);
        assert_eq!(GLOW.blur, 36.2);
        assert_eq!(FLUSH.blur, 58.6);
        assert_eq!(CAPTION_WEIGHT, 500);
    }

    #[test]
    fn the_component_reports_its_own_identity() {
        let component = ReelCollage::default();
        assert_eq!(ReelCollage::meta_static().id, "reel-collage");
        assert_eq!(ReelCollage::meta_static().feature, "reel-collage");
        assert_eq!(component.meta().name, "Reel Collage");
        assert_eq!(component.background(), Some("#f5f4f2"));
        let custom = ReelCollage::new(ReelCollageConfig {
            duration: 120,
            ..Default::default()
        });
        assert_eq!(custom.duration(), 120);
    }
}
