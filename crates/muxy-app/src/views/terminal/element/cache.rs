use super::{Palette, Pixels, Point, Rc, RowPainting, RowRenderer, Run};

#[derive(Default)]
pub(crate) struct Rows {
    #[cfg(test)]
    pub(super) bypass: bool,
    configuration: Option<Configuration>,
    entries: Vec<Option<Row>>,
}

struct Configuration {
    settings: muxy_app_core::settings::FontOptions,
    cell: gpui::Size<Pixels>,
    palette: Palette,
    font: gpui::Font,
    font_size: Pixels,
    metrics: (Pixels, Pixels),
    scale: f32,
}

struct Row {
    runs: Vec<Run>,
    position: Point<Pixels>,
    collect_glyphs: bool,
    painting: Rc<RowPainting>,
}

impl Configuration {
    fn matches(&self, renderer: &RowRenderer<'_>) -> bool {
        self.settings == *renderer.settings
            && self.cell == renderer.cell
            && self.palette == *renderer.palette
            && self.font == *renderer.base_font
            && self.font_size == renderer.font_size
            && self.metrics == renderer.metrics
            && self.scale.to_bits() == renderer.window.scale_factor().to_bits()
    }
}

impl Rows {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn begin(&mut self, renderer: &RowRenderer<'_>, count: usize) {
        if !self
            .configuration
            .as_ref()
            .is_some_and(|key| key.matches(renderer))
        {
            self.entries.clear();
            self.configuration = Some(Configuration {
                settings: renderer.settings.clone(),
                cell: renderer.cell,
                palette: *renderer.palette,
                font: renderer.base_font.clone(),
                font_size: renderer.font_size,
                metrics: renderer.metrics,
                scale: renderer.window.scale_factor(),
            });
        }
        self.entries.resize_with(count, || None);
    }

    pub(super) fn prepare(
        &mut self,
        index: usize,
        runs: &[Run],
        position: Point<Pixels>,
        collect_glyphs: bool,
        renderer: &mut RowRenderer<'_>,
    ) -> Rc<RowPainting> {
        let bypass = crate::profiler::bypass_row_cache();
        #[cfg(test)]
        let bypass = bypass || self.bypass;
        if bypass {
            crate::profiler::count(crate::profiler::Metric::RowCacheMiss, 1);
            let mut painting = RowPainting::default();
            renderer.row(runs, position, &mut painting, collect_glyphs);
            return Rc::new(painting);
        }
        let slot = &mut self.entries[index];
        if let Some(row) = slot
            && row.runs == runs
            && row.position == position
            && row.collect_glyphs == collect_glyphs
        {
            crate::profiler::count(crate::profiler::Metric::RowCacheHit, 1);
            return row.painting.clone();
        }
        crate::profiler::count(crate::profiler::Metric::RowCacheMiss, 1);
        let mut painting = RowPainting::default();
        renderer.row(runs, position, &mut painting, collect_glyphs);
        let painting = Rc::new(painting);
        *slot = Some(Row {
            runs: runs.to_vec(),
            position,
            collect_glyphs,
            painting: painting.clone(),
        });
        painting
    }
}
