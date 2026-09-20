use gpui::{Bounds, Hsla, Path, Pixels, Window, fill};

#[cfg(test)]
#[path = "layers_tests.rs"]
mod tests;

#[derive(Default)]
pub(super) struct Paths {
    #[cfg(test)]
    pub(super) legacy_dots: bool,
    groups: Vec<PathGroup>,
}

struct PathGroup {
    bounds: Option<Bounds<Pixels>>,
    shapes: Shapes,
}

enum Shapes {
    Paths(Vec<(Path<Pixels>, Hsla)>),
    Dots(Vec<(Bounds<Pixels>, Hsla)>),
}

impl Paths {
    pub(super) fn push(&mut self, path: (Path<Pixels>, Hsla)) {
        self.groups.push(PathGroup {
            bounds: None,
            shapes: Shapes::Paths(vec![path]),
        });
    }

    #[cfg(test)]
    pub(super) fn extend_disjoint(
        &mut self,
        bounds: Bounds<Pixels>,
        paths: Vec<(Path<Pixels>, Hsla)>,
    ) {
        if paths.is_empty() {
            return;
        }
        if let Some(previous) = self.groups.last_mut()
            && let Shapes::Paths(previous_paths) = &mut previous.shapes
            && let Some(previous_bounds) = &mut previous.bounds
            && !previous_bounds.intersects(&bounds)
        {
            *previous_bounds = previous_bounds.union(&bounds);
            previous_paths.extend(paths);
            return;
        }
        self.groups.push(PathGroup {
            bounds: Some(bounds),
            shapes: Shapes::Paths(paths),
        });
    }

    pub(super) fn dots(
        &mut self,
        bounds: Option<Bounds<Pixels>>,
        dots: Vec<(Bounds<Pixels>, Hsla)>,
    ) {
        if let Some(previous) = self.groups.last_mut()
            && let Shapes::Dots(previous_dots) = &mut previous.shapes
            && let Some(previous_bounds) = &mut previous.bounds
            && let Some(bounds) = bounds
            && !previous_bounds.intersects(&bounds)
        {
            *previous_bounds = previous_bounds.union(&bounds);
            previous_dots.extend(dots);
            return;
        }
        self.groups.push(PathGroup {
            bounds,
            shapes: Shapes::Dots(dots),
        });
    }

    pub(super) fn paint(&self, color: Option<Hsla>, layered: bool, window: &mut Window) {
        let clip = window.content_mask().bounds;
        for group in &self.groups {
            if group.bounds.is_some_and(|bounds| !bounds.intersects(&clip)) {
                continue;
            }
            let paint = |window: &mut Window| match &group.shapes {
                Shapes::Paths(paths) => {
                    crate::profiler::count(crate::profiler::Metric::PaintPaths, paths.len());
                    for (path, original) in paths {
                        window.paint_path(path.clone(), color.unwrap_or(*original));
                    }
                }
                Shapes::Dots(dots) => {
                    let mut painted = 0;
                    for &(bounds, original) in dots {
                        if !bounds.intersects(&clip) {
                            continue;
                        }
                        painted += 1;
                        window.paint_quad(
                            fill(bounds, color.unwrap_or(original))
                                .corner_radii(bounds.size.width / 2.0),
                        );
                    }
                    crate::profiler::count(crate::profiler::Metric::PaintDots, painted);
                }
            };
            if layered && let Some(bounds) = group.bounds {
                crate::profiler::count(crate::profiler::Metric::PaintLayers, 1);
                window.paint_layer(bounds, paint);
            } else {
                paint(window);
            }
        }
    }
}

pub(super) fn quads(
    mut quads: &[(Bounds<Pixels>, Hsla)],
    opacity: f32,
    layered: bool,
    window: &mut Window,
) {
    crate::profiler::count(crate::profiler::Metric::PaintQuads, quads.len());
    while let Some(&(first, _)) = quads.first() {
        let (count, bounds) = if layered {
            disjoint_prefix(quads)
        } else {
            (quads.len(), first)
        };
        let (group, rest) = quads.split_at(count);
        let paint = |window: &mut Window| {
            for &(bounds, mut color) in group {
                color.a *= opacity;
                window.paint_quad(fill(bounds, color));
            }
        };
        if layered {
            crate::profiler::count(crate::profiler::Metric::PaintLayers, 1);
            window.paint_layer(bounds, paint);
        } else {
            paint(window);
        }
        quads = rest;
    }
}

fn disjoint_prefix(quads: &[(Bounds<Pixels>, Hsla)]) -> (usize, Bounds<Pixels>) {
    let mut bounds = quads[0].0;
    let mut count = 1;
    for &(next, _) in &quads[1..] {
        if bounds.intersects(&next) {
            break;
        }
        bounds = bounds.union(&next);
        count += 1;
    }
    (count, bounds)
}

#[derive(Default)]
pub(super) struct QuadLayers {
    layers: Vec<QuadLayer>,
    group_start: usize,
    group_bounds: Option<Bounds<Pixels>>,
}

struct QuadLayer {
    bounds: Bounds<Pixels>,
    indices: Vec<usize>,
}

impl QuadLayers {
    pub(super) fn append_cell(&mut self, quads: &[(Bounds<Pixels>, Hsla)], start: usize) {
        let Some(bounds) = quads[start..]
            .iter()
            .map(|(bounds, _)| *bounds)
            .reduce(|previous, next| previous.union(&next))
        else {
            return;
        };
        if self
            .group_bounds
            .is_none_or(|previous| previous.intersects(&bounds))
        {
            self.group_start = self.layers.len();
            self.group_bounds = Some(bounds);
        } else if let Some(previous) = &mut self.group_bounds {
            *previous = previous.union(&bounds);
        }
        for (offset, &(bounds, _)) in quads[start..].iter().enumerate() {
            let index = start + offset;
            if let Some(layer) = self.layers.get_mut(self.group_start + offset) {
                layer.bounds = layer.bounds.union(&bounds);
                layer.indices.push(index);
            } else {
                self.layers.push(QuadLayer {
                    bounds,
                    indices: vec![index],
                });
            }
        }
    }

    pub(super) fn paint(
        &self,
        quads: &[(Bounds<Pixels>, Hsla)],
        color: Option<Hsla>,
        layered: bool,
        window: &mut Window,
    ) {
        crate::profiler::count(crate::profiler::Metric::PaintQuads, quads.len());
        if !layered {
            for &(bounds, original) in quads {
                window.paint_quad(fill(bounds, color.unwrap_or(original)));
            }
            return;
        }
        let clip = window.content_mask().bounds;
        for layer in &self.layers {
            if !layer.bounds.intersects(&clip) {
                continue;
            }
            crate::profiler::count(crate::profiler::Metric::PaintLayers, 1);
            window.paint_layer(layer.bounds, |window| {
                for &index in &layer.indices {
                    let (bounds, original) = quads[index];
                    window.paint_quad(fill(bounds, color.unwrap_or(original)));
                }
            });
        }
    }
}

#[cfg(test)]
#[path = "dots_tests.rs"]
mod dot_tests;
