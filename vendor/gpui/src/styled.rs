use crate::{
    self as gpui, AbsoluteLength, AlignContent, AlignItems, BorderStyle, CursorStyle,
    DefiniteLength, Display, Fill, FlexDirection, FlexWrap, Font, FontStyle, FontWeight,
    GridPlacement, Hsla, JustifyContent, Length, SharedString, StrikethroughStyle, StyleRefinement,
    TextAlign, TextOverflow, TextStyleRefinement, UnderlineStyle, WhiteSpace, px, relative, rems,
};
pub use gpui_macros::{
    border_style_methods, box_shadow_style_methods, cursor_style_methods, margin_style_methods,
    overflow_style_methods, padding_style_methods, position_style_methods,
    visibility_style_methods,
};

const ELLIPSIS: SharedString = SharedString::new_static("…");

#[cfg_attr(
    any(feature = "inspector", debug_assertions),
    gpui_macros::derive_inspector_reflection
)]
pub trait Styled: Sized {
    fn style(&mut self) -> &mut StyleRefinement;

    gpui_macros::style_helpers!();
    gpui_macros::visibility_style_methods!();
    gpui_macros::margin_style_methods!();
    gpui_macros::padding_style_methods!();
    gpui_macros::position_style_methods!();
    gpui_macros::overflow_style_methods!();
    gpui_macros::cursor_style_methods!();
    gpui_macros::border_style_methods!();
    gpui_macros::box_shadow_style_methods!();

    #[doc = " Sets the display type of the element to `block`."]
    #[doc = " [Docs](https://tailwindcss.com/docs/display)"]
    fn block(mut self) -> Self {
        self.style().display = Some(Display::Block);
        self
    }

    #[doc = " Sets the display type of the element to `flex`."]
    #[doc = " [Docs](https://tailwindcss.com/docs/display)"]
    fn flex(mut self) -> Self {
        self.style().display = Some(Display::Flex);
        self
    }

    #[doc = " Sets the display type of the element to `grid`."]
    #[doc = " [Docs](https://tailwindcss.com/docs/display)"]
    fn grid(mut self) -> Self {
        self.style().display = Some(Display::Grid);
        self
    }

    #[doc = " Sets the display type of the element to `none`."]
    #[doc = " [Docs](https://tailwindcss.com/docs/display)"]
    fn hidden(mut self) -> Self {
        self.style().display = Some(Display::None);
        self
    }

    #[doc = " Sets the whitespace of the element to `normal`."]
    #[doc = " [Docs](https://tailwindcss.com/docs/whitespace#normal)"]
    fn whitespace_normal(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .white_space = Some(WhiteSpace::Normal);
        self
    }

    #[doc = " Sets the whitespace of the element to `nowrap`."]
    #[doc = " [Docs](https://tailwindcss.com/docs/whitespace#nowrap)"]
    fn whitespace_nowrap(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .white_space = Some(WhiteSpace::Nowrap);
        self
    }

    #[doc = " Sets the truncate overflowing text with an ellipsis (…) if needed."]
    #[doc = " [Docs](https://tailwindcss.com/docs/text-overflow#ellipsis)"]
    fn text_ellipsis(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .text_overflow = Some(TextOverflow::Truncate(ELLIPSIS));
        self
    }

    fn text_overflow(mut self, overflow: TextOverflow) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .text_overflow = Some(overflow);
        self
    }

    fn text_align(mut self, align: TextAlign) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .text_align = Some(align);
        self
    }

    #[doc = " Sets the text alignment to left"]
    fn text_left(mut self) -> Self {
        self.text_align(TextAlign::Left)
    }

    #[doc = " Sets the text alignment to center"]
    fn text_center(mut self) -> Self {
        self.text_align(TextAlign::Center)
    }

    #[doc = " Sets the text alignment to right"]
    fn text_right(mut self) -> Self {
        self.text_align(TextAlign::Right)
    }

    #[doc = " Sets the truncate to prevent text from wrapping and truncate overflowing text with an ellipsis (…) if needed."]
    #[doc = " [Docs](https://tailwindcss.com/docs/text-overflow#truncate)"]
    fn truncate(mut self) -> Self {
        self.overflow_hidden().whitespace_nowrap().text_ellipsis()
    }

    fn line_clamp(mut self, lines: usize) -> Self {
        let mut text_style = self.text_style().get_or_insert_with(Default::default);
        text_style.line_clamp = Some(lines);
        self.overflow_hidden()
    }

    #[doc = " Sets the flex direction of the element to `column`."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex-direction#column)"]
    fn flex_col(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::Column);
        self
    }

    #[doc = " Sets the flex direction of the element to `column-reverse`."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex-direction#column-reverse)"]
    fn flex_col_reverse(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::ColumnReverse);
        self
    }

    #[doc = " Sets the flex direction of the element to `row`."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex-direction#row)"]
    fn flex_row(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::Row);
        self
    }

    #[doc = " Sets the flex direction of the element to `row-reverse`."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex-direction#row-reverse)"]
    fn flex_row_reverse(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::RowReverse);
        self
    }

    #[doc = " Sets the element to allow a flex item to grow and shrink as needed, ignoring its initial size."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex#flex-1)"]
    fn flex_1(mut self) -> Self {
        self.style().flex_grow = Some(1.);
        self.style().flex_shrink = Some(1.);
        self.style().flex_basis = Some(relative(0.).into());
        self
    }

    #[doc = " Sets the element to allow a flex item to grow and shrink, taking into account its initial size."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex#auto)"]
    fn flex_auto(mut self) -> Self {
        self.style().flex_grow = Some(1.);
        self.style().flex_shrink = Some(1.);
        self.style().flex_basis = Some(Length::Auto);
        self
    }

    #[doc = " Sets the element to allow a flex item to shrink but not grow, taking into account its initial size."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex#initial)"]
    fn flex_initial(mut self) -> Self {
        self.style().flex_grow = Some(0.);
        self.style().flex_shrink = Some(1.);
        self.style().flex_basis = Some(Length::Auto);
        self
    }

    #[doc = " Sets the element to prevent a flex item from growing or shrinking."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex#none)"]
    fn flex_none(mut self) -> Self {
        self.style().flex_grow = Some(0.);
        self.style().flex_shrink = Some(0.);
        self
    }

    fn flex_basis(mut self, basis: impl Into<Length>) -> Self {
        self.style().flex_basis = Some(basis.into());
        self
    }

    #[doc = " Sets the element to allow a flex item to grow to fill any available space."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex-grow)"]
    fn flex_grow(mut self) -> Self {
        self.style().flex_grow = Some(1.);
        self
    }

    #[doc = " Sets the element to allow a flex item to shrink if needed."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex-shrink)"]
    fn flex_shrink(mut self) -> Self {
        self.style().flex_shrink = Some(1.);
        self
    }

    #[doc = " Sets the element to prevent a flex item from shrinking."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex-shrink#dont-shrink)"]
    fn flex_shrink_0(mut self) -> Self {
        self.style().flex_shrink = Some(0.);
        self
    }

    #[doc = " Sets the element to allow flex items to wrap."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex-wrap#wrap-normally)"]
    fn flex_wrap(mut self) -> Self {
        self.style().flex_wrap = Some(FlexWrap::Wrap);
        self
    }

    #[doc = " Sets the element wrap flex items in the reverse direction."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex-wrap#wrap-reversed)"]
    fn flex_wrap_reverse(mut self) -> Self {
        self.style().flex_wrap = Some(FlexWrap::WrapReverse);
        self
    }

    #[doc = " Sets the element to prevent flex items from wrapping, causing inflexible items to overflow the container if necessary."]
    #[doc = " [Docs](https://tailwindcss.com/docs/flex-wrap#dont-wrap)"]
    fn flex_nowrap(mut self) -> Self {
        self.style().flex_wrap = Some(FlexWrap::NoWrap);
        self
    }

    #[doc = " Sets the element to align flex items to the start of the container's cross axis."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-items#start)"]
    fn items_start(mut self) -> Self {
        self.style().align_items = Some(AlignItems::FlexStart);
        self
    }

    #[doc = " Sets the element to align flex items to the end of the container's cross axis."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-items#end)"]
    fn items_end(mut self) -> Self {
        self.style().align_items = Some(AlignItems::FlexEnd);
        self
    }

    #[doc = " Sets the element to align flex items along the center of the container's cross axis."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-items#center)"]
    fn items_center(mut self) -> Self {
        self.style().align_items = Some(AlignItems::Center);
        self
    }

    #[doc = " Sets the element to align flex items along the baseline of the container's cross axis."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-items#baseline)"]
    fn items_baseline(mut self) -> Self {
        self.style().align_items = Some(AlignItems::Baseline);
        self
    }

    #[doc = " Sets the element to justify flex items against the start of the container's main axis."]
    #[doc = " [Docs](https://tailwindcss.com/docs/justify-content#start)"]
    fn justify_start(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::Start);
        self
    }

    #[doc = " Sets the element to justify flex items against the end of the container's main axis."]
    #[doc = " [Docs](https://tailwindcss.com/docs/justify-content#end)"]
    fn justify_end(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::End);
        self
    }

    #[doc = " Sets the element to justify flex items along the center of the container's main axis."]
    #[doc = " [Docs](https://tailwindcss.com/docs/justify-content#center)"]
    fn justify_center(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::Center);
        self
    }

    #[doc = " Sets the element to justify flex items along the container's main axis"]
    #[doc = " such that there is an equal amount of space between each item."]
    #[doc = " [Docs](https://tailwindcss.com/docs/justify-content#space-between)"]
    fn justify_between(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::SpaceBetween);
        self
    }

    #[doc = " Sets the element to justify items along the container's main axis such"]
    #[doc = " that there is an equal amount of space on each side of each item."]
    #[doc = " [Docs](https://tailwindcss.com/docs/justify-content#space-around)"]
    fn justify_around(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::SpaceAround);
        self
    }

    #[doc = " Sets the element to pack content items in their default position as if no align-content value was set."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-content#normal)"]
    fn content_normal(mut self) -> Self {
        self.style().align_content = None;
        self
    }

    #[doc = " Sets the element to pack content items in the center of the container's cross axis."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-content#center)"]
    fn content_center(mut self) -> Self {
        self.style().align_content = Some(AlignContent::Center);
        self
    }

    #[doc = " Sets the element to pack content items against the start of the container's cross axis."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-content#start)"]
    fn content_start(mut self) -> Self {
        self.style().align_content = Some(AlignContent::FlexStart);
        self
    }

    #[doc = " Sets the element to pack content items against the end of the container's cross axis."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-content#end)"]
    fn content_end(mut self) -> Self {
        self.style().align_content = Some(AlignContent::FlexEnd);
        self
    }

    #[doc = " Sets the element to pack content items along the container's cross axis"]
    #[doc = " such that there is an equal amount of space between each item."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-content#space-between)"]
    fn content_between(mut self) -> Self {
        self.style().align_content = Some(AlignContent::SpaceBetween);
        self
    }

    #[doc = " Sets the element to pack content items along the container's cross axis"]
    #[doc = " such that there is an equal amount of space on each side of each item."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-content#space-around)"]
    fn content_around(mut self) -> Self {
        self.style().align_content = Some(AlignContent::SpaceAround);
        self
    }

    #[doc = " Sets the element to pack content items along the container's cross axis"]
    #[doc = " such that there is an equal amount of space between each item."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-content#space-evenly)"]
    fn content_evenly(mut self) -> Self {
        self.style().align_content = Some(AlignContent::SpaceEvenly);
        self
    }

    #[doc = " Sets the element to allow content items to fill the available space along the container's cross axis."]
    #[doc = " [Docs](https://tailwindcss.com/docs/align-content#stretch)"]
    fn content_stretch(mut self) -> Self {
        self.style().align_content = Some(AlignContent::Stretch);
        self
    }

    fn bg<F>(mut self, fill: F) -> Self
    where
        F: Into<Fill>,
        Self: Sized,
    {
        self.style().background = Some(fill.into());
        self
    }

    #[doc = " Sets the border style of the element."]
    fn border_dashed(mut self) -> Self {
        self.style().border_style = Some(BorderStyle::Dashed);
        self
    }

    fn text_style(&mut self) -> &mut Option<TextStyleRefinement> {
        let style: &mut StyleRefinement = self.style();
        &mut style.text
    }

    fn text_color(mut self, color: impl Into<Hsla>) -> Self {
        self.text_style().get_or_insert_with(Default::default).color = Some(color.into());
        self
    }

    fn font_weight(mut self, weight: FontWeight) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_weight = Some(weight);
        self
    }

    fn text_bg(mut self, bg: impl Into<Hsla>) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .background_color = Some(bg.into());
        self
    }

    fn text_size(mut self, size: impl Into<AbsoluteLength>) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_size = Some(size.into());
        self
    }

    #[doc = " Sets the text size to 'extra small'."]
    #[doc = " [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)"]
    fn text_xs(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_size = Some(rems(0.75).into());
        self
    }

    #[doc = " Sets the text size to 'small'."]
    #[doc = " [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)"]
    fn text_sm(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_size = Some(rems(0.875).into());
        self
    }

    #[doc = " Sets the text size to 'base'."]
    #[doc = " [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)"]
    fn text_base(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_size = Some(rems(1.0).into());
        self
    }

    #[doc = " Sets the text size to 'large'."]
    #[doc = " [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)"]
    fn text_lg(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_size = Some(rems(1.125).into());
        self
    }

    #[doc = " Sets the text size to 'extra large'."]
    #[doc = " [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)"]
    fn text_xl(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_size = Some(rems(1.25).into());
        self
    }

    #[doc = " Sets the text size to 'extra extra large'."]
    #[doc = " [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)"]
    fn text_2xl(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_size = Some(rems(1.5).into());
        self
    }

    #[doc = " Sets the text size to 'extra extra extra large'."]
    #[doc = " [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)"]
    fn text_3xl(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_size = Some(rems(1.875).into());
        self
    }

    #[doc = " Sets the font style of the element to italic."]
    #[doc = " [Docs](https://tailwindcss.com/docs/font-style#italicizing-text)"]
    fn italic(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_style = Some(FontStyle::Italic);
        self
    }

    #[doc = " Sets the font style of the element to normal (not italic)."]
    #[doc = " [Docs](https://tailwindcss.com/docs/font-style#displaying-text-normally)"]
    fn not_italic(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_style = Some(FontStyle::Normal);
        self
    }

    #[doc = " Sets the text decoration to underline."]
    #[doc = " [Docs](https://tailwindcss.com/docs/text-decoration-line#underling-text)"]
    fn underline(mut self) -> Self {
        let style = self.text_style().get_or_insert_with(Default::default);
        style.underline = Some(UnderlineStyle {
            thickness: px(1.),
            ..Default::default()
        });
        self
    }

    #[doc = " Sets the decoration of the text to have a line through it."]
    #[doc = " [Docs](https://tailwindcss.com/docs/text-decoration-line#adding-a-line-through-text)"]
    fn line_through(mut self) -> Self {
        let style = self.text_style().get_or_insert_with(Default::default);
        style.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.),
            ..Default::default()
        });
        self
    }

    #[doc = " Removes the text decoration on this element."]
    #[doc = ""]
    #[doc = " This value cascades to its child elements."]
    fn text_decoration_none(mut self) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .underline = None;
        self
    }

    fn text_decoration_color(mut self, color: impl Into<Hsla>) -> Self {
        let style = self.text_style().get_or_insert_with(Default::default);
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.color = Some(color.into());
        self
    }

    #[doc = " Sets the text decoration style to a solid line."]
    #[doc = " [Docs](https://tailwindcss.com/docs/text-decoration-style)"]
    fn text_decoration_solid(mut self) -> Self {
        let style = self.text_style().get_or_insert_with(Default::default);
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.wavy = false;
        self
    }

    #[doc = " Sets the text decoration style to a wavy line."]
    #[doc = " [Docs](https://tailwindcss.com/docs/text-decoration-style)"]
    fn text_decoration_wavy(mut self) -> Self {
        let style = self.text_style().get_or_insert_with(Default::default);
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.wavy = true;
        self
    }

    #[doc = " Sets the text decoration to be 0px thick."]
    #[doc = " [Docs](https://tailwindcss.com/docs/text-decoration-thickness)"]
    fn text_decoration_0(mut self) -> Self {
        let style = self.text_style().get_or_insert_with(Default::default);
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(0.);
        self
    }

    #[doc = " Sets the text decoration to be 1px thick."]
    #[doc = " [Docs](https://tailwindcss.com/docs/text-decoration-thickness)"]
    fn text_decoration_1(mut self) -> Self {
        let style = self.text_style().get_or_insert_with(Default::default);
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(1.);
        self
    }

    #[doc = " Sets the text decoration to be 2px thick."]
    #[doc = " [Docs](https://tailwindcss.com/docs/text-decoration-thickness)"]
    fn text_decoration_2(mut self) -> Self {
        let style = self.text_style().get_or_insert_with(Default::default);
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(2.);
        self
    }

    #[doc = " Sets the text decoration to be 4px thick."]
    #[doc = " [Docs](https://tailwindcss.com/docs/text-decoration-thickness)"]
    fn text_decoration_4(mut self) -> Self {
        let style = self.text_style().get_or_insert_with(Default::default);
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(4.);
        self
    }

    #[doc = " Sets the text decoration to be 8px thick."]
    #[doc = " [Docs](https://tailwindcss.com/docs/text-decoration-thickness)"]
    fn text_decoration_8(mut self) -> Self {
        let style = self.text_style().get_or_insert_with(Default::default);
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(8.);
        self
    }

    fn font_family(mut self, family_name: impl Into<SharedString>) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .font_family = Some(family_name.into());
        self
    }

    fn font(mut self, font: Font) -> Self {
        let Font {
            family,
            features,
            fallbacks,
            weight,
            style,
        } = font;

        let text_style = self.text_style().get_or_insert_with(Default::default);
        text_style.font_family = Some(family);
        text_style.font_features = Some(features);
        text_style.font_weight = Some(weight);
        text_style.font_style = Some(style);
        text_style.font_fallbacks = fallbacks;

        self
    }

    fn line_height(mut self, line_height: impl Into<DefiniteLength>) -> Self {
        self.text_style()
            .get_or_insert_with(Default::default)
            .line_height = Some(line_height.into());
        self
    }

    fn opacity(mut self, opacity: f32) -> Self {
        self.style().opacity = Some(opacity);
        self
    }

    fn grid_cols(mut self, cols: u16) -> Self {
        self.style().grid_cols = Some(cols);
        self
    }

    fn grid_rows(mut self, rows: u16) -> Self {
        self.style().grid_rows = Some(rows);
        self
    }

    fn col_start(mut self, start: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.start = GridPlacement::Line(start);
        self
    }

    #[doc = " Sets the column start of this element to auto."]
    fn col_start_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.start = GridPlacement::Auto;
        self
    }

    fn col_end(mut self, end: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.end = GridPlacement::Line(end);
        self
    }

    #[doc = " Sets the column end of this element to auto."]
    fn col_end_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.end = GridPlacement::Auto;
        self
    }

    fn col_span(mut self, span: u16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column = GridPlacement::Span(span)..GridPlacement::Span(span);
        self
    }

    #[doc = " Sets the row span of this element."]
    fn col_span_full(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column = GridPlacement::Line(1)..GridPlacement::Line(-1);
        self
    }

    fn row_start(mut self, start: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.start = GridPlacement::Line(start);
        self
    }

    #[doc = " Sets the row start of this element to \"auto\""]
    fn row_start_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.start = GridPlacement::Auto;
        self
    }

    fn row_end(mut self, end: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.end = GridPlacement::Line(end);
        self
    }

    #[doc = " Sets the row end of this element to \"auto\""]
    fn row_end_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.end = GridPlacement::Auto;
        self
    }

    fn row_span(mut self, span: u16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row = GridPlacement::Span(span)..GridPlacement::Span(span);
        self
    }

    #[doc = " Sets the row span of this element."]
    fn row_span_full(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row = GridPlacement::Line(1)..GridPlacement::Line(-1);
        self
    }

    #[doc = " Draws a debug border around this element."]
    #[cfg(debug_assertions)]
    fn debug(mut self) -> Self {
        self.style().debug = Some(true);
        self
    }

    #[doc = " Draws a debug border on all conforming elements below this element."]
    #[cfg(debug_assertions)]
    fn debug_below(mut self) -> Self {
        self.style().debug_below = Some(true);
        self
    }
}
