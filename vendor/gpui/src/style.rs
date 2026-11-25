use std::{
    hash::{Hash, Hasher},
    iter, mem,
    ops::Range,
};

use crate::{
    AbsoluteLength, App, Background, BackgroundTag, BorderStyle, Bounds, ContentMask, Corners,
    CornersRefinement, CursorStyle, DefiniteLength, DevicePixels, Edges, EdgesRefinement, Font,
    FontFallbacks, FontFeatures, FontStyle, FontWeight, GridLocation, Hsla, Length, Pixels, Point,
    PointRefinement, Rgba, SharedString, Size, SizeRefinement, Styled, TextRun, Window, black, phi,
    point, quad, rems, size,
};
use collections::HashSet;
use refineable::Refineable;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(debug_assertions)]
pub struct DebugBelow;

#[cfg(debug_assertions)]
impl crate::Global for DebugBelow {}

pub enum ObjectFit {
    Fill,
    Contain,
    Cover,
    ScaleDown,
    None,
}

impl ObjectFit {
    pub fn get_bounds(
        &self,
        bounds: Bounds<Pixels>,
        image_size: Size<DevicePixels>,
    ) -> Bounds<Pixels> {
        let image_size = image_size.map(|dimension| Pixels::from(u32::from(dimension)));
        let image_ratio = image_size.width / image_size.height;
        let bounds_ratio = bounds.size.width / bounds.size.height;

        match self {
            ObjectFit::Fill => bounds,
            ObjectFit::Contain => {
                let new_size = if bounds_ratio > image_ratio {
                    size(
                        image_size.width * (bounds.size.height / image_size.height),
                        bounds.size.height,
                    )
                } else {
                    size(
                        bounds.size.width,
                        image_size.height * (bounds.size.width / image_size.width),
                    )
                };

                Bounds {
                    origin: point(
                        bounds.origin.x + (bounds.size.width - new_size.width) / 2.0,
                        bounds.origin.y + (bounds.size.height - new_size.height) / 2.0,
                    ),
                    size: new_size,
                }
            }
            ObjectFit::ScaleDown => {
                if image_size.width > bounds.size.width || image_size.height > bounds.size.height {
                    let new_size = if bounds_ratio > image_ratio {
                        size(
                            image_size.width * (bounds.size.height / image_size.height),
                            bounds.size.height,
                        )
                    } else {
                        size(
                            bounds.size.width,
                            image_size.height * (bounds.size.width / image_size.width),
                        )
                    };

                    Bounds {
                        origin: point(
                            bounds.origin.x + (bounds.size.width - new_size.width) / 2.0,
                            bounds.origin.y + (bounds.size.height - new_size.height) / 2.0,
                        ),
                        size: new_size,
                    }
                } else {
                    let original_size = size(image_size.width, image_size.height);
                    Bounds {
                        origin: point(
                            bounds.origin.x + (bounds.size.width - original_size.width) / 2.0,
                            bounds.origin.y + (bounds.size.height - original_size.height) / 2.0,
                        ),
                        size: original_size,
                    }
                }
            }
            ObjectFit::Cover => {
                let new_size = if bounds_ratio > image_ratio {
                    size(
                        bounds.size.width,
                        image_size.height * (bounds.size.width / image_size.width),
                    )
                } else {
                    size(
                        image_size.width * (bounds.size.height / image_size.height),
                        bounds.size.height,
                    )
                };

                Bounds {
                    origin: point(
                        bounds.origin.x + (bounds.size.width - new_size.width) / 2.0,
                        bounds.origin.y + (bounds.size.height - new_size.height) / 2.0,
                    ),
                    size: new_size,
                }
            }
            ObjectFit::None => Bounds {
                origin: bounds.origin,
                size: image_size,
            },
        }
    }
}

#[derive(Clone, Refineable, Debug)]
#[refineable(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Style {
    pub display: Display,

    pub visibility: Visibility,

    #[refineable]
    pub overflow: Point<Overflow>,
    pub scrollbar_width: AbsoluteLength,
    pub allow_concurrent_scroll: bool,
    pub restrict_scroll_to_axis: bool,

    pub position: Position,
    #[refineable]
    pub inset: Edges<Length>,

    #[refineable]
    pub size: Size<Length>,
    #[refineable]
    pub min_size: Size<Length>,
    #[refineable]
    pub max_size: Size<Length>,
    pub aspect_ratio: Option<f32>,

    #[refineable]
    pub margin: Edges<Length>,
    #[refineable]
    pub padding: Edges<DefiniteLength>,
    #[refineable]
    pub border_widths: Edges<AbsoluteLength>,

    pub align_items: Option<AlignItems>,
    pub align_self: Option<AlignSelf>,
    pub align_content: Option<AlignContent>,
    pub justify_content: Option<JustifyContent>,
    #[refineable]
    pub gap: Size<DefiniteLength>,

    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub flex_basis: Length,
    pub flex_grow: f32,
    pub flex_shrink: f32,

    pub background: Option<Fill>,

    pub border_color: Option<Hsla>,

    pub border_style: BorderStyle,

    #[refineable]
    pub corner_radii: Corners<AbsoluteLength>,

    pub box_shadow: Vec<BoxShadow>,

    pub text: TextStyleRefinement,

    pub mouse_cursor: Option<CursorStyle>,

    pub opacity: Option<f32>,

    pub grid_cols: Option<u16>,

    pub grid_rows: Option<u16>,

    pub grid_location: Option<GridLocation>,

    #[cfg(debug_assertions)]
    pub debug: bool,

    #[cfg(debug_assertions)]
    pub debug_below: bool,
}

impl Styled for StyleRefinement {
    fn style(&mut self) -> &mut StyleRefinement {
        self
    }
}

impl StyleRefinement {
    pub fn grid_location_mut(&mut self) -> &mut GridLocation {
        self.grid_location.get_or_insert_default()
    }
}

#[doc = " The value of the visibility property, similar to the CSS property `visibility`"]
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Visibility {
    #[doc = " The element should be drawn as normal."]
    #[default]
    Visible,
    #[doc = " The element should not be drawn, but should still take up space in the layout."]
    Hidden,
}

#[doc = " The possible values of the box-shadow property"]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BoxShadow {
    #[doc = " What color should the shadow have?"]
    pub color: Hsla,
    #[doc = " How should it be offset from its element?"]
    pub offset: Point<Pixels>,
    #[doc = " How much should the shadow be blurred?"]
    pub blur_radius: Pixels,
    #[doc = " How much should the shadow spread?"]
    pub spread_radius: Pixels,
}

#[doc = " How to handle whitespace in text"]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum WhiteSpace {
    #[doc = " Normal line wrapping when text overflows the width of the element"]
    #[default]
    Normal,
    #[doc = " No line wrapping, text will overflow the width of the element"]
    Nowrap,
}

#[doc = " How to truncate text that overflows the width of the element"]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TextOverflow {
    #[doc = " Truncate the text when it doesn't fit, and represent this truncation by displaying the"]
    #[doc = " provided string."]
    Truncate(SharedString),
}

#[doc = " How to align text within the element"]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TextAlign {
    #[doc = " Align the text to the left of the element"]
    #[default]
    Left,

    #[doc = " Center the text within the element"]
    Center,

    #[doc = " Align the text to the right of the element"]
    Right,
}

#[derive(Refineable, Clone, Debug, PartialEq)]
#[refineable(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TextStyle {
    pub color: Hsla,

    pub font_family: SharedString,

    pub font_features: FontFeatures,

    pub font_fallbacks: Option<FontFallbacks>,

    pub font_size: AbsoluteLength,

    pub line_height: DefiniteLength,

    pub font_weight: FontWeight,

    pub font_style: FontStyle,

    pub background_color: Option<Hsla>,

    pub underline: Option<UnderlineStyle>,

    pub strikethrough: Option<StrikethroughStyle>,

    pub white_space: WhiteSpace,

    pub text_overflow: Option<TextOverflow>,

    pub text_align: TextAlign,

    pub line_clamp: Option<usize>,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            color: black(),
            font_family: ".SystemUIFont".into(),
            font_features: FontFeatures::default(),
            font_fallbacks: None,
            font_size: rems(1.).into(),
            line_height: phi(),
            font_weight: FontWeight::default(),
            font_style: FontStyle::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
            white_space: WhiteSpace::Normal,
            text_overflow: None,
            text_align: TextAlign::default(),
            line_clamp: None,
        }
    }
}

impl TextStyle {
    pub fn highlight(mut self, style: impl Into<HighlightStyle>) -> Self {
        let style = style.into();
        if let Some(weight) = style.font_weight {
            self.font_weight = weight;
        }
        if let Some(style) = style.font_style {
            self.font_style = style;
        }

        if let Some(color) = style.color {
            self.color = self.color.blend(color);
        }

        if let Some(factor) = style.fade_out {
            self.color.fade_out(factor);
        }

        if let Some(background_color) = style.background_color {
            self.background_color = Some(background_color);
        }

        if let Some(underline) = style.underline {
            self.underline = Some(underline);
        }

        if let Some(strikethrough) = style.strikethrough {
            self.strikethrough = Some(strikethrough);
        }

        self
    }

    pub fn font(&self) -> Font {
        Font {
            family: self.font_family.clone(),
            features: self.font_features.clone(),
            fallbacks: self.font_fallbacks.clone(),
            weight: self.font_weight,
            style: self.font_style,
        }
    }

    pub fn line_height_in_pixels(&self, rem_size: Pixels) -> Pixels {
        self.line_height.to_pixels(self.font_size, rem_size).round()
    }

    pub fn to_run(&self, len: usize) -> TextRun {
        TextRun {
            len,
            font: Font {
                family: self.font_family.clone(),
                features: self.font_features.clone(),
                fallbacks: self.font_fallbacks.clone(),
                weight: self.font_weight,
                style: self.font_style,
            },
            color: self.color,
            background_color: self.background_color,
            underline: self.underline,
            strikethrough: self.strikethrough,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct HighlightStyle {
    pub color: Option<Hsla>,

    pub font_weight: Option<FontWeight>,

    pub font_style: Option<FontStyle>,

    pub background_color: Option<Hsla>,

    pub underline: Option<UnderlineStyle>,

    pub strikethrough: Option<StrikethroughStyle>,

    pub fade_out: Option<f32>,
}

impl Eq for HighlightStyle {}

impl Hash for HighlightStyle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.color.hash(state);
        self.font_weight.hash(state);
        self.font_style.hash(state);
        self.background_color.hash(state);
        self.underline.hash(state);
        self.strikethrough.hash(state);
        state.write_u32(u32::from_be_bytes(
            self.fade_out.map(|f| f.to_be_bytes()).unwrap_or_default(),
        ));
    }
}

impl Style {
    pub fn has_opaque_background(&self) -> bool {
        self.background
            .as_ref()
            .is_some_and(|fill| fill.color().is_some_and(|color| !color.is_transparent()))
    }

    pub fn text_style(&self) -> Option<&TextStyleRefinement> {
        if self.text.is_some() {
            Some(&self.text)
        } else {
            None
        }
    }

    pub fn overflow_mask(
        &self,
        bounds: Bounds<Pixels>,
        rem_size: Pixels,
    ) -> Option<ContentMask<Pixels>> {
        match self.overflow {
            Point {
                x: Overflow::Visible,
                y: Overflow::Visible,
            } => None,
            _ => {
                let mut min = bounds.origin;
                let mut max = bounds.bottom_right();

                if self
                    .border_color
                    .is_some_and(|color| !color.is_transparent())
                {
                    min.x += self.border_widths.left.to_pixels(rem_size);
                    max.x -= self.border_widths.right.to_pixels(rem_size);
                    min.y += self.border_widths.top.to_pixels(rem_size);
                    max.y -= self.border_widths.bottom.to_pixels(rem_size);
                }

                let bounds = match (
                    self.overflow.x == Overflow::Visible,
                    self.overflow.y == Overflow::Visible,
                ) {
                    (true, true) => return None,
                    (true, false) => Bounds::from_corners(
                        point(min.x, bounds.origin.y),
                        point(max.x, bounds.bottom_right().y),
                    ),
                    (false, true) => Bounds::from_corners(
                        point(bounds.origin.x, min.y),
                        point(bounds.bottom_right().x, max.y),
                    ),
                    (false, false) => Bounds::from_corners(min, max),
                };

                Some(ContentMask { bounds })
            }
        }
    }

    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        continuation: impl FnOnce(&mut Window, &mut App),
    ) {
        #[cfg(debug_assertions)]
        if self.debug_below {
            cx.set_global(DebugBelow)
        }

        #[cfg(debug_assertions)]
        if self.debug || cx.has_global::<DebugBelow>() {
            window.paint_quad(crate::outline(bounds, crate::red(), BorderStyle::default()));
        }

        let rem_size = window.rem_size();
        let corner_radii = self
            .corner_radii
            .to_pixels(rem_size)
            .clamp_radii_for_quad_size(bounds.size);

        window.paint_shadows(bounds, corner_radii, &self.box_shadow);

        let background_color = self.background.as_ref().and_then(Fill::color);
        if background_color.is_some_and(|color| !color.is_transparent()) {
            let mut border_color = match background_color {
                Some(color) => match color.tag {
                    BackgroundTag::Solid => color.solid,
                    BackgroundTag::LinearGradient => color
                        .colors
                        .first()
                        .map(|stop| stop.color)
                        .unwrap_or_default(),
                    BackgroundTag::PatternSlash => color.solid,
                },
                None => Hsla::default(),
            };
            border_color.a = 0.;
            window.paint_quad(quad(
                bounds,
                corner_radii,
                background_color.unwrap_or_default(),
                Edges::default(),
                border_color,
                self.border_style,
            ));
        }

        continuation(window, cx);

        if self.is_border_visible() {
            let border_widths = self.border_widths.to_pixels(rem_size);
            let max_border_width = border_widths.max();
            let max_corner_radius = corner_radii.max();

            let top_bounds = Bounds::from_corners(
                bounds.origin,
                bounds.top_right() + point(Pixels::ZERO, max_border_width.max(max_corner_radius)),
            );
            let bottom_bounds = Bounds::from_corners(
                bounds.bottom_left() - point(Pixels::ZERO, max_border_width.max(max_corner_radius)),
                bounds.bottom_right(),
            );
            let left_bounds = Bounds::from_corners(
                top_bounds.bottom_left(),
                bottom_bounds.origin + point(max_border_width, Pixels::ZERO),
            );
            let right_bounds = Bounds::from_corners(
                top_bounds.bottom_right() - point(max_border_width, Pixels::ZERO),
                bottom_bounds.top_right(),
            );

            let mut background = self.border_color.unwrap_or_default();
            background.a = 0.;
            let quad = quad(
                bounds,
                corner_radii,
                background,
                border_widths,
                self.border_color.unwrap_or_default(),
                self.border_style,
            );

            window.with_content_mask(Some(ContentMask { bounds: top_bounds }), |window| {
                window.paint_quad(quad.clone());
            });
            window.with_content_mask(
                Some(ContentMask {
                    bounds: right_bounds,
                }),
                |window| {
                    window.paint_quad(quad.clone());
                },
            );
            window.with_content_mask(
                Some(ContentMask {
                    bounds: bottom_bounds,
                }),
                |window| {
                    window.paint_quad(quad.clone());
                },
            );
            window.with_content_mask(
                Some(ContentMask {
                    bounds: left_bounds,
                }),
                |window| {
                    window.paint_quad(quad);
                },
            );
        }

        #[cfg(debug_assertions)]
        if self.debug_below {
            cx.remove_global::<DebugBelow>();
        }
    }

    fn is_border_visible(&self) -> bool {
        self.border_color
            .is_some_and(|color| !color.is_transparent())
            && self.border_widths.any(|length| !length.is_zero())
    }
}

impl Default for Style {
    fn default() -> Self {
        Style {
            display: Display::Block,
            visibility: Visibility::Visible,
            overflow: Point {
                x: Overflow::Visible,
                y: Overflow::Visible,
            },
            allow_concurrent_scroll: false,
            restrict_scroll_to_axis: false,
            scrollbar_width: AbsoluteLength::default(),
            position: Position::Relative,
            inset: Edges::auto(),
            margin: Edges::<Length>::zero(),
            padding: Edges::<DefiniteLength>::zero(),
            border_widths: Edges::<AbsoluteLength>::zero(),
            size: Size::auto(),
            min_size: Size::auto(),
            max_size: Size::auto(),
            aspect_ratio: None,
            gap: Size::default(),
            align_items: None,
            align_self: None,
            align_content: None,
            justify_content: None,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Length::Auto,
            background: None,
            border_color: None,
            border_style: BorderStyle::default(),
            corner_radii: Corners::default(),
            box_shadow: Default::default(),
            text: TextStyleRefinement::default(),
            mouse_cursor: None,
            opacity: None,
            grid_rows: None,
            grid_cols: None,
            grid_location: None,

            #[cfg(debug_assertions)]
            debug: false,
            #[cfg(debug_assertions)]
            debug_below: false,
        }
    }
}

#[doc = " The properties that can be applied to an underline."]
#[derive(
    Refineable, Copy, Clone, Default, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct UnderlineStyle {
    #[doc = " The thickness of the underline."]
    pub thickness: Pixels,

    #[doc = " The color of the underline."]
    pub color: Option<Hsla>,

    #[doc = " Whether the underline should be wavy, like in a spell checker."]
    pub wavy: bool,
}

#[doc = " The properties that can be applied to a strikethrough."]
#[derive(
    Refineable, Copy, Clone, Default, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct StrikethroughStyle {
    #[doc = " The thickness of the strikethrough."]
    pub thickness: Pixels,

    #[doc = " The color of the strikethrough."]
    pub color: Option<Hsla>,
}

#[doc = " The kinds of fill that can be applied to a shape."]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Fill {
    #[doc = " A solid color fill."]
    Color(Background),
}

impl Fill {
    pub fn color(&self) -> Option<Background> {
        match self {
            Fill::Color(color) => Some(*color),
        }
    }
}

impl Default for Fill {
    fn default() -> Self {
        Self::Color(Background::default())
    }
}

impl From<Hsla> for Fill {
    fn from(color: Hsla) -> Self {
        Self::Color(color.into())
    }
}

impl From<Rgba> for Fill {
    fn from(color: Rgba) -> Self {
        Self::Color(color.into())
    }
}

impl From<Background> for Fill {
    fn from(background: Background) -> Self {
        Self::Color(background)
    }
}

impl From<TextStyle> for HighlightStyle {
    fn from(other: TextStyle) -> Self {
        Self::from(&other)
    }
}

impl From<&TextStyle> for HighlightStyle {
    fn from(other: &TextStyle) -> Self {
        Self {
            color: Some(other.color),
            font_weight: Some(other.font_weight),
            font_style: Some(other.font_style),
            background_color: other.background_color,
            underline: other.underline,
            strikethrough: other.strikethrough,
            fade_out: None,
        }
    }
}

impl HighlightStyle {
    pub fn color(color: Hsla) -> Self {
        Self {
            color: Some(color),
            ..Default::default()
        }
    }
    #[must_use]
    pub fn highlight(self, other: HighlightStyle) -> Self {
        Self {
            color: other
                .color
                .map(|other_color| {
                    if let Some(color) = self.color {
                        color.blend(other_color)
                    } else {
                        other_color
                    }
                })
                .or(self.color),
            font_weight: other.font_weight.or(self.font_weight),
            font_style: other.font_style.or(self.font_style),
            background_color: other.background_color.or(self.background_color),
            underline: other.underline.or(self.underline),
            strikethrough: other.strikethrough.or(self.strikethrough),
            fade_out: other
                .fade_out
                .map(|source_fade| {
                    self.fade_out
                        .map(|dest_fade| (dest_fade * (1. + source_fade)).clamp(0., 1.))
                        .unwrap_or(source_fade)
                })
                .or(self.fade_out),
        }
    }
}

impl From<Hsla> for HighlightStyle {
    fn from(color: Hsla) -> Self {
        Self {
            color: Some(color),
            ..Default::default()
        }
    }
}

impl From<FontWeight> for HighlightStyle {
    fn from(font_weight: FontWeight) -> Self {
        Self {
            font_weight: Some(font_weight),
            ..Default::default()
        }
    }
}

impl From<FontStyle> for HighlightStyle {
    fn from(font_style: FontStyle) -> Self {
        Self {
            font_style: Some(font_style),
            ..Default::default()
        }
    }
}

impl From<Rgba> for HighlightStyle {
    fn from(color: Rgba) -> Self {
        Self {
            color: Some(color.into()),
            ..Default::default()
        }
    }
}

pub fn combine_highlights(
    a: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    b: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
) -> impl Iterator<Item = (Range<usize>, HighlightStyle)> {
    let mut endpoints = Vec::new();
    let mut highlights = Vec::new();
    for (range, highlight) in a.into_iter().chain(b) {
        if !range.is_empty() {
            let highlight_id = highlights.len();
            endpoints.push((range.start, highlight_id, true));
            endpoints.push((range.end, highlight_id, false));
            highlights.push(highlight);
        }
    }
    endpoints.sort_unstable_by_key(|(position, _, _)| *position);
    let mut endpoints = endpoints.into_iter().peekable();

    let mut active_styles = HashSet::default();
    let mut ix = 0;
    iter::from_fn(move || {
        while let Some((endpoint_ix, highlight_id, is_start)) = endpoints.peek() {
            let prev_index = mem::replace(&mut ix, *endpoint_ix);
            if ix > prev_index && !active_styles.is_empty() {
                let current_style = active_styles
                    .iter()
                    .fold(HighlightStyle::default(), |acc, highlight_id| {
                        acc.highlight(highlights[*highlight_id])
                    });
                return Some((prev_index..ix, current_style));
            }

            if *is_start {
                active_styles.insert(*highlight_id);
            } else {
                active_styles.remove(highlight_id);
            }
            endpoints.next();
        }
        None
    })
}

#[doc = " Used to control how child nodes are aligned."]
#[doc = " For Flexbox it controls alignment in the cross axis"]
#[doc = " For Grid it controls alignment in the block axis"]
#[doc = ""]
#[doc = " [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/align-items)"]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub enum AlignItems {
    #[doc = " Items are packed toward the start of the axis"]
    Start,
    #[doc = " Items are packed toward the end of the axis"]
    End,
    #[doc = " Items are packed towards the flex-relative start of the axis."]
    #[doc = ""]
    #[doc = " For flex containers with flex_direction RowReverse or ColumnReverse this is equivalent"]
    #[doc = " to End. In all other cases it is equivalent to Start."]
    FlexStart,
    #[doc = " Items are packed towards the flex-relative end of the axis."]
    #[doc = ""]
    #[doc = " For flex containers with flex_direction RowReverse or ColumnReverse this is equivalent"]
    #[doc = " to Start. In all other cases it is equivalent to End."]
    FlexEnd,
    #[doc = " Items are packed along the center of the cross axis"]
    Center,
    #[doc = " Items are aligned such as their baselines align"]
    Baseline,
    #[doc = " Stretch to fill the container"]
    Stretch,
}
pub type JustifyItems = AlignItems;
pub type AlignSelf = AlignItems;
pub type JustifySelf = AlignItems;

#[doc = " Sets the distribution of space between and around content items"]
#[doc = " For Flexbox it controls alignment in the cross axis"]
#[doc = " For Grid it controls alignment in the block axis"]
#[doc = ""]
#[doc = " [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/align-content)"]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub enum AlignContent {
    #[doc = " Items are packed toward the start of the axis"]
    Start,
    #[doc = " Items are packed toward the end of the axis"]
    End,
    #[doc = " Items are packed towards the flex-relative start of the axis."]
    #[doc = ""]
    #[doc = " For flex containers with flex_direction RowReverse or ColumnReverse this is equivalent"]
    #[doc = " to End. In all other cases it is equivalent to Start."]
    FlexStart,
    #[doc = " Items are packed towards the flex-relative end of the axis."]
    #[doc = ""]
    #[doc = " For flex containers with flex_direction RowReverse or ColumnReverse this is equivalent"]
    #[doc = " to Start. In all other cases it is equivalent to End."]
    FlexEnd,
    #[doc = " Items are centered around the middle of the axis"]
    Center,
    #[doc = " Items are stretched to fill the container"]
    Stretch,
    #[doc = " The first and last items are aligned flush with the edges of the container (no gap)"]
    #[doc = " The gap between items is distributed evenly."]
    SpaceBetween,
    #[doc = " The gap between the first and last items is exactly THE SAME as the gap between items."]
    #[doc = " The gaps are distributed evenly"]
    SpaceEvenly,
    #[doc = " The gap between the first and last items is exactly HALF the gap between items."]
    #[doc = " The gaps are distributed evenly in proportion to these ratios."]
    SpaceAround,
}

pub type JustifyContent = AlignContent;

#[doc = " Sets the layout used for the children of this node"]
#[doc = ""]
#[doc = " The default values depends on on which feature flags are enabled. The order of precedence is: Flex, Grid, Block, None."]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub enum Display {
    #[doc = " The children will follow the block layout algorithm"]
    Block,
    #[doc = " The children will follow the flexbox layout algorithm"]
    #[default]
    Flex,
    #[doc = " The children will follow the CSS Grid layout algorithm"]
    Grid,
    #[doc = " The children will not be laid out, and will follow absolute positioning"]
    None,
}

#[doc = " Controls whether flex items are forced onto one line or can wrap onto multiple lines."]
#[doc = ""]
#[doc = " Defaults to [`FlexWrap::NoWrap`]"]
#[doc = ""]
#[doc = " [Specification](https://www.w3.org/TR/css-flexbox-1/#flex-wrap-property)"]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub enum FlexWrap {
    #[doc = " Items will not wrap and stay on a single line"]
    #[default]
    NoWrap,
    #[doc = " Items will wrap according to this item's [`FlexDirection`]"]
    Wrap,
    #[doc = " Items will wrap in the opposite direction to this item's [`FlexDirection`]"]
    WrapReverse,
}

#[doc = " The direction of the flexbox layout main axis."]
#[doc = ""]
#[doc = " There are always two perpendicular layout axes: main (or primary) and cross (or secondary)."]
#[doc = " Adding items will cause them to be positioned adjacent to each other along the main axis."]
#[doc = " By varying this value throughout your tree, you can create complex axis-aligned layouts."]
#[doc = ""]
#[doc = " Items are always aligned relative to the cross axis, and justified relative to the main axis."]
#[doc = ""]
#[doc = " The default behavior is [`FlexDirection::Row`]."]
#[doc = ""]
#[doc = " [Specification](https://www.w3.org/TR/css-flexbox-1/#flex-direction-property)"]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub enum FlexDirection {
    #[doc = " Defines +x as the main axis"]
    #[doc = ""]
    #[doc = " Items will be added from left to right in a row."]
    #[default]
    Row,
    #[doc = " Defines +y as the main axis"]
    #[doc = ""]
    #[doc = " Items will be added from top to bottom in a column."]
    Column,
    #[doc = " Defines -x as the main axis"]
    #[doc = ""]
    #[doc = " Items will be added from right to left in a row."]
    RowReverse,
    #[doc = " Defines -y as the main axis"]
    #[doc = ""]
    #[doc = " Items will be added from bottom to top in a column."]
    ColumnReverse,
}

#[doc = " How children overflowing their container should affect layout"]
#[doc = ""]
#[doc = " In CSS the primary effect of this property is to control whether contents of a parent container that overflow that container should"]
#[doc = " be displayed anyway, be clipped, or trigger the container to become a scroll container. However it also has secondary effects on layout,"]
#[doc = " the main ones being:"]
#[doc = ""]
#[doc = "   - The automatic minimum size Flexbox/CSS Grid items with non-`Visible` overflow is `0` rather than being content based"]
#[doc = "   - `Overflow::Scroll` nodes have space in the layout reserved for a scrollbar (width controlled by the `scrollbar_width` property)"]
#[doc = ""]
#[doc = " In Taffy, we only implement the layout related secondary effects as we are not concerned with drawing/painting. The amount of space reserved for"]
#[doc = " a scrollbar is controlled by the `scrollbar_width` property. If this is `0` then `Scroll` behaves identically to `Hidden`."]
#[doc = ""]
#[doc = " <https://developer.mozilla.org/en-US/docs/Web/CSS/overflow>"]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub enum Overflow {
    #[doc = " The automatic minimum size of this node as a flexbox/grid item should be based on the size of its content."]
    #[doc = " Content that overflows this node *should* contribute to the scroll region of its parent."]
    #[default]
    Visible,
    #[doc = " The automatic minimum size of this node as a flexbox/grid item should be based on the size of its content."]
    #[doc = " Content that overflows this node should *not* contribute to the scroll region of its parent."]
    Clip,
    #[doc = " The automatic minimum size of this node as a flexbox/grid item should be `0`."]
    #[doc = " Content that overflows this node should *not* contribute to the scroll region of its parent."]
    Hidden,
    #[doc = " The automatic minimum size of this node as a flexbox/grid item should be `0`. Additionally, space should be reserved"]
    #[doc = " for a scrollbar. The amount of space reserved is controlled by the `scrollbar_width` property."]
    #[doc = " Content that overflows this node should *not* contribute to the scroll region of its parent."]
    Scroll,
}

#[doc = " The positioning strategy for this item."]
#[doc = ""]
#[doc = " This controls both how the origin is determined for the [`Style::position`] field,"]
#[doc = " and whether or not the item will be controlled by flexbox's layout algorithm."]
#[doc = ""]
#[doc = " WARNING: this enum follows the behavior of [CSS's `position` property](https://developer.mozilla.org/en-US/docs/Web/CSS/position),"]
#[doc = " which can be unintuitive."]
#[doc = ""]
#[doc = " [`Position::Relative`] is the default value, in contrast to the default behavior in CSS."]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub enum Position {
    #[doc = " The offset is computed relative to the final position given by the layout algorithm."]
    #[doc = " Offsets do not affect the position of any other items; they are effectively a correction factor applied at the end."]
    #[default]
    Relative,
    #[doc = " The offset is computed relative to this item's closest positioned ancestor, if any."]
    #[doc = " Otherwise, it is placed relative to the origin."]
    #[doc = " No space is created for the item in the page layout, and its size will not be altered."]
    #[doc = ""]
    #[doc = " WARNING: to opt-out of layouting entirely, you must use [`Display::None`] instead on your [`Style`] object."]
    Absolute,
}

impl From<AlignItems> for taffy::style::AlignItems {
    fn from(value: AlignItems) -> Self {
        match value {
            AlignItems::Start => Self::Start,
            AlignItems::End => Self::End,
            AlignItems::FlexStart => Self::FlexStart,
            AlignItems::FlexEnd => Self::FlexEnd,
            AlignItems::Center => Self::Center,
            AlignItems::Baseline => Self::Baseline,
            AlignItems::Stretch => Self::Stretch,
        }
    }
}

impl From<AlignContent> for taffy::style::AlignContent {
    fn from(value: AlignContent) -> Self {
        match value {
            AlignContent::Start => Self::Start,
            AlignContent::End => Self::End,
            AlignContent::FlexStart => Self::FlexStart,
            AlignContent::FlexEnd => Self::FlexEnd,
            AlignContent::Center => Self::Center,
            AlignContent::Stretch => Self::Stretch,
            AlignContent::SpaceBetween => Self::SpaceBetween,
            AlignContent::SpaceEvenly => Self::SpaceEvenly,
            AlignContent::SpaceAround => Self::SpaceAround,
        }
    }
}

impl From<Display> for taffy::style::Display {
    fn from(value: Display) -> Self {
        match value {
            Display::Block => Self::Block,
            Display::Flex => Self::Flex,
            Display::Grid => Self::Grid,
            Display::None => Self::None,
        }
    }
}

impl From<FlexWrap> for taffy::style::FlexWrap {
    fn from(value: FlexWrap) -> Self {
        match value {
            FlexWrap::NoWrap => Self::NoWrap,
            FlexWrap::Wrap => Self::Wrap,
            FlexWrap::WrapReverse => Self::WrapReverse,
        }
    }
}

impl From<FlexDirection> for taffy::style::FlexDirection {
    fn from(value: FlexDirection) -> Self {
        match value {
            FlexDirection::Row => Self::Row,
            FlexDirection::Column => Self::Column,
            FlexDirection::RowReverse => Self::RowReverse,
            FlexDirection::ColumnReverse => Self::ColumnReverse,
        }
    }
}

impl From<Overflow> for taffy::style::Overflow {
    fn from(value: Overflow) -> Self {
        match value {
            Overflow::Visible => Self::Visible,
            Overflow::Clip => Self::Clip,
            Overflow::Hidden => Self::Hidden,
            Overflow::Scroll => Self::Scroll,
        }
    }
}

impl From<Position> for taffy::style::Position {
    fn from(value: Position) -> Self {
        match value {
            Position::Relative => Self::Relative,
            Position::Absolute => Self::Absolute,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{blue, green, px, red, yellow};

    use super::*;

    use util_macros::perf;

    #[perf]
    fn test_basic_highlight_style_combination() {
        let style_a = HighlightStyle::default();
        let style_b = HighlightStyle::default();
        let style_a = style_a.highlight(style_b);
        assert_eq!(
            style_a,
            HighlightStyle::default(),
            "Combining empty styles should not produce a non-empty style."
        );

        let mut style_b = HighlightStyle {
            color: Some(red()),
            strikethrough: Some(StrikethroughStyle {
                thickness: px(2.),
                color: Some(blue()),
            }),
            fade_out: Some(0.),
            font_style: Some(FontStyle::Italic),
            font_weight: Some(FontWeight(300.)),
            background_color: Some(yellow()),
            underline: Some(UnderlineStyle {
                thickness: px(2.),
                color: Some(red()),
                wavy: true,
            }),
        };
        let expected_style = style_b;

        let style_a = style_a.highlight(style_b);
        assert_eq!(
            style_a, expected_style,
            "Blending an empty style with another style should return the other style"
        );

        let style_b = style_b.highlight(Default::default());
        assert_eq!(
            style_b, expected_style,
            "Blending a style with an empty style should not change the style."
        );

        let mut style_c = expected_style;

        let style_d = HighlightStyle {
            color: Some(blue().alpha(0.7)),
            strikethrough: Some(StrikethroughStyle {
                thickness: px(4.),
                color: Some(crate::red()),
            }),
            fade_out: Some(0.),
            font_style: Some(FontStyle::Oblique),
            font_weight: Some(FontWeight(800.)),
            background_color: Some(green()),
            underline: Some(UnderlineStyle {
                thickness: px(4.),
                color: None,
                wavy: false,
            }),
        };

        let expected_style = HighlightStyle {
            color: Some(red().blend(blue().alpha(0.7))),
            strikethrough: Some(StrikethroughStyle {
                thickness: px(4.),
                color: Some(red()),
            }),
            fade_out: Some(0.),
            font_style: Some(FontStyle::Oblique),
            font_weight: Some(FontWeight(800.)),
            background_color: Some(green()),
            underline: Some(UnderlineStyle {
                thickness: px(4.),
                color: None,
                wavy: false,
            }),
        };

        let style_c = style_c.highlight(style_d);
        assert_eq!(
            style_c, expected_style,
            "Blending styles should blend properties where possible and override all others"
        );
    }

    #[perf]
    fn test_combine_highlights() {
        assert_eq!(
            combine_highlights(
                [
                    (0..5, green().into()),
                    (4..10, FontWeight::BOLD.into()),
                    (15..20, yellow().into()),
                ],
                [
                    (2..6, FontStyle::Italic.into()),
                    (1..3, blue().into()),
                    (21..23, red().into()),
                ]
            )
            .collect::<Vec<_>>(),
            [
                (
                    0..1,
                    HighlightStyle {
                        color: Some(green()),
                        ..Default::default()
                    }
                ),
                (
                    1..2,
                    HighlightStyle {
                        color: Some(blue()),
                        ..Default::default()
                    }
                ),
                (
                    2..3,
                    HighlightStyle {
                        color: Some(blue()),
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    }
                ),
                (
                    3..4,
                    HighlightStyle {
                        color: Some(green()),
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    }
                ),
                (
                    4..5,
                    HighlightStyle {
                        color: Some(green()),
                        font_weight: Some(FontWeight::BOLD),
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    }
                ),
                (
                    5..6,
                    HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    }
                ),
                (
                    6..10,
                    HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        ..Default::default()
                    }
                ),
                (
                    15..20,
                    HighlightStyle {
                        color: Some(yellow()),
                        ..Default::default()
                    }
                ),
                (
                    21..23,
                    HighlightStyle {
                        color: Some(red()),
                        ..Default::default()
                    }
                )
            ]
        );
    }
}
