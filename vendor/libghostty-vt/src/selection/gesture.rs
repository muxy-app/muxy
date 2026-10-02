//! [Selection gestures](Gesture) provide a reusable state machine for turning
//! UI pointer interactions into selection snapshots.
//!
//! A caller creates one [`Gesture`] per active gesture stream, reuses typed
//! gesture event objects for synthetic [press](PressEvent), [drag](DragEvent),
//! [release](ReleaseEvent), [autoscroll tick](AutoscrollTickEvent),
//! and [deep-press](DeepPressEvent) events, and applies each event with
//! their respective `apply` method (e.g. [PressEvent::apply]). The returned
//! [`Selection`] is a snapshot; the embedder decides whether to render it,
//! format/copy it, or install it as the terminal's active selection.

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

/// Opaque handle to state for interpreting terminal selection gestures.
///
/// The gesture owns only the state required to interpret pointer events.
/// Calls that use a gesture are not concurrency-safe and must be serialized
/// with terminal mutations.
///
/// # Memory management
///
/// When dropped, this type will temporarily **leak** a small amount of memory
/// belonging to the internal [`TrackedGridRef`](crate::screen::TrackedGridRef)s.
/// They will instead be reclaimed when the terminal they belong to is dropped.
/// This memory can be preemptively reclaimed by calling the [`Gesture::reset`]
/// method if needed.
#[derive(Debug)]
pub struct Gesture<'alloc> {
    inner: Object<'alloc, ffi::SelectionGestureImpl>,
}
impl<'alloc> Gesture<'alloc> {
    /// Create a new selection gesture instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new selection gesture instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
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
        // SAFETY: Value should be initialized after successful call.
        Ok(unsafe { value.assume_init() })
    }
    /// Reset any active selection gesture state.
    ///
    /// This cancels the active click sequence and releases any tracked terminal
    /// references owned by the gesture without dropping the gesture object.
    pub fn reset(&mut self, terminal: &Terminal<'_, '_>) {
        unsafe {
            ffi::ghostty_selection_gesture_reset(self.inner.as_raw(), terminal.inner.as_raw());
        }
    }

    /// Get the current click count. 0 means inactive.
    pub fn click_count(&self, terminal: &Terminal<'_, '_>) -> Result<u8> {
        self.get(terminal, ffi::SelectionGestureData::CLICK_COUNT)
    }
    /// Whether the current/last left-click gesture has dragged.
    pub fn dragged(&self, terminal: &Terminal<'_, '_>) -> Result<bool> {
        self.get(terminal, ffi::SelectionGestureData::DRAGGED)
    }
    /// Get the current autoscroll request.
    pub fn autoscroll(&self, terminal: &Terminal<'_, '_>) -> Result<Autoscroll> {
        let v = self.get::<ffi::SelectionGestureAutoscroll::Type>(
            terminal,
            ffi::SelectionGestureData::AUTOSCROLL,
        )?;
        Autoscroll::try_from(v).map_err(|_| Error::InvalidValue)
    }
    /// Get the current gesture behavior.
    pub fn behavior(&self, terminal: &Terminal<'_, '_>) -> Result<Behavior> {
        let v = self.get::<ffi::SelectionGestureBehavior::Type>(
            terminal,
            ffi::SelectionGestureData::BEHAVIOR,
        )?;
        Behavior::try_from(v).map_err(|_| Error::InvalidValue)
    }
    /// Get the current left-click anchor.
    ///
    /// Returns `None` if there is no valid active anchor.
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
        // SAFETY: We trust libghostty to return a GridRef
        // with the correct lifetime requirements.
        Ok(grid_ref.map(|v| unsafe { GridRef::from_raw(v) }))
    }
}
impl Drop for Gesture<'_> {
    fn drop(&mut self) {
        // NOTE: We can't pass the terminal in here to eagerly reclaim memory
        // taken up by the tracked grid refs, since that would require taking
        // a reference to the terminal and hold it within the selection gesture
        // struct and thereby prevent any mutation.
        //
        // However, leaking a bit of memory here is mostly fine since the
        // memory will eventually be reclaimed by the terminal anyway,
        // (which in the typical use case will be shortly after freeing
        // the selection gesture), and memory-conscious embedders can simply
        // call `reset` to manually reclaim memory if needed.
        unsafe {
            ffi::ghostty_selection_gesture_free(self.inner.as_raw(), std::ptr::null_mut());
        }
    }
}

#[derive(Debug)]
struct Event<'alloc> {
    inner: Object<'alloc, ffi::SelectionGestureEventImpl>,
}
impl<'alloc> Event<'alloc> {
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

        // SAFETY: We trust that libghostty will give us a
        // selection object with correct lifetimes.
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

/// Opaque handle to reusable input data for selection gesture press operations.
#[derive(Debug)]
pub struct PressEvent<'alloc> {
    base: Event<'alloc>,
}
impl<'alloc> PressEvent<'alloc> {
    /// Create a new selection gesture press event instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new selection gesture press event instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        Ok(Self {
            base: unsafe { Event::new_inner(alloc, ffi::SelectionGestureEventType::PRESS)? },
        })
    }

    /// Set the surface-space pointer position.
    #[inline]
    pub fn set_position(&mut self, x: f64, y: f64) -> Result<&mut Self> {
        let value = ffi::SurfacePosition { x, y };
        self.base
            .set(ffi::SelectionGestureEventOption::POSITION, &value)?;
        Ok(self)
    }

    /// Set the maximum repeat-click distance in pixels.
    #[inline]
    pub fn set_repeat_distance(&mut self, value: f64) -> Result<&mut Self> {
        self.base
            .set(ffi::SelectionGestureEventOption::REPEAT_DISTANCE, &value)?;
        Ok(self)
    }

    /// Set the monotonic event time.
    ///
    /// If unset, press treats the event as untimed and only single-click behavior
    /// is available.
    ///
    /// Intervals above [`u64::MAX`] nanoseconds in length will be
    /// silently truncated.
    #[inline]
    pub fn set_time(&mut self, value: Duration) -> Result<&mut Self> {
        let nanos = value.as_nanos() as u64;
        self.base
            .set(ffi::SelectionGestureEventOption::TIME_NS, &nanos)?;
        Ok(self)
    }

    /// Set the maximum interval between repeat clicks.
    ///
    /// Intervals above [`u64::MAX`] nanoseconds in length will be
    /// silently truncated.
    #[inline]
    pub fn set_repeat_interval(&mut self, value: Duration) -> Result<&mut Self> {
        let nanos = value.as_nanos() as u64;
        self.base
            .set(ffi::SelectionGestureEventOption::REPEAT_INTERVAL_NS, &nanos)?;
        Ok(self)
    }

    /// Set the word-boundary codepoints.
    ///
    /// The codepoints are copied into event-owned storage when set.
    /// If unset, operations that need word boundaries use Ghostty's defaults.
    #[inline]
    pub fn set_word_boundary_codepoints(&mut self, value: &[char]) -> Result<&mut Self> {
        let cp = ffi::Codepoints {
            // It is safe to cast char -> u32 in a readonly fashion.
            ptr: value.as_ptr().cast(),
            len: value.len(),
        };

        self.base.set(
            ffi::SelectionGestureEventOption::WORD_BOUNDARY_CODEPOINTS,
            &cp,
        )?;
        Ok(self)
    }

    /// Set the selection behavior table.
    ///
    /// If unset, press uses the default behavior table: cell, word, line.
    #[inline]
    pub fn set_behaviors(&mut self, value: &Behaviors) -> Result<&mut Self> {
        self.base
            .set(ffi::SelectionGestureEventOption::BEHAVIORS, &value.inner)?;
        Ok(self)
    }

    /// Apply a selection gesture press event and return the resulting selection snapshot.
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

/// Opaque handle to reusable input data for selection gesture release operations.
#[derive(Debug)]
pub struct ReleaseEvent<'alloc> {
    base: Event<'alloc>,
}
impl<'alloc> ReleaseEvent<'alloc> {
    /// Create a new selection gesture release event instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new selection gesture release event instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        Ok(Self {
            base: unsafe { Event::new_inner(alloc, ffi::SelectionGestureEventType::RELEASE)? },
        })
    }

    /// Apply a selection gesture release event and return the resulting selection snapshot.
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

        // A release event always returns None.
        _ = self.base.apply(gesture, terminal)?;
        Ok(())
    }
}

/// Opaque handle to reusable input data for selection gesture drag operations.
#[derive(Debug)]
pub struct DragEvent<'alloc> {
    base: Event<'alloc>,
}
impl<'alloc> DragEvent<'alloc> {
    /// Create a new selection gesture drag event instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new selection gesture drag event instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        Ok(Self {
            base: unsafe { Event::new_inner(alloc, ffi::SelectionGestureEventType::DRAG)? },
        })
    }

    /// Set the surface-space pointer position.
    #[inline]
    pub fn set_position(&mut self, x: f64, y: f64) -> Result<&mut Self> {
        let value = ffi::SurfacePosition { x, y };
        self.base
            .set(ffi::SelectionGestureEventOption::POSITION, &value)?;
        Ok(self)
    }

    /// Set the whether this drag should produce a rectangular selection.
    #[inline]
    pub fn set_rectangle(&mut self, value: bool) -> Result<&mut Self> {
        self.base
            .set(ffi::SelectionGestureEventOption::RECTANGLE, &value)?;
        Ok(self)
    }

    /// Set the word-boundary codepoints.
    ///
    /// The codepoints are copied into event-owned storage when set.
    /// If unset, operations that need word boundaries use Ghostty's defaults.
    #[inline]
    pub fn set_word_boundary_codepoints(&mut self, value: &[char]) -> Result<&mut Self> {
        let cp = ffi::Codepoints {
            // It is safe to cast char -> u32 in a readonly fashion.
            ptr: value.as_ptr().cast(),
            len: value.len(),
        };

        self.base.set(
            ffi::SelectionGestureEventOption::WORD_BOUNDARY_CODEPOINTS,
            &cp,
        )?;
        Ok(self)
    }

    /// Apply a selection gesture drag event and return the resulting selection snapshot.
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

/// Opaque handle to reusable input data for selection gesture autoscroll tick operations.
#[derive(Debug)]
pub struct AutoscrollTickEvent<'alloc> {
    base: Event<'alloc>,
}
impl<'alloc> AutoscrollTickEvent<'alloc> {
    /// Create a new selection gesture autoscroll tick event instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new selection gesture autoscroll tick event instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        Ok(Self {
            base: unsafe {
                Event::new_inner(alloc, ffi::SelectionGestureEventType::AUTOSCROLL_TICK)?
            },
        })
    }

    /// Set the surface-space pointer position.
    #[inline]
    pub fn set_position(&mut self, x: f64, y: f64) -> Result<&mut Self> {
        let value = ffi::SurfacePosition { x, y };
        self.base
            .set(ffi::SelectionGestureEventOption::POSITION, &value)?;
        Ok(self)
    }

    /// Set the whether this drag should produce a rectangular selection.
    #[inline]
    pub fn set_rectangle(&mut self, value: bool) -> Result<&mut Self> {
        self.base
            .set(ffi::SelectionGestureEventOption::RECTANGLE, &value)?;
        Ok(self)
    }

    /// Set the word-boundary codepoints.
    ///
    /// The codepoints are copied into event-owned storage when set.
    /// If unset, operations that need word boundaries use Ghostty's defaults.
    #[inline]
    pub fn set_word_boundary_codepoints(&mut self, value: &[char]) -> Result<&mut Self> {
        let cp = ffi::Codepoints {
            // It is safe to cast char -> u32 in a readonly fashion.
            ptr: value.as_ptr().cast(),
            len: value.len(),
        };

        self.base.set(
            ffi::SelectionGestureEventOption::WORD_BOUNDARY_CODEPOINTS,
            &cp,
        )?;
        Ok(self)
    }

    /// Apply a selection gesture autoscroll tick event and return the resulting selection snapshot.
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

/// Opaque handle to reusable input data for selection gesture deep press operations.
#[derive(Debug)]
pub struct DeepPressEvent<'alloc> {
    base: Event<'alloc>,
}
impl<'alloc> DeepPressEvent<'alloc> {
    /// Create a new selection gesture deep press event instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new selection gesture deep press event instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        Ok(Self {
            base: unsafe { Event::new_inner(alloc, ffi::SelectionGestureEventType::DEEP_PRESS)? },
        })
    }

    /// Set the word-boundary codepoints.
    ///
    /// The codepoints are copied into event-owned storage when set.
    /// If unset, operations that need word boundaries use Ghostty's defaults.
    #[inline]
    pub fn set_word_boundary_codepoints(&mut self, value: &[char]) -> Result<&mut Self> {
        let cp = ffi::Codepoints {
            // It is safe to cast char -> u32 in a readonly fashion.
            ptr: value.as_ptr().cast(),
            len: value.len(),
        };

        self.base.set(
            ffi::SelectionGestureEventOption::WORD_BOUNDARY_CODEPOINTS,
            &cp,
        )?;
        Ok(self)
    }

    /// Apply a selection gesture deep press event and return the resulting selection snapshot.
    #[inline]
    pub fn apply<'t>(
        &mut self,
        gesture: &mut Gesture<'_>,
        terminal: &'t Terminal<'_, '_>,
    ) -> Result<Option<Selection<'t>>> {
        self.base.apply(gesture, terminal)
    }
}

/// Current autoscroll direction for an active selection drag gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(u32)]
#[non_exhaustive]
pub enum Autoscroll {
    /// No selection autoscroll is requested.
    None,
    /// Selection dragging should autoscroll the viewport upward.
    Up,
    /// Selection dragging should autoscroll the viewport downward.
    Down,
}

/// Selection behavior chosen for a gesture's click sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(u32)]
#[non_exhaustive]
pub enum Behavior {
    /// Cell-granular drag selection.
    Cell = ffi::SelectionGestureBehavior::CELL,
    /// Word selection on press and word-granular drag selection.
    Word = ffi::SelectionGestureBehavior::WORD,
    /// Line selection on press and line-granular drag selection.
    Line = ffi::SelectionGestureBehavior::LINE,
    /// Semantic command output selection on press and drag.
    Output = ffi::SelectionGestureBehavior::OUTPUT,
}

/// Selection behaviors for single-, double-, and triple-click gestures.
#[derive(Clone, Copy, Debug, Default)]
pub struct Behaviors {
    inner: ffi::SelectionGestureBehaviors,
}
impl Behaviors {
    /// Create the default selection behaviors.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the single click behavior.
    pub fn with_single_click_behavior(mut self, behavior: Behavior) -> Self {
        self.inner.single_click = behavior.into();
        self
    }
    /// Set the double click behavior.
    pub fn with_double_click_behavior(mut self, behavior: Behavior) -> Self {
        self.inner.double_click = behavior.into();
        self
    }
    /// Set the triple click behavior.
    pub fn with_triple_click_behavior(mut self, behavior: Behavior) -> Self {
        self.inner.triple_click = behavior.into();
        self
    }
}
