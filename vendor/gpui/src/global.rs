use crate::{App, BorrowAppContext};

pub trait Global: 'static {
}

pub trait ReadGlobal {
    fn global(cx: &App) -> &Self;
}

impl<T: Global> ReadGlobal for T {
    fn global(cx: &App) -> &Self {
        cx.global::<T>()
    }
}

pub trait UpdateGlobal {
    fn update_global<C, F, R>(cx: &mut C, update: F) -> R
    where
        C: BorrowAppContext,
        F: FnOnce(&mut Self, &mut C) -> R;

    fn set_global<C>(cx: &mut C, global: Self)
    where
        C: BorrowAppContext;
}

impl<T: Global> UpdateGlobal for T {
    #[track_caller]
    fn update_global<C, F, R>(cx: &mut C, update: F) -> R
    where
        C: BorrowAppContext,
        F: FnOnce(&mut Self, &mut C) -> R,
    {
        cx.update_global(update)
    }

    fn set_global<C>(cx: &mut C, global: Self)
    where
        C: BorrowAppContext,
    {
        cx.set_global(global)
    }
}
