use gpui::{
    AnyElement, AppContext, Context, Entity, IntoElement, Render, Subscription, WeakEntity, Window,
    div,
};

pub(crate) struct CachedView<T> {
    #[cfg(test)]
    pub(crate) render_count: usize,
    parent: WeakEntity<T>,
    render: fn(&mut T, &mut Window, &mut Context<T>) -> AnyElement,
    _subscription: Subscription,
}

impl<T: 'static> CachedView<T> {
    pub(crate) fn new(
        render: fn(&mut T, &mut Window, &mut Context<T>) -> AnyElement,
        cx: &mut Context<T>,
    ) -> Entity<Self> {
        let parent = cx.entity();
        cx.new(|cx| Self {
            #[cfg(test)]
            render_count: 0,
            parent: parent.downgrade(),
            render,
            _subscription: cx.observe(&parent, |_, _, cx| cx.notify()),
        })
    }
}

impl<T: 'static> Render for CachedView<T> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(test)]
        {
            self.render_count += 1;
        }
        self.parent
            .update(cx, |parent, cx| (self.render)(parent, window, cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}
