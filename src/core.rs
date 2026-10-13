//! The component trait, the catalog, and the pieces every component shares.

use std::marker::PhantomData;

use fframes::{AudioMap, Color, Duration, FFramesContext, Frame, Svgr, Video};

use crate::ctx::{ComponentFrame, RenderCtx};

/// A video canvas: the geometry a [`ComponentsVideo`] renders at.
///
/// fframes reads the render target from the `Video` trait's **associated constants**, which are
/// per-type, so the canvas cannot be a runtime field. It is a type parameter instead: pick
/// [`Landscape`] (1920x1080) or [`Portrait`] (1080x1920), or implement this trait for a canvas of
/// your own.
///
/// Every component lays itself out against whatever geometry the video reports, so a component
/// renders correctly on any canvas that implements this trait.
pub trait Canvas: std::fmt::Debug + Clone + Copy + PartialEq + Eq + Sync + Send + 'static {
    /// Canvas width in pixels.
    const WIDTH: usize;
    /// Canvas height in pixels.
    const HEIGHT: usize;
    /// Frames per second.
    const FPS: usize = 30;
    /// Human-readable name, used by `--list` and the timeline report.
    const LABEL: &'static str;
}

/// Landscape 16:9 — 1920x1080 at 30 fps. The default canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Landscape;

impl Canvas for Landscape {
    const WIDTH: usize = 1920;
    const HEIGHT: usize = 1080;
    const FPS: usize = 30;
    const LABEL: &'static str = "landscape 16:9 (1920x1080)";
}

/// Portrait 9:16 — 1080x1920 at 30 fps. For stories, reels and chat-style components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Portrait;

impl Canvas for Portrait {
    const WIDTH: usize = 1080;
    const HEIGHT: usize = 1920;
    const FPS: usize = 30;
    const LABEL: &'static str = "portrait 9:16 (1080x1920)";
}

/// Canonical canvas width of the default [`Landscape`] video.
pub const CANVAS_WIDTH: usize = <Landscape as Canvas>::WIDTH;
/// Canonical canvas height of the default [`Landscape`] video.
pub const CANVAS_HEIGHT: usize = <Landscape as Canvas>::HEIGHT;
/// Frame rate of every built-in canvas.
pub const CANVAS_FPS: usize = <Landscape as Canvas>::FPS;

/// Canvas and font settings shared by every component in a video.
#[derive(Debug, Clone, PartialEq)]
pub struct Fonts {
    /// Family used for the sans-serif components (headlines, chat, UI chrome).
    pub sans: String,
    /// Family used for the monospaced components (typewriter, code block, matrix).
    pub mono: String,
    /// Weight used when a component asks for a "medium" face.
    pub weight_medium: u16,
    /// Weight used when a component asks for a "bold" face.
    pub weight_bold: u16,
    /// Weight used when a component asks for a "black"/heavy face.
    pub weight_black: u16,
}

impl Default for Fonts {
    fn default() -> Self {
        // fframes resolves fonts by the family name inside the font file, not the file name.
        //
        // `mono` deliberately points at the same face as `sans`. The crate embeds only DM Sans,
        // which is not monospaced, so the mono components (type-writer, multi-text-typewriter,
        // typing-code-block, glitch-cycle, matrix-rain) are laid out in a proportional face by
        // default. To give them a real mono face, drop a permissively licensed `.ttf` into the
        // host crate's `media/` directory, include it in that crate's media provider, and set
        // `mono` to the family name inside the file:
        //
        //     let video = video.with_fonts(Fonts { mono: "JetBrains Mono".to_owned(), ..Default::default() });
        //
        // Measurement follows `mono` automatically (`TextMeters::for_family` picks the mono meter
        // when the family matches), so what is measured stays what is drawn.
        Self {
            sans: "DM Sans".to_owned(),
            mono: "DM Sans".to_owned(),
            weight_medium: 500,
            weight_bold: 700,
            weight_black: 800,
        }
    }
}

/// Static description of a component, used by the CLI and the generated docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentMeta {
    /// Stable kebab-case id used by `--components`.
    pub id: &'static str,
    /// Human-readable name.
    pub name: &'static str,
    /// One-line description of what the component does.
    pub summary: &'static str,
    /// The Cargo feature that compiles this component in.
    pub feature: &'static str,
}

/// One animated component that can render a single frame.
///
/// Implementors are plain data (the component's config) plus a pure `show` function: given the
/// local frame, return the SVG tree. Implementations must not perform I/O or panic.
///
/// The elided lifetimes on `show` tie the returned tree to `&self`, which is what lets a
/// component return SVG borrowing from its own config. The render context is a separate,
/// short-lived borrow.
pub trait Component: std::fmt::Debug + Sync + Send {
    /// Static metadata for this component.
    fn meta(&self) -> ComponentMeta;

    /// Total frames this component occupies.
    fn duration(&self) -> usize;

    /// Background painted behind the component, or `None` to inherit the video background.
    fn background(&self) -> Option<&'static str> {
        None
    }

    /// Render the frame at `ctx.info.local`.
    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_>;
}

/// Builds a component for a video that holds it.
///
/// This deliberately carries no lifetime parameter: the catalog is a `'static` registry, while
/// the components it produces live for as long as the video that owns them.
pub trait ComponentFactory: std::fmt::Debug + Sync + Send {
    /// Static metadata for the component this factory builds.
    fn meta(&self) -> ComponentMeta;

    /// Build the component with its default configuration.
    fn build(&self) -> Box<dyn Component>;
}

/// A built-in component in the catalog: its metadata plus a build function.
///
/// The constructor is stored as a plain `fn`, which is all a `'static` catalog needs.
#[derive(Debug)]
pub struct CatalogEntry {
    /// Produces the component's static metadata.
    pub meta_fn: fn() -> ComponentMeta,
    /// Builds the component with its default configuration.
    pub build_fn: fn() -> Box<dyn Component>,
}

impl CatalogEntry {
    /// Static metadata for this component.
    pub fn meta(&self) -> ComponentMeta {
        (self.meta_fn)()
    }

    /// Build the component with its default configuration.
    pub fn build(&self) -> Box<dyn Component> {
        (self.build_fn)()
    }
}

impl ComponentFactory for CatalogEntry {
    fn meta(&self) -> ComponentMeta {
        (self.meta_fn)()
    }

    fn build(&self) -> Box<dyn Component> {
        CatalogEntry::build(self)
    }
}

/// Every component compiled into this build, in catalog order.
pub fn catalog() -> &'static [&'static CatalogEntry] {
    crate::catalog::REGISTRY
}

/// Look a component up by id (case-insensitive).
pub fn find(id: &str) -> Option<&'static CatalogEntry> {
    let wanted = id.trim().to_ascii_lowercase();
    crate::catalog::REGISTRY
        .iter()
        .copied()
        .find(|entry| entry.meta().id.eq_ignore_ascii_case(&wanted))
}

/// Error returned when a component selection cannot be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownComponent(pub String);

impl std::fmt::Display for UnknownComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unknown component \"{}\" (this build has {} components compiled in; run with \
             --list to see them)",
            self.0,
            catalog().len()
        )
    }
}

impl std::error::Error for UnknownComponent {}

/// Resolve a comma-separated selection into catalog entries, in catalog order.
///
/// `spec` accepts:
/// - `all` or an empty string — every compiled-in component,
/// - one or more exact ids, e.g. `type-writer,text-reveal` (case-insensitive).
///
/// The result is always in catalog order regardless of the order given, so the same spec always
/// produces the same video.
pub fn selection(spec: &str) -> Result<Vec<&'static CatalogEntry>, UnknownComponent> {
    let trimmed = spec.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        return Ok(catalog().to_vec());
    }

    let mut chosen: Vec<&'static CatalogEntry> = Vec::new();
    for part in trimmed.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let entry = find(part).ok_or_else(|| UnknownComponent(part.to_owned()))?;
        if !chosen.iter().any(|e| e.meta().id == entry.meta().id) {
            chosen.push(entry);
        }
    }

    let order = catalog();
    chosen.sort_by_key(|entry| {
        order
            .iter()
            .position(|e| e.meta().id == entry.meta().id)
            .unwrap_or(usize::MAX)
    });

    Ok(chosen)
}

/// Where a component sits on the video timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    /// First frame of the component (inclusive).
    pub start: usize,
    /// Past-the-end frame of the component (exclusive).
    pub end: usize,
    /// Index into the component list.
    pub index: usize,
}

impl Slot {
    /// Frames this slot covers.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether the slot covers no frames.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A video assembled from a selection of components, played back to back.
///
/// Every selected component becomes one segment: it renders its own frames with a local frame
/// index starting at 0, then the next component starts.
///
/// # Canvas
///
/// `C` is the canvas. Use the [`Horizontal`] / [`Vertical`] aliases:
///
/// ```no_run
/// use fframes_components::{ComponentsVideo, Horizontal, Vertical};
///
/// let wide: Horizontal = ComponentsVideo::from_spec("text-messages").unwrap();
/// let tall: Vertical = ComponentsVideo::from_spec("text-messages").unwrap();
/// ```
///
/// Components lay themselves out against whatever geometry the video reports, so the same
/// selection renders on either canvas. Use `--scale` on the CLI for a smaller output file.
#[derive(Debug)]
pub struct ComponentsVideo<C: Canvas = Landscape> {
    components: Vec<Box<dyn Component>>,
    slots: Vec<Slot>,
    fonts: Fonts,
    meters: crate::ctx::TextMeters,
    canvas: PhantomData<C>,
}

/// The default landscape 16:9 canvas (1920x1080) assembled video.
pub type Horizontal = ComponentsVideo<Landscape>;
/// The portrait 9:16 canvas (1080x1920) assembled video.
pub type Vertical = ComponentsVideo<Portrait>;

impl<C: Canvas> ComponentsVideo<C> {
    /// Build a video from already-constructed components on this canvas.
    ///
    /// The font meters are built from the faces embedded in this crate, so measurement is
    /// available to every component without the host configuring anything.
    pub fn from_components(components: Vec<Box<dyn Component>>) -> Self {
        let mut slots = Vec::with_capacity(components.len());
        let mut cursor = 0;
        for (index, component) in components.iter().enumerate() {
            let frames = component.duration().max(1);
            slots.push(Slot {
                start: cursor,
                end: cursor + frames,
                index,
            });
            cursor += frames;
        }
        Self {
            components,
            slots,
            fonts: Fonts::default(),
            meters: crate::embedded_meters(),
            canvas: PhantomData,
        }
    }

    /// Build from a `--components` style selection string on this canvas.
    ///
    /// ```no_run
    /// use fframes_components::{ComponentsVideo, Vertical};
    ///
    /// let tall: Vertical = ComponentsVideo::from_spec("text-messages").unwrap();
    /// ```
    pub fn from_spec(spec: &str) -> Result<Self, UnknownComponent> {
        let entries = selection(spec)?;
        let components = entries.iter().map(|entry| entry.build()).collect();
        Ok(Self::from_components(components))
    }

    /// Override the font families used by the components.
    pub fn with_fonts(mut self, fonts: Fonts) -> Self {
        self.fonts = fonts;
        self
    }

    /// The selected components, in playback order.
    pub fn components(&self) -> &[Box<dyn Component>] {
        &self.components
    }

    /// The resolved timeline.
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    /// Total frames of the assembled video.
    pub fn total_frames(&self) -> usize {
        self.slots.last().map_or(0, |slot| slot.end)
    }

    /// The font settings in use.
    pub fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    /// The font meters measurement goes through.
    pub fn meters(&self) -> &crate::ctx::TextMeters {
        &self.meters
    }

    /// Canvas width in pixels.
    pub fn width(&self) -> usize {
        C::WIDTH
    }

    /// Canvas height in pixels.
    pub fn height(&self) -> usize {
        C::HEIGHT
    }

    /// Frame rate.
    pub fn fps(&self) -> usize {
        C::FPS
    }

    /// The slot containing `frame`, if any.
    pub fn slot_at(&self, frame: usize) -> Option<Slot> {
        self.slots
            .iter()
            .find(|slot| frame >= slot.start && frame < slot.end)
            .copied()
    }

    /// Human-readable timeline report, one line per component.
    pub fn timeline_report(&self) -> String {
        let fps = C::FPS;
        let mut out = String::new();
        out.push_str(&format!(
            "{} -- {}x{} @ {} fps, {} frames ({:.2}s), {} components\n",
            C::LABEL,
            C::WIDTH,
            C::HEIGHT,
            fps,
            self.total_frames(),
            self.total_frames() as f32 / fps as f32,
            self.components.len(),
        ));
        for slot in &self.slots {
            let component = &self.components[slot.index];
            out.push_str(&format!(
                "  {:<24} frames {:>5}..{:<5} ({:>5.2}s..{:<5.2}s)  {}\n",
                component.meta().id,
                slot.start,
                slot.end,
                slot.start as f32 / fps as f32,
                slot.end as f32 / fps as f32,
                component.meta().summary,
            ));
        }
        out
    }

    /// Render the frame at global index `global`.
    ///
    /// Both canvases' `Video` impls delegate here, so the drawing code exists once.
    fn render_global<'a>(
        &'a self,
        global: usize,
        frame: Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let Some(slot) = self.slot_at(global) else {
            return Svgr::empty();
        };
        let component = &self.components[slot.index];
        let info = ComponentFrame {
            local: global.saturating_sub(slot.start) as f32,
            global,
            fps: C::FPS as f32,
            duration: slot.len(),
            width: C::WIDTH as f32,
            height: C::HEIGHT as f32,
        };
        let mut frame = frame;
        let mut render_ctx = RenderCtx::new(&mut frame, self.fonts.clone(), &self.meters, info);
        let body = component.show(&mut render_ctx);

        let background = component.background().unwrap_or("#000000");
        let view_box = format!("0 0 {} {}", C::WIDTH, C::HEIGHT);
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox={view_box}
                 width={C::WIDTH} height={C::HEIGHT}>
                <rect x="0" y="0" width={C::WIDTH} height={C::HEIGHT} fill={background} />
                {body}
            </svg>
        )
    }
}

// The `Video` trait reads its geometry from associated constants, which are per-type, so each
// canvas needs its own impl. Both delegate to `render_global`; only the constants differ.
macro_rules! impl_video_for_canvas {
    ($canvas:ty) => {
        impl Video for ComponentsVideo<$canvas> {
            const FPS: usize = <$canvas as Canvas>::FPS;
            const WIDTH: usize = <$canvas as Canvas>::WIDTH;
            const HEIGHT: usize = <$canvas as Canvas>::HEIGHT;
            const BACKGROUND_COLOR: Color = Color::BLACK;

            fn duration(&self) -> Duration<'_> {
                Duration::Frames(self.total_frames().max(1))
            }

            fn audio(&self) -> AudioMap<'_> {
                AudioMap::none()
            }

            fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
                let global = frame.global_index;
                self.render_global(global, frame, ctx)
            }
        }
    };
}

impl_video_for_canvas!(Landscape);
impl_video_for_canvas!(Portrait);
