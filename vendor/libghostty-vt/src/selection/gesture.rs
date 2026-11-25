use std::{mem::MaybeUninit, time::Duration};

use crate::{
    alloc::{Allocator, Object},
    error::{Error, Result, from_optional_result, from_result},
    ffi,
    screen::GridRef,
    selection::Selection,
    terminal::{PointCoordinate, Terminal},
};

#[doc(inline)]
pub use ffi::SelectionGestureGeometry as Geometry;

#[derive(Debug)]
pub struct Gesture<'alloc> {
    inner: Object<'alloc, ffi::SelectionGestureImpl>,
}
impl<'alloc> Gesture<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::SelectionGesture = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_selection_gesture_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self {
            inner: Object::new(raw)?,
        })
    }

    fn get<T>(
        &self,
        terminal: &Terminal<'_, '_>,
        tag: ffi::SelectionGestureData::Type,
    ) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_selection_gesture_get(
                self.inner.as_raw(),
                terminal.inner.as_raw(),
                tag,
                value.as_mut_ptr().cast(),
            )
        };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }
    pub fn reset(&mut self, terminal: &Terminal<'_, '_>) {
        unsafe {
            ffi::ghostty_selection_gesture_reset(self.inner.as_raw(), terminal.inner.as_raw());
        }
    }

    pub fn click_count(&self, terminal: &Terminal<'_, '_>) -> Result<u8> {
        self.get(terminal, ffi::SelectionGestureData::CLICK_COUNT)
    }
    pub fn dragged(&self, terminal: &Terminal<'_, '_>) -> Result<bool> {
        self.get(terminal, ffi::SelectionGestureData::DRAGGED)
    }
    pub fn autoscroll(&self, terminal: &Terminal<'_, '_>) -> Result<Autoscroll> {
        let v = self.get::<ffi::SelectionGestureAutoscroll::Type>(
            terminal,
            ffi::SelectionGestureData::AUTOSCROLL,
        )?;
        Autoscroll::try_from(v).map_err(|_| Error::InvalidValue)
    }
    pub fn behavior(&self, terminal: &Terminal<'_, '_>) -> Result<Behavior> {
        let v = self.get::<ffi::SelectionGestureBehavior::Type>(
            terminal,
            ffi::SelectionGestureData::BEHAVIOR,
        )?;
        Behavior::try_from(v).map_err(|_| Error::InvalidValue)
    }
    pub fn anchor<'t>(&self, terminal: &'t Terminal<'_, '_>) -> Result<Option<GridRef<'t>>> {
        let mut grid_ref = ffi::sized!(ffi::GridRef);
        let result = unsafe {
            ffi::ghostty_selection_gesture_get(
                self.inner.as_raw(),
                terminal.inner.as_raw(),
                ffi::SelectionGestureData::ANCHOR,
                (&raw mut grid_ref).cast(),
            )
        };
        let grid_ref = from_optional_result(result, grid_ref)?;
        Ok(grid_ref.map(|v| unsafe { GridRef::from_raw(v) }))
    }
}
impl Drop for Gesture<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::ghostty_selection_gesture_free(self.inner.as_raw(), std::ptr::null_mut());
        }
    }
}

#[derive(Debug)]
struct Event<'alloc> {
    inner: Object<'alloc, ffi::SelectionGestureEventImpl>,
}
impl Event<'_> {
    unsafe fn new_inner(
        alloc: *const ffi::Allocator,
        ty: ffi::SelectionGestureEventType::Type,
    ) -> Result<Self> {
        let mut raw: ffi::SelectionGestureEvent = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_selection_gesture_event_new(alloc, &raw mut raw, ty) };
        from_result(result)?;
        Ok(Self {
            inner: Object::new(raw)?,
        })
    }

    fn set<T>(&mut self, field: ffi::SelectionGestureEventOption::Type, v: &T) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_selection_gesture_event_set(
                self.inner.as_raw(),
                field,
                std::ptr::from_ref(v).cast(),
            )
        };
        from_result(result)?;
        Ok(())
    }

    fn unset(&mut self, field: ffi::SelectionGestureEventOption::Type) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_selection_gesture_event_set(self.inner.as_raw(), field, std::ptr::null())
        };
        from_result(result)?;
        Ok(())
    }

    fn apply<'t>(
        &mut self,
        gesture: &mut Gesture<'_>,
        terminal: &'t Terminal<'_, '_>,
    ) -> Result<Option<Selection<'t>>> {
        let mut selection = ffi::sized!(ffi::Selection);

        let result = unsafe {
            ffi::ghostty_selection_gesture_event(
                gesture.inner.as_raw(),
                terminal.inner.as_raw(),
                self.inner.as_raw(),
                &raw mut selection,
            )
        };
        let selection = from_optional_result(result, selection)?;

        Ok(selection.map(|v| unsafe { Selection::from_raw(v) }))
    }
}
impl Drop for Event<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::ghostty_selection_gesture_event_free(self.inner.as_raw());
        }
    }
}

#[derive(Debug)]
pub struct PressEvent<'alloc> {
    base: Event<'alloc>,
}
impl<'alloc> PressEvent<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        Ok(Self {
            base: unsafe { Event::new_inner(alloc, ffi::SelectionGestureEventType::PRESS)? },
        })
    }

    #[inline]
    pub fn set_position(&mut self, x: f64, y: f64) -> Result<&mut Self> {
        let value = ffi::SurfacePosition { x, y };
        self.base
            .set(ffi::SelectionGestureEventOption::POSITION, &value)?;
        Ok(self)
    }

    #[inline]
    pub fn set_repeat_distance(&mut self, value: f64) -> Result<&mut Self> {
        self.base
            .set(ffi::SelectionGestureEventOption::REPEAT_DISTANCE, &value)?;
        Ok(self)
    }

    #[inline]
    pub fn set_time(&mut self, value: Duration) -> Result<&mut Self> {
        let nanos = u64::try_from(value.as_nanos()).unwrap_or(u64::MAX);
        self.base
            .set(ffi::SelectionGestureEventOption::TIME_NS, &nanos)?;
        Ok(self)
    }

    #[inline]
    pub fn set_repeat_interval(&mut self, value: Duration) -> Result<&mut Self> {
        let nanos = u64::try_from(value.as_nanos()).unwrap_or(u64::MAX);
        self.base
            .set(ffi::SelectionGestureEventOption::REPEAT_INTERVAL_NS, &nanos)?;
        Ok(self)
    }

    #[inline]
    pub fn set_word_boundary_codepoints(&mut self, value: &[char]) -> Result<&mut Self> {
        let cp = ffi::Codepoints {
            ptr: value.as_ptr().cast(),
            len: value.len(),
        };

        self.base.set(
            ffi::SelectionGestureEventOption::WORD_BOUNDARY_CODEPOINTS,
            &cp,
        )?;
        Ok(self)
    }

    #[inline]
    pub fn set_behaviors(&mut self, value: &Behaviors) -> Result<&mut Self> {
        self.base
            .set(ffi::SelectionGestureEventOption::BEHAVIORS, &value.inner)?;
        Ok(self)
    }

    #[inline]
    pub fn apply<'t>(
        &mut self,
        gesture: &mut Gesture<'_>,
        terminal: &'t Terminal<'_, '_>,
        grid_ref: GridRef<'t>,
    ) -> Result<Option<Selection<'t>>> {
        self.base
            .set(ffi::SelectionGestureEventOption::REF, &grid_ref.inner)?;
        self.base.apply(gesture, terminal)
    }
}

#[derive(Debug)]
pub struct ReleaseEvent<'alloc> {
    base: Event<'alloc>,
}
impl<'alloc> ReleaseEvent<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        Ok(Self {
            base: unsafe { Event::new_inner(alloc, ffi::SelectionGestureEventType::RELEASE)? },
        })
    }

    #[inline]
    pub fn apply<'t>(
        &mut self,
        gesture: &mut Gesture<'_>,
        terminal: &'t Terminal<'_, '_>,
        grid_ref: Option<GridRef<'t>>,
    ) -> Result<()> {
        match grid_ref {
            Some(g) => self
                .base
                .set(ffi::SelectionGestureEventOption::REF, &g.inner)?,
            None => self.base.unset(ffi::SelectionGestureEventOption::REF)?,
        }

        _ = self.base.apply(gesture, terminal)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct DragEvent<'alloc> {
    base: Event<'alloc>,
}
impl<'alloc> DragEvent<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        Ok(Self {
            base: unsafe { Event::new_inner(alloc, ffi::SelectionGestureEventType::DRAG)? },
        })
    }

    #[inline]
    pub fn set_position(&mut self, x: f64, y: f64) -> Result<&mut Self> {
        let value = ffi::SurfacePosition { x, y };
        self.base
            .set(ffi::SelectionGestureEventOption::POSITION, &value)?;
        Ok(self)
    }

    #[inline]
    pub fn set_rectangle(&mut self, value: bool) -> Result<&mut Self> {
        self.base
            .set(ffi::SelectionGestureEventOption::RECTANGLE, &value)?;
        Ok(self)
    }

    #[inline]
    pub fn set_word_boundary_codepoints(&mut self, value: &[char]) -> Result<&mut Self> {
        let cp = ffi::Codepoints {
            ptr: value.as_ptr().cast(),
            len: value.len(),
        };

        self.base.set(
            ffi::SelectionGestureEventOption::WORD_BOUNDARY_CODEPOINTS,
            &cp,
        )?;
        Ok(self)
    }

    #[inline]
    pub fn apply<'t>(
        &mut self,
        gesture: &mut Gesture<'_>,
        terminal: &'t Terminal<'_, '_>,
        grid_ref: GridRef<'t>,
        geometry: Geometry,
    ) -> Result<Option<Selection<'t>>> {
        self.base
            .set(ffi::SelectionGestureEventOption::REF, &grid_ref.inner)?;
        self.base
            .set(ffi::SelectionGestureEventOption::GEOMETRY, &geometry)?;
        self.base.apply(gesture, terminal)
    }
}

#[derive(Debug)]
pub struct AutoscrollTickEvent<'alloc> {
    base: Event<'alloc>,
}
impl<'alloc> AutoscrollTickEvent<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        Ok(Self {
            base: unsafe {
                Event::new_inner(alloc, ffi::SelectionGestureEventType::AUTOSCROLL_TICK)?
            },
        })
    }

    #[inline]
    pub fn set_position(&mut self, x: f64, y: f64) -> Result<&mut Self> {
        let value = ffi::SurfacePosition { x, y };
        self.base
            .set(ffi::SelectionGestureEventOption::POSITION, &value)?;
        Ok(self)
    }

    #[inline]
    pub fn set_rectangle(&mut self, value: bool) -> Result<&mut Self> {
        self.base
            .set(ffi::SelectionGestureEventOption::RECTANGLE, &value)?;
        Ok(self)
    }

    #[inline]
    pub fn set_word_boundary_codepoints(&mut self, value: &[char]) -> Result<&mut Self> {
        let cp = ffi::Codepoints {
            ptr: value.as_ptr().cast(),
            len: value.len(),
        };

        self.base.set(
            ffi::SelectionGestureEventOption::WORD_BOUNDARY_CODEPOINTS,
            &cp,
        )?;
        Ok(self)
    }

    #[inline]
    pub fn apply<'t>(
        &mut self,
        gesture: &mut Gesture<'_>,
        terminal: &'t Terminal<'_, '_>,
        viewport: PointCoordinate,
        geometry: Geometry,
    ) -> Result<Option<Selection<'t>>> {
        let viewport = ffi::PointCoordinate::from(viewport);
        self.base
            .set(ffi::SelectionGestureEventOption::VIEWPORT, &viewport)?;
        self.base
            .set(ffi::SelectionGestureEventOption::GEOMETRY, &geometry)?;
        self.base.apply(gesture, terminal)
    }
}

#[derive(Debug)]
pub struct DeepPressEvent<'alloc> {
    base: Event<'alloc>,
}
impl<'alloc> DeepPressEvent<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        Ok(Self {
            base: unsafe { Event::new_inner(alloc, ffi::SelectionGestureEventType::DEEP_PRESS)? },
        })
    }

    #[inline]
    pub fn set_word_boundary_codepoints(&mut self, value: &[char]) -> Result<&mut Self> {
        let cp = ffi::Codepoints {
            ptr: value.as_ptr().cast(),
            len: value.len(),
        };

        self.base.set(
            ffi::SelectionGestureEventOption::WORD_BOUNDARY_CODEPOINTS,
            &cp,
        )?;
        Ok(self)
    }

    #[inline]
    pub fn apply<'t>(
        &mut self,
        gesture: &mut Gesture<'_>,
        terminal: &'t Terminal<'_, '_>,
    ) -> Result<Option<Selection<'t>>> {
        self.base.apply(gesture, terminal)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum Autoscroll {
    None,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum Behavior {
    Cell = ffi::SelectionGestureBehavior::CELL,
    Word = ffi::SelectionGestureBehavior::WORD,
    Line = ffi::SelectionGestureBehavior::LINE,
    Output = ffi::SelectionGestureBehavior::OUTPUT,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Behaviors {
    inner: ffi::SelectionGestureBehaviors,
}
impl Behaviors {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_single_click_behavior(mut self, behavior: Behavior) -> Self {
        self.inner.single_click = behavior.into();
        self
    }
    #[must_use]
    pub fn with_double_click_behavior(mut self, behavior: Behavior) -> Self {
        self.inner.double_click = behavior.into();
        self
    }
    #[must_use]
    pub fn with_triple_click_behavior(mut self, behavior: Behavior) -> Self {
        self.inner.triple_click = behavior.into();
        self
    }
}
