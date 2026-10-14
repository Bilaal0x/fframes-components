//! An A-to-B map flyover: the camera follows a route drawn over satellite imagery.
//!
//! Ported from Remotion Elements' `MapFlyover` (`a-to-b-map-flyover.tsx`). The route is a great
//! circle between two coordinates, bent half way back toward the straight line on the Web
//! Mercator plane. The route draws itself as the camera travels with its head: the map is a
//! "plate" larger than the canvas that is only ever translated, never zoomed.
//!
//! Deliberate differences from the source:
//!
//! - **One embedded image, not live tiles.** The source loads NASA Blue Marble tiles through
//!   MapLibre. This crate renders offline and deterministically, so the basemap is
//!   `media/blue-marble-z4.jpg`: the same imagery as the 16 x 16 zoom-4 GIBS tiles, stitched into
//!   one 4096 x 4096 Web Mercator picture of the whole world. It is placed with the source's own
//!   projection (`projectToMapPlate`), so it lines up with the source's MapLibre view, but it has
//!   less detail than the source's tiles. Long routes look alike; short city-to-city routes (high
//!   plate zoom) look softer.
//! - **World copies.** The source sets `renderWorldCopies: true`, so a route unwrapped past
//!   +-180 degrees longitude still sits on land. Here the image is drawn again beside itself
//!   whenever a neighbouring copy is on screen.
//! - **Thinner route.** The default `line_width` is 16, not the source's 24. It stays
//!   configurable (the source allows 2 to 24) and everything else derives from it as in the
//!   source (`markerRadius = max(14, lineWidth * 0.8)`, and so on).
//! - **Duration.** The source has none (it is a layer inside a composition). The default here is
//!   240 frames, 8 s at 30 fps: 160 frames of travel, then 80 for the arrival.
//! - **Shadow.** `drop-shadow(0 2px 3px rgba(0, 0, 0, 0.9))` on the labels is a small ring of
//!   offset black copies, from [`crate::ui::blur`], since SVG filters are not used in this crate.
//! - **Great circle.** `turf.greatCircle` is replaced by a plain spherical interpolation of the
//!   same 200 points; `turf.length` by a haversine sum with the same earth radius.
//! - **Reveal.** The source animates a dash offset of a `pathLength = 1` path. Here the polyline
//!   is cut at the same arc length, with the same round caps and joins.
//! - **Font.** The labels use the embedded Inter, not whatever `sans-serif` the browser has.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{clamp01, ease_in_out_cubic, interpolate, lerp};
use crate::ui::{blur, text};

/// The page and the area outside the map plate: `backgroundColor: '#dbe4e8'`.
pub const BACKGROUND: &str = "#dbe4e8";
/// The image's edge in px: the 16 x 16 zoom-4 GIBS tiles of 256 px.
pub const IMAGE_SIZE: f64 = 4096.0;
/// The imagery file, by its name in `media/`.
pub const IMAGE_NAME: &str = "blue-marble-z4.jpg";
/// MapLibre's tile size: a world is `512 * 2^zoom` px wide.
pub const WORLD_BASE: f64 = 512.0;
/// The largest plate edge: `4096 / width`, `4096 / height` in `calculateMapPlate`.
pub const MAX_PLATE: f64 = 4096.0;
/// Mean earth radius in km used by `turf.length` (`earthRadius`).
pub const EARTH_RADIUS_KM: f64 = 6371.0088;
/// Points in the great circle: `turf.greatCircle(start, end, {npoints: 200})`.
pub const ROUTE_POINTS: usize = 200;
/// How far the route keeps its great-circle bend: `* 0.5` toward the straight line.
pub const BEND_KEEP: f64 = 0.5;
/// A start and end closer than this (degrees) make a stub route: `0.000001`.
pub const SAME_POINT_EPSILON: f64 = 0.000001;
/// The stub route's length in degrees of longitude: `start[0] + 0.0001`.
pub const STUB_LENGTH: f64 = 0.0001;

/// The share of the canvas width the overview must fit: `width * 0.82`.
pub const FIT_WIDTH: f64 = 0.82;
/// The share of the canvas height the overview must fit: `height * 0.72`.
pub const FIT_HEIGHT: f64 = 0.72;
/// The route distances (km) the maximum overview zoom is interpolated over.
pub const DISTANCE_RANGE: [f64; 2] = [100.0, 10000.0];
/// The maximum overview zoom at those distances: `[7, 3]`.
pub const MAX_ZOOM_RANGE: [f64; 2] = [7.0, 3.0];
/// How far past the overview the plate zoom may go: `overviewZoom + 1.25`.
pub const ZOOM_EXTRA: f64 = 1.25;

/// The default route width: the source's 24, made thinner at the user's request.
pub const DEFAULT_LINE_WIDTH: f32 = 16.0;
/// The smallest route width the source's schema allows.
pub const MIN_LINE_WIDTH: f32 = 2.0;
/// The largest route width the source's schema allows.
pub const MAX_LINE_WIDTH: f32 = 24.0;
/// The default route colour.
pub const DEFAULT_ROUTE_COLOR: &str = "#ff5c4d";
/// The default length in frames (the source has none): 8 s at 30 fps.
pub const DEFAULT_DURATION: usize = 240;
/// Frames held after the travel for the arrival: `durationInFrames - 80`.
pub const ARRIVAL_FRAMES: f32 = 80.0;

/// The origin label fades out over this many frames from the start.
pub const ORIGIN_FADE_FRAMES: f32 = 14.0;
/// The marker's smallest radius: `Math.max(14, ...)`.
pub const MARKER_MIN_RADIUS: f32 = 14.0;
/// The marker's radius per px of line width: `lineWidth * 0.8`.
pub const MARKER_PER_LINE: f32 = 0.8;
/// The destination centre dot's radius per px of marker radius: `markerRadius * 0.27`.
pub const CENTER_RATIO: f32 = 0.27;
/// The destination marker grows over these frames after the travel: `[travelEnd + 5, + 17]`.
pub const MARKER_FRAMES: [f32; 2] = [5.0, 17.0];
/// The centre dot grows over these frames after the travel: `[travelEnd + 19, + 29]`.
pub const CENTER_FRAMES: [f32; 2] = [19.0, 29.0];
/// The destination label fades in over these frames after the travel: `[travelEnd + 27, + 35]`.
pub const LABEL_FRAMES: [f32; 2] = [27.0, 35.0];

/// The labels' font size.
pub const LABEL_SIZE: f32 = 26.0;
/// The labels' font weight.
pub const LABEL_WEIGHT: u16 = 600;
/// The labels' colour.
pub const LABEL_INK: &str = "#ffffff";
/// The label baseline's distance below the marker's rim: `y + markerRadius + 26`.
pub const LABEL_DROP: f32 = 26.0;
/// The label shadow's vertical offset: `drop-shadow(0 2px ...)`.
pub const SHADOW_DY: f32 = 2.0;
/// The label shadow's blur radius, in px: `... 3px ...`.
pub const SHADOW_BLUR: f32 = 3.0;
/// The opacity of the shadow copy under the glyphs; with the ring it builds toward the source's 0.9.
pub const SHADOW_CORE_ALPHA: f32 = 0.6;
/// The opacity of each ring copy of the shadow.
pub const SHADOW_RING_ALPHA: f32 = 0.3;

/// The vignette's colour: `rgba(10, 20, 28, 0.22)`.
pub const VIGNETTE_INK: &str = "#0a141c";
/// The vignette's opacity at its outer stop.
pub const VIGNETTE_OPACITY: f32 = 0.22;
/// Where the vignette starts to darken, as a share of the farthest corner: `45%`.
pub const VIGNETTE_START: f32 = 0.45;
/// Where the vignette reaches full strength, as a share of the farthest corner: `130%`.
pub const VIGNETTE_END: f32 = 1.30;

/// The attribution badge's text; the imagery's credit.
pub const ATTRIBUTION: &str = "NASA EOSDIS / GIBS";
/// The badge's inset from the right and bottom edges: `right: 8, bottom: 8`.
pub const BADGE_INSET: f32 = 8.0;
/// The badge's font size.
pub const BADGE_SIZE: f32 = 12.0;
/// The badge's font weight.
pub const BADGE_WEIGHT: u16 = 400;
/// The badge's text colour.
pub const BADGE_INK: &str = "#111827";
/// The badge's fill (white) opacity: `rgba(255, 255, 255, 0.8)`.
pub const BADGE_OPACITY: f32 = 0.8;
/// The badge's corner radius.
pub const BADGE_RADIUS: f32 = 4.0;
/// The badge's horizontal padding.
pub const BADGE_PAD_X: f32 = 6.0;
/// The badge's vertical padding.
pub const BADGE_PAD_Y: f32 = 3.0;
/// Inter's `line-height: normal`, as a multiple of the size.
pub const INTER_LINE_NORMAL: f32 = 1.2102;

/// A coordinate in Web Mercator, `0..1` across the world (`MercatorCoordinate`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mercator {
    /// West to east, `0` at -180 degrees and `1` at +180. Unwrapped longitudes go past `0..1`.
    pub x: f64,
    /// North to south, `0` at 85.0511 degrees north.
    pub y: f64,
}

/// `MercatorCoordinate.fromLngLat`.
#[must_use]
pub fn from_lng_lat(lng: f64, lat: f64) -> Mercator {
    let pi = std::f64::consts::PI;
    Mercator {
        x: (180.0 + lng) / 360.0,
        y: (180.0 - (180.0 / pi) * (pi / 4.0 + lat * pi / 360.0).tan().ln()) / 360.0,
    }
}

/// `MercatorCoordinate.toLngLat`, as `[lng, lat]`.
#[must_use]
pub fn to_lng_lat(point: Mercator) -> [f64; 2] {
    let pi = std::f64::consts::PI;
    let y2 = 180.0 - point.y * 360.0;
    [
        point.x * 360.0 - 180.0,
        360.0 / pi * (y2 * pi / 180.0).exp().atan() - 90.0,
    ]
}

/// `unwrapLongitude`: `longitude` shifted by whole turns to lie within 180 of `reference`.
#[must_use]
pub fn unwrap_longitude(longitude: f64, reference: f64) -> f64 {
    let mut unwrapped = longitude;
    while unwrapped - reference > 180.0 {
        unwrapped -= 360.0;
    }
    while unwrapped - reference < -180.0 {
        unwrapped += 360.0;
    }
    unwrapped
}

/// `points` evenly spaced positions along the great circle from `start` to `end`, as `[lng, lat]`.
///
/// Spherical linear interpolation, the same points `turf.greatCircle` returns: both ends
/// included, longitudes in `-180..=180`. Antipodal ends have no unique circle; those fall back to
/// a straight line in longitude and latitude.
#[must_use]
pub fn great_circle(start: [f64; 2], end: [f64; 2], points: usize) -> Vec<[f64; 2]> {
    let to_vector = |p: [f64; 2]| {
        let (lng, lat) = (p[0].to_radians(), p[1].to_radians());
        [lat.cos() * lng.cos(), lat.cos() * lng.sin(), lat.sin()]
    };
    let (a, b) = (to_vector(start), to_vector(end));
    let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1.0, 1.0);
    let angle = dot.acos();
    let sin_angle = angle.sin();
    let last = points.saturating_sub(1).max(1) as f64;
    (0..points)
        .map(|i| {
            let f = i as f64 / last;
            if i == 0 {
                return start;
            }
            if i + 1 == points {
                return end;
            }
            if sin_angle.abs() < 1e-12 {
                return [
                    start[0] + (end[0] - start[0]) * f,
                    start[1] + (end[1] - start[1]) * f,
                ];
            }
            let wa = ((1.0 - f) * angle).sin() / sin_angle;
            let wb = (f * angle).sin() / sin_angle;
            let v = [
                wa * a[0] + wb * b[0],
                wa * a[1] + wb * b[1],
                wa * a[2] + wb * b[2],
            ];
            [
                v[1].atan2(v[0]).to_degrees(),
                v[2].atan2(v[0].hypot(v[1])).to_degrees(),
            ]
        })
        .collect()
}

/// `createFlyoverRoute`: the route's `[lng, lat]` points.
///
/// The great circle, unwrapped across the antimeridian, then each point pulled half way back
/// toward the straight line between the ends on the Mercator plane.
#[must_use]
pub fn flyover_route(start: [f64; 2], end: [f64; 2]) -> Vec<[f64; 2]> {
    if (start[0] - end[0]).abs() < SAME_POINT_EPSILON
        && (start[1] - end[1]).abs() < SAME_POINT_EPSILON
    {
        return vec![start, [start[0] + STUB_LENGTH, start[1]]];
    }
    let mut coordinates: Vec<[f64; 2]> = Vec::with_capacity(ROUTE_POINTS);
    for coordinate in great_circle(start, end, ROUTE_POINTS) {
        let previous = coordinates.last().map_or(coordinate[0], |c| c[0]);
        coordinates.push([unwrap_longitude(coordinate[0], previous), coordinate[1]]);
    }
    let first = from_lng_lat(coordinates[0][0], coordinates[0][1]);
    let last_coordinate = coordinates[coordinates.len() - 1];
    let last = from_lng_lat(last_coordinate[0], last_coordinate[1]);
    let span = (coordinates.len() - 1) as f64;
    coordinates
        .iter()
        .enumerate()
        .map(|(index, coordinate)| {
            let progress = index as f64 / span;
            let arc = from_lng_lat(coordinate[0], coordinate[1]);
            let straight_x = first.x + (last.x - first.x) * progress;
            let straight_y = first.y + (last.y - first.y) * progress;
            to_lng_lat(Mercator {
                x: straight_x + (arc.x - straight_x) * BEND_KEEP,
                y: straight_y + (arc.y - straight_y) * BEND_KEEP,
            })
        })
        .collect()
}

/// `turf.length` of a line in km: the haversine distance summed along it.
#[must_use]
pub fn route_length_km(route: &[[f64; 2]]) -> f64 {
    route.windows(2).map(|w| haversine_km(w[0], w[1])).sum()
}

/// The haversine distance in km between two `[lng, lat]` points.
#[must_use]
pub fn haversine_km(a: [f64; 2], b: [f64; 2]) -> f64 {
    let d_lat = (b[1] - a[1]).to_radians();
    let d_lng = (b[0] - a[0]).to_radians();
    let h = (d_lat / 2.0).sin().powi(2)
        + (d_lng / 2.0).sin().powi(2) * a[1].to_radians().cos() * b[1].to_radians().cos();
    2.0 * EARTH_RADIUS_KM * h.sqrt().atan2((1.0 - h).sqrt())
}

/// The map plate: the oversized rectangle the camera slides over (`calculateMapPlate`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapPlate {
    /// The plate's centre, as `[lng, lat]`.
    pub center: [f64; 2],
    /// The plate's height in px.
    pub height: f64,
    /// The plate's width in px.
    pub width: f64,
    /// The map zoom the plate is drawn at.
    pub zoom: f64,
}

/// `calculateMapPlate` for a `width` x `height` canvas.
#[must_use]
pub fn calculate_map_plate(
    width: f64,
    height: f64,
    route: &[[f64; 2]],
    route_distance: f64,
) -> MapPlate {
    let points: Vec<Mercator> = route.iter().map(|c| from_lng_lat(c[0], c[1])).collect();
    let min_x = points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let max_x = points.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let min_y = points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_y = points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let span_x = (max_x - min_x).max(0.000001);
    let span_y = (max_y - min_y).max(0.000001);
    let center = to_lng_lat(Mercator {
        x: (min_x + max_x) / 2.0,
        y: (min_y + max_y) / 2.0,
    });
    let multiplier = (MAX_PLATE / width).min(MAX_PLATE / height);
    let plate_width = (width * multiplier).floor();
    let plate_height = (height * multiplier).floor();
    let max_overview = interpolate_f64(route_distance, DISTANCE_RANGE, MAX_ZOOM_RANGE);
    let overview_zoom = ((width * FIT_WIDTH) / (WORLD_BASE * span_x))
        .log2()
        .min(((height * FIT_HEIGHT) / (WORLD_BASE * span_y)).log2())
        .min(max_overview);
    let max_plate_zoom = ((plate_width - width).max(1.0) / (WORLD_BASE * span_x))
        .log2()
        .min(((plate_height - height).max(1.0) / (WORLD_BASE * span_y)).log2());
    let max_covered_zoom = overview_zoom + (plate_width / width).min(plate_height / height).log2();
    let zoom = overview_zoom.max(
        (overview_zoom + ZOOM_EXTRA)
            .min(max_plate_zoom)
            .min(max_covered_zoom),
    );
    MapPlate {
        center,
        height: plate_height,
        width: plate_width,
        zoom,
    }
}

/// `interpolate` for f64, clamped at both ends.
fn interpolate_f64(v: f64, input: [f64; 2], output: [f64; 2]) -> f64 {
    let t = ((v - input[0]) / (input[1] - input[0])).clamp(0.0, 1.0);
    output[0] + (output[1] - output[0]) * t
}

/// The world's width in px at the plate's zoom: `512 * 2^zoom`.
#[must_use]
pub fn world_size(plate: &MapPlate) -> f64 {
    WORLD_BASE * 2f64.powf(plate.zoom)
}

/// `projectToMapPlate`: `[lng, lat]` to a position in plate px.
#[must_use]
pub fn project_to_plate(coordinate: [f64; 2], plate: &MapPlate) -> (f64, f64) {
    let center = from_lng_lat(plate.center[0], plate.center[1]);
    let point = from_lng_lat(coordinate[0], coordinate[1]);
    let world = world_size(plate);
    (
        (point.x - center.x) * world + plate.width / 2.0,
        (point.y - center.y) * world + plate.height / 2.0,
    )
}

/// One projected route point and its distance along the route.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoutePoint {
    /// Distance from the route's start, in plate px.
    pub distance: f64,
    /// Plate x.
    pub x: f64,
    /// Plate y.
    pub y: f64,
}

/// The route in plate px with its arc length (`projectFlyoverRoute`).
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedRoute {
    /// Every point, with its running distance.
    pub points: Vec<RoutePoint>,
}

impl ProjectedRoute {
    /// Project `route` onto `plate`.
    #[must_use]
    pub fn new(route: &[[f64; 2]], plate: &MapPlate) -> Self {
        Self::from_points(route.iter().map(|c| project_to_plate(*c, plate)))
    }

    /// A route through plate positions, in order.
    #[must_use]
    pub fn from_points(positions: impl IntoIterator<Item = (f64, f64)>) -> Self {
        let mut length = 0.0;
        let mut previous: Option<(f64, f64)> = None;
        let points = positions
            .into_iter()
            .map(|(x, y)| {
                if let Some((px, py)) = previous {
                    length += (x - px).hypot(y - py);
                }
                previous = Some((x, y));
                RoutePoint {
                    distance: length,
                    x,
                    y,
                }
            })
            .collect();
        Self { points }
    }

    /// The route's first point.
    #[must_use]
    pub fn start(&self) -> RoutePoint {
        self.points[0]
    }

    /// The route's last point.
    #[must_use]
    pub fn end(&self) -> RoutePoint {
        self.points[self.points.len() - 1]
    }

    /// The route's whole length in plate px.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.end().distance
    }

    /// `pointAtProgress`: the position `progress` (`0..=1`) of the way along the route by length.
    #[must_use]
    pub fn point_at_progress(&self, progress: f64) -> (f64, f64) {
        let target = self.length() * progress;
        let next_index = match self.points.iter().position(|p| p.distance >= target) {
            None => self.points.len() - 1,
            Some(found) => found.max(1),
        };
        let next = self.points[next_index];
        let previous = self.points[next_index - 1];
        let t = if next.distance == previous.distance {
            0.0
        } else {
            (target - previous.distance) / (next.distance - previous.distance)
        };
        (
            previous.x + (next.x - previous.x) * t,
            previous.y + (next.y - previous.y) * t,
        )
    }

    /// The route's path from its start to `progress`, as SVG path data; empty at no progress.
    ///
    /// The source draws the whole path with a dash offset of `1 - progress` on a `pathLength = 1`
    /// path. Cutting at the same arc length draws the same line.
    #[must_use]
    pub fn path_to(&self, progress: f64) -> String {
        if progress <= 0.0 {
            return String::new();
        }
        let progress = progress.min(1.0);
        let target = self.length() * progress;
        let mut path = String::new();
        for point in self.points.iter().filter(|p| p.distance < target) {
            let command = if path.is_empty() { 'M' } else { 'L' };
            path.push_str(&format!("{command} {} {} ", point.x as f32, point.y as f32));
        }
        let (x, y) = self.point_at_progress(progress);
        let command = if path.is_empty() { 'M' } else { 'L' };
        path.push_str(&format!("{command} {} {}", x as f32, y as f32));
        path
    }
}

/// The world copies (`0` is the centre one) whose span overlaps the visible part of the plate.
///
/// `left` is where copy `0` starts, `world` its width, `view` the visible `[min, max]` plate x and
/// `plate_width` the plate's extent: nothing is drawn outside `0..plate_width`.
#[must_use]
pub fn visible_copies(left: f64, world: f64, view: [f64; 2], plate_width: f64) -> Vec<i32> {
    let low = view[0].max(0.0);
    let high = view[1].min(plate_width);
    if high <= low || world <= 0.0 {
        return Vec::new();
    }
    let first = ((low - left) / world).floor() as i32;
    let last = ((high - left) / world).floor() as i32;
    (first..=last).collect()
}

/// `Easing.inOut(Easing.quad)`.
#[must_use]
pub fn ease_in_out_quad(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - 2.0 * (1.0 - t) * (1.0 - t)
    }
}

/// The frame at which the travel ends: `Math.max(1, durationInFrames - 80)`.
#[must_use]
pub fn travel_end(duration: usize) -> f32 {
    (duration as f32 - ARRIVAL_FRAMES).max(1.0)
}

/// How far along the route the camera is: `easeInOut(quad)` over `[0, travelEnd]`.
#[must_use]
pub fn travel_progress(frame: f32, duration: usize) -> f32 {
    ease_in_out_quad(interpolate(frame, [0.0, travel_end(duration)], [0.0, 1.0]))
}

/// The origin label's opacity: gone after 14 frames.
#[must_use]
pub fn origin_label_opacity(frame: f32) -> f32 {
    interpolate(frame, [0.0, ORIGIN_FADE_FRAMES], [1.0, 0.0])
}

/// The markers' radius: `Math.max(14, lineWidth * 0.8)`.
#[must_use]
pub fn marker_radius(line_width: f32) -> f32 {
    MARKER_MIN_RADIUS.max(line_width * MARKER_PER_LINE)
}

/// The destination marker's scale: from the line's own cap to full size.
#[must_use]
pub fn destination_marker_scale(frame: f32, duration: usize, line_width: f32) -> f32 {
    let end = travel_end(duration);
    let t = clamp01((frame - (end + MARKER_FRAMES[0])) / (MARKER_FRAMES[1] - MARKER_FRAMES[0]));
    let from = line_width / 2.0 / marker_radius(line_width);
    lerp(from, 1.0, ease_in_out_cubic(t))
}

/// The destination marker's centre dot scale, `0` to `1`.
#[must_use]
pub fn destination_center_scale(frame: f32, duration: usize) -> f32 {
    let end = travel_end(duration);
    ease_in_out_cubic((frame - (end + CENTER_FRAMES[0])) / (CENTER_FRAMES[1] - CENTER_FRAMES[0]))
}

/// The destination label's opacity, `0` to `1`.
#[must_use]
pub fn destination_label_opacity(frame: f32, duration: usize) -> f32 {
    let end = travel_end(duration);
    ease_in_out_quad((frame - (end + LABEL_FRAMES[0])) / (LABEL_FRAMES[1] - LABEL_FRAMES[0]))
}

/// What a [`MapFlyover`] shows.
#[derive(Debug, Clone, PartialEq)]
pub struct MapFlyoverConfig {
    /// Where the route starts, as `[longitude, latitude]` in degrees.
    pub origin: [f64; 2],
    /// Where the route ends, as `[longitude, latitude]` in degrees.
    pub destination: [f64; 2],
    /// The label under the origin marker, shown at the start.
    pub origin_label: String,
    /// The label under the destination marker, shown on arrival.
    pub destination_label: String,
    /// The route and marker colour, as a hex string.
    pub route_color: String,
    /// The route's stroke width in px; the source allows 2 to 24.
    pub line_width: f32,
    /// The font family, or `None` for the embedded Inter.
    pub font_family: Option<String>,
    /// The length in frames; the last 80 are the arrival.
    pub duration: usize,
}

impl Default for MapFlyoverConfig {
    fn default() -> Self {
        Self {
            origin: [-0.1276, 51.5072],
            destination: [139.6917, 35.6895],
            origin_label: "London".to_owned(),
            destination_label: "Tokyo".to_owned(),
            route_color: DEFAULT_ROUTE_COLOR.to_owned(),
            line_width: DEFAULT_LINE_WIDTH,
            font_family: None,
            duration: DEFAULT_DURATION,
        }
    }
}

/// The map flyover component.
#[derive(Debug, Clone, Default)]
pub struct MapFlyover {
    /// What the flyover shows.
    pub config: MapFlyoverConfig,
}

impl MapFlyover {
    /// A flyover with `config`.
    #[must_use]
    pub fn new(config: MapFlyoverConfig) -> Self {
        Self { config }
    }

    /// The component's catalog entry, without an instance.
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "map-flyover",
            name: "Map Flyover",
            summary: "The camera pans along a route drawn on satellite imagery, from A to B",
            feature: "map-flyover",
        }
    }
}

/// An empty node.
fn nothing() -> Svgr<'static> {
    Svgr::from(Vec::<Svgr<'static>>::new())
}

/// A centred label with the source's drop shadow, at `opacity`.
fn label(
    ctx: &RenderCtx<'_>,
    content: &str,
    x: f32,
    y: f32,
    family: &str,
    opacity: f32,
) -> Svgr<'static> {
    if opacity <= 0.0 || content.is_empty() {
        return nothing();
    }
    let shadow = |dx: f32, dy: f32, alpha: f32| {
        text::text_node(
            ctx,
            content,
            x + dx,
            y + SHADOW_DY + dy,
            family.to_owned(),
            LABEL_SIZE,
            LABEL_WEIGHT,
            0.0,
            "#000000".to_owned(),
            alpha * opacity,
        )
    };
    let mut nodes = vec![shadow(0.0, 0.0, SHADOW_CORE_ALPHA)];
    for (dx, dy) in blur::ghost_offsets(SHADOW_BLUR, 1.0) {
        nodes.push(shadow(dx, dy, SHADOW_RING_ALPHA));
    }
    nodes.push(text::text_node(
        ctx,
        content,
        x,
        y,
        family.to_owned(),
        LABEL_SIZE,
        LABEL_WEIGHT,
        0.0,
        LABEL_INK.to_owned(),
        opacity,
    ));
    Svgr::from(nodes)
}

/// How far each world copy is stretched past its right edge, in plate px. One px hides the seam
/// where neighbouring copies meet; over a world thousands of px wide the stretch is invisible.
const SEAM_OVERLAP_PX: f64 = 1.0;

/// The basemap: the world image, repeated beside itself where those copies are visible.
fn basemap(plate: &MapPlate, view: [f64; 2], view_y: [f64; 2]) -> Svgr<'static> {
    let world = world_size(plate);
    let center = from_lng_lat(plate.center[0], plate.center[1]);
    let left = -center.x * world + plate.width / 2.0;
    let top = -center.y * world + plate.height / 2.0;
    if top + world < view_y[0].max(0.0) || top > view_y[1].min(plate.height) {
        return nothing();
    }
    let images: Vec<Svgr<'static>> = visible_copies(left, world, view, plate.width)
        .into_iter()
        .map(|copy| {
            let x = (left + f64::from(copy) * world) as f32;
            // Each copy runs [`SEAM_OVERLAP_PX`] past its right edge, under the next copy, so the
            // two anti-aliased edges never leave a sliver of background at the antimeridian.
            let width = (world + SEAM_OVERLAP_PX) as f32;
            fframes::svgr!(
                <image href={IMAGE_NAME} x={x} y={top as f32} width={width}
                       height={world as f32} preserveAspectRatio="none" />
            )
        })
        .collect();
    fframes::svgr!(
        <g clip-path="url(#map-flyover-plate)">{Svgr::from(images)}</g>
    )
}

impl Component for MapFlyover {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        Some(BACKGROUND)
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let family = config
            .font_family
            .clone()
            .unwrap_or_else(|| crate::INTER_FAMILY.to_owned());
        let frame = ctx.frame();
        let (w, h) = (ctx.width(), ctx.height());

        let route = flyover_route(config.origin, config.destination);
        let distance = route_length_km(&route);
        let plate = calculate_map_plate(f64::from(w), f64::from(h), &route, distance);
        let projected = ProjectedRoute::new(&route, &plate);

        // The camera: a pure translate that keeps the route's head centred.
        let progress = travel_progress(frame, config.duration);
        let (cx, cy) = projected.point_at_progress(f64::from(progress));
        let (shift_x, shift_y) = (f64::from(w) / 2.0 - cx, f64::from(h) / 2.0 - cy);
        let camera = format!("translate({} {})", shift_x as f32, shift_y as f32);
        // The part of the plate the canvas shows.
        let view = [-shift_x, -shift_x + f64::from(w)];
        let view_y = [-shift_y, -shift_y + f64::from(h)];

        let radius = marker_radius(config.line_width);
        let center_radius = radius * CENTER_RATIO;
        let (start, end) = (projected.start(), projected.end());
        let (sx, sy) = (start.x as f32, start.y as f32);
        let (ex, ey) = (end.x as f32, end.y as f32);
        let route_color = config.route_color.clone();

        let path = projected.path_to(f64::from(progress));
        let line = if path.is_empty() {
            nothing()
        } else {
            fframes::svgr!(
                <path d={path} fill="none" stroke={route_color.clone()}
                      stroke-width={config.line_width} stroke-linecap="round"
                      stroke-linejoin="round" />
            )
        };

        let end_marker = if frame >= travel_end(config.duration) + MARKER_FRAMES[0] {
            let outer = destination_marker_scale(frame, config.duration, config.line_width);
            let inner = destination_center_scale(frame, config.duration);
            let dot = if inner > 0.0 {
                fframes::svgr!(<circle r={center_radius * inner} fill="#ffffff" />)
            } else {
                nothing()
            };
            fframes::svgr!(
                <g transform={format!("translate({ex} {ey})")}>
                    <circle r={radius * outer} fill={route_color.clone()} />
                    {dot}
                </g>
            )
        } else {
            nothing()
        };

        let origin_label = label(
            ctx,
            &config.origin_label,
            sx,
            sy + radius + LABEL_DROP,
            &family,
            origin_label_opacity(frame),
        );
        let destination_label = label(
            ctx,
            &config.destination_label,
            ex,
            ey + radius + LABEL_DROP,
            &family,
            destination_label_opacity(frame, config.duration),
        );

        // The vignette: `radial-gradient(circle, ...)` reaches the farthest corner at 100%.
        let reach = (w / 2.0).hypot(h / 2.0) * VIGNETTE_END;
        let clear = VIGNETTE_START / VIGNETTE_END;

        // The attribution badge, bottom right.
        let badge_text_w = ctx.text_width_at(&family, BADGE_WEIGHT, BADGE_SIZE, ATTRIBUTION);
        let line_h = BADGE_SIZE * INTER_LINE_NORMAL;
        let badge_w = badge_text_w + 2.0 * BADGE_PAD_X;
        let badge_h = line_h + 2.0 * BADGE_PAD_Y;
        let badge_x = w - BADGE_INSET - badge_w;
        let badge_y = h - BADGE_INSET - badge_h;
        let badge_base = badge_y
            + BADGE_PAD_Y
            + ctx.baseline_offset_at(&family, BADGE_WEIGHT, BADGE_SIZE, line_h);
        let badge_label = text::text_node_left(
            ATTRIBUTION,
            badge_x + BADGE_PAD_X,
            badge_base,
            family,
            BADGE_SIZE,
            BADGE_WEIGHT,
            0.0,
            BADGE_INK.to_owned(),
            1.0,
        );

        fframes::svgr!(
            <g>
                <defs>
                    <clipPath id="map-flyover-plate">
                        <rect x="0" y="0" width={plate.width as f32} height={plate.height as f32} />
                    </clipPath>
                    <radialGradient id="map-flyover-vignette" gradientUnits="userSpaceOnUse"
                                    cx={w / 2.0} cy={h / 2.0} r={reach}>
                        <stop offset="0" stop-color={VIGNETTE_INK} stop-opacity="0" />
                        <stop offset={clear} stop-color={VIGNETTE_INK} stop-opacity="0" />
                        <stop offset="1" stop-color={VIGNETTE_INK} stop-opacity={VIGNETTE_OPACITY} />
                    </radialGradient>
                </defs>
                <g transform={camera}>
                    {basemap(&plate, view, view_y)}
                    {line}
                    <circle cx={sx} cy={sy} r={radius} fill={route_color} />
                    {origin_label}
                    {end_marker}
                    {destination_label}
                </g>
                <rect x="0" y="0" width={w} height={h} fill="url(#map-flyover-vignette)" />
                <rect x={badge_x} y={badge_y} width={badge_w} height={badge_h}
                      rx={BADGE_RADIUS} fill="#ffffff" fill-opacity={BADGE_OPACITY} />
                {badge_label}
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONDON: [f64; 2] = [-0.1276, 51.5072];
    const TOKYO: [f64; 2] = [139.6917, 35.6895];

    fn default_plate() -> (Vec<[f64; 2]>, MapPlate) {
        let route = flyover_route(LONDON, TOKYO);
        let distance = route_length_km(&route);
        let plate = calculate_map_plate(1920.0, 1080.0, &route, distance);
        (route, plate)
    }

    #[test]
    fn the_defaults_are_the_documented_config() {
        let config = MapFlyoverConfig::default();
        assert_eq!(config.origin, LONDON);
        assert_eq!(config.destination, TOKYO);
        assert_eq!(config.origin_label, "London");
        assert_eq!(config.destination_label, "Tokyo");
        assert_eq!(config.route_color, "#ff5c4d");
        assert_eq!(config.line_width, 16.0);
        assert_eq!(config.duration, 240);
        assert!((MIN_LINE_WIDTH..=MAX_LINE_WIDTH).contains(&config.line_width));
    }

    #[test]
    fn the_meta_names_the_component_and_its_feature() {
        let component = MapFlyover::default();
        assert_eq!(component.meta().id, "map-flyover");
        assert_eq!(MapFlyover::meta_static().feature, "map-flyover");
        assert_eq!(component.duration(), 240);
        assert_eq!(component.background(), Some(BACKGROUND));
    }

    #[test]
    fn mercator_matches_maplibre_at_known_points() {
        let origin = from_lng_lat(0.0, 0.0);
        assert!((origin.x - 0.5).abs() < 1e-12 && (origin.y - 0.5).abs() < 1e-12);
        let corner = from_lng_lat(-180.0, 85.0511287798066);
        assert!(corner.x.abs() < 1e-12 && corner.y.abs() < 1e-9);
        // Unwrapped longitudes run past the first world.
        assert!((from_lng_lat(270.0, 0.0).x - 1.25).abs() < 1e-12);
    }

    #[test]
    fn mercator_round_trips() {
        for [lng, lat] in [LONDON, TOKYO, [-122.4, 37.8], [241.76, 34.05], [0.0, -60.0]] {
            let back = to_lng_lat(from_lng_lat(lng, lat));
            assert!((back[0] - lng).abs() < 1e-9, "lng {lng}");
            assert!((back[1] - lat).abs() < 1e-9, "lat {lat}");
        }
    }

    #[test]
    fn the_great_circle_has_200_points_and_exact_ends() {
        let points = great_circle(LONDON, TOKYO, ROUTE_POINTS);
        assert_eq!(points.len(), 200);
        assert_eq!(points[0], LONDON);
        assert_eq!(points[199], TOKYO);
        // It bows north of both ends on the way to Tokyo.
        assert!(points.iter().any(|p| p[1] > 60.0));
    }

    #[test]
    fn the_great_circle_follows_the_equator_and_the_meridians() {
        let along_equator = great_circle([0.0, 0.0], [90.0, 0.0], 3);
        assert!((along_equator[1][0] - 45.0).abs() < 1e-9 && along_equator[1][1].abs() < 1e-9);
        let along_meridian = great_circle([10.0, 0.0], [10.0, 60.0], 3);
        assert!((along_meridian[1][0] - 10.0).abs() < 1e-9);
        assert!((along_meridian[1][1] - 30.0).abs() < 1e-9);
    }

    #[test]
    fn longitudes_unwrap_across_the_antimeridian() {
        assert_eq!(unwrap_longitude(-179.0, 179.0), 181.0);
        assert_eq!(unwrap_longitude(179.0, -179.0), -181.0);
        assert_eq!(unwrap_longitude(10.0, 0.0), 10.0);
        assert_eq!(unwrap_longitude(-170.0, 540.0), 550.0);
    }

    #[test]
    fn a_route_across_the_pacific_keeps_going_past_180() {
        let route = flyover_route(TOKYO, [-118.2437, 34.0522]);
        assert_eq!(route.len(), 200);
        assert!(route.windows(2).all(|w| w[1][0] >= w[0][0]));
        assert!((route[199][0] - (360.0 - 118.2437)).abs() < 1e-9);
        assert!(route.iter().any(|p| p[0] > 180.0));
    }

    #[test]
    fn the_reduced_bend_keeps_the_ends_and_straightens_the_middle() {
        let route = flyover_route(LONDON, TOKYO);
        assert!((route[0][0] - LONDON[0]).abs() < 1e-9 && (route[0][1] - LONDON[1]).abs() < 1e-9);
        assert!(
            (route[199][0] - TOKYO[0]).abs() < 1e-9 && (route[199][1] - TOKYO[1]).abs() < 1e-9
        );
        let full = great_circle(LONDON, TOKYO, 200);
        let mid_full = from_lng_lat(full[100][0], full[100][1]);
        let mid_route = from_lng_lat(route[100][0], route[100][1]);
        let a = from_lng_lat(LONDON[0], LONDON[1]);
        let b = from_lng_lat(TOKYO[0], TOKYO[1]);
        let straight_y = a.y + (b.y - a.y) * (100.0 / 199.0);
        // Half way between the great circle and the straight line.
        assert!((mid_route.y - (straight_y + (mid_full.y - straight_y) * 0.5)).abs() < 1e-9);
        assert!(mid_route.y > mid_full.y && mid_route.y < straight_y);
    }

    #[test]
    fn a_degenerate_route_is_a_stub() {
        let route = flyover_route(LONDON, LONDON);
        assert_eq!(route, vec![LONDON, [LONDON[0] + 0.0001, LONDON[1]]]);
    }

    #[test]
    fn haversine_matches_the_london_tokyo_distance() {
        let km = haversine_km(LONDON, TOKYO);
        assert!((km - 9559.0).abs() < 15.0, "{km}");
        assert!((haversine_km([0.0, 0.0], [0.0, 1.0]) - 111.195).abs() < 0.01);
        // The route is longer than the direct arc only by its bend.
        let route = route_length_km(&flyover_route(LONDON, TOKYO));
        assert!(route > km && route < km * 1.3, "{route}");
    }

    #[test]
    fn the_default_plate_matches_the_source_maths() {
        let (route, plate) = default_plate();
        assert_eq!((plate.width, plate.height), (4096.0, 2304.0));
        println!("default plate zoom {}", plate.zoom);
        assert!(plate.zoom > 0.5 && plate.zoom < 3.5, "{}", plate.zoom);
        // Every route point lies on the plate.
        let projected = ProjectedRoute::new(&route, &plate);
        for p in &projected.points {
            assert!(p.x >= 0.0 && p.x <= plate.width, "x {}", p.x);
            assert!(p.y >= 0.0 && p.y <= plate.height, "y {}", p.y);
        }
        // The plate is centred on the route's bounding box.
        let min_x = projected.points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        let max_x = projected.points.iter().map(|p| p.x).fold(f64::MIN, f64::max);
        assert!(((min_x + max_x) / 2.0 - plate.width / 2.0).abs() < 1e-6);
    }

    #[test]
    fn a_short_route_zooms_in_further() {
        let route = flyover_route([-0.1276, 51.5072], [2.3522, 48.8566]);
        let km = route_length_km(&route);
        let short = calculate_map_plate(1920.0, 1080.0, &route, km);
        assert!(km < 400.0);
        assert!(short.zoom > default_plate().1.zoom + 2.0, "{}", short.zoom);
    }

    #[test]
    fn projection_puts_the_plate_centre_at_the_middle() {
        let (_, plate) = default_plate();
        let (x, y) = project_to_plate(plate.center, &plate);
        assert!((x - 2048.0).abs() < 1e-6 && (y - 1152.0).abs() < 1e-6);
        // One zoom step doubles the distance from the centre.
        let double = MapPlate {
            zoom: plate.zoom + 1.0,
            ..plate
        };
        let (x1, _) = project_to_plate(LONDON, &plate);
        let (x2, _) = project_to_plate(LONDON, &double);
        assert!(((x2 - 2048.0) - 2.0 * (x1 - 2048.0)).abs() < 1e-6);
    }

    #[test]
    fn the_arc_length_lookup_walks_the_route_by_distance() {
        let route = ProjectedRoute::from_points([(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        assert_eq!(route.length(), 20.0);
        assert_eq!(route.point_at_progress(0.0), (0.0, 0.0));
        assert_eq!(route.point_at_progress(0.25), (5.0, 0.0));
        assert_eq!(route.point_at_progress(0.5), (10.0, 0.0));
        assert_eq!(route.point_at_progress(0.75), (10.0, 5.0));
        assert_eq!(route.point_at_progress(1.0), (10.0, 10.0));
    }

    #[test]
    fn the_path_is_cut_at_the_arc_length() {
        let route = ProjectedRoute::from_points([(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        assert_eq!(route.path_to(0.0), "");
        assert_eq!(route.path_to(0.25), "M 0 0 L 5 0");
        assert_eq!(route.path_to(0.75), "M 0 0 L 10 0 L 10 5");
        assert_eq!(route.path_to(1.0), "M 0 0 L 10 0 L 10 10");
    }

    #[test]
    fn world_copies_cover_only_what_is_visible() {
        assert_eq!(visible_copies(0.0, 4096.0, [100.0, 1000.0], 4096.0), vec![0]);
        // The world starts right of the view's left edge: the copy before it shows too.
        assert_eq!(
            visible_copies(500.0, 4096.0, [0.0, 1000.0], 4096.0),
            vec![-1, 0]
        );
        // Past the plate's right edge nothing is drawn.
        assert!(visible_copies(0.0, 4096.0, [5000.0, 6000.0], 4096.0).is_empty());
        // A small world repeats.
        assert_eq!(
            visible_copies(100.0, 400.0, [0.0, 1000.0], 4096.0),
            vec![-1, 0, 1, 2]
        );
    }

    #[test]
    fn travel_is_an_ease_in_out_over_all_but_the_last_80_frames() {
        assert_eq!(travel_end(240), 160.0);
        assert_eq!(travel_end(50), 1.0);
        assert_eq!(travel_progress(0.0, 240), 0.0);
        assert!((travel_progress(80.0, 240) - 0.5).abs() < 1e-6);
        assert_eq!(travel_progress(160.0, 240), 1.0);
        assert_eq!(travel_progress(239.0, 240), 1.0);
        assert!((travel_progress(40.0, 240) - 0.125).abs() < 1e-6);
    }

    #[test]
    fn the_markers_derive_from_the_line_width() {
        assert!((marker_radius(16.0) - 14.0).abs() < 1e-6);
        assert!((marker_radius(24.0) - 19.2).abs() < 1e-5);
        assert_eq!(marker_radius(2.0), 14.0);
        // The destination marker starts at the line's own cap.
        let start = destination_marker_scale(165.0, 240, 16.0);
        assert!((start * marker_radius(16.0) - 8.0).abs() < 1e-4);
        assert_eq!(destination_marker_scale(177.0, 240, 16.0), 1.0);
    }

    #[test]
    fn the_arrival_runs_marker_then_dot_then_label() {
        assert_eq!(origin_label_opacity(0.0), 1.0);
        assert!((origin_label_opacity(7.0) - 0.5).abs() < 1e-6);
        assert_eq!(origin_label_opacity(14.0), 0.0);
        assert_eq!(destination_center_scale(178.0, 240), 0.0);
        assert!(destination_center_scale(184.0, 240) > 0.0);
        assert_eq!(destination_center_scale(189.0, 240), 1.0);
        assert_eq!(destination_label_opacity(186.0, 240), 0.0);
        assert!(destination_label_opacity(190.0, 240) > 0.0);
        assert_eq!(destination_label_opacity(195.0, 240), 1.0);
    }

    #[test]
    fn the_basemap_is_the_embedded_4096_square_jpeg() {
        let media = crate::ComponentsMedia::new().expect("embedded media");
        let image = media.blue_marble_z4_jpg;
        assert_eq!(image.metadata.width, 4096);
        assert_eq!(image.metadata.height, 4096);
        assert_eq!(IMAGE_NAME, "blue-marble-z4.jpg");
        assert_eq!(IMAGE_SIZE, 4096.0);
    }
}
