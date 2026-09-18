use gpui::{
    AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Subscription, Task,
    Window,
};
use muxy_app_core::modal::{ModalItem, ModalOptions, ModalQuery, ModalState, ModalToken};
use muxy_ui::picker::{
    Picker, PickerAction, PickerConfig, PickerEvent, PickerItem, PickerRow, PickerStatus,
};
use muxy_ui::theme::{Metrics, Theme};

pub(crate) struct NativeModal {
    state: ModalState,
    picker: Entity<Picker>,
    completion: async_channel::Sender<Option<ModalItem>>,
    queries: async_channel::Sender<ModalQuery>,
    pending_queries: async_channel::Receiver<ModalQuery>,
    debounce: Option<Task<()>>,
    _subscription: Subscription,
}

impl NativeModal {
    pub(crate) fn new(
        options: ModalOptions,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> (
        Self,
        async_channel::Receiver<Option<ModalItem>>,
        async_channel::Receiver<ModalQuery>,
    ) {
        let (completion, result) = async_channel::bounded(1);
        let (queries, query_receiver) = async_channel::bounded(1);
        let state = ModalState::new(options, Vec::new(), true);
        let picker = cx.new(|cx| {
            Picker::new(
                PickerConfig::new("native-modal", state.options.placeholder.clone()),
                theme,
                metrics,
                cx,
            )
        });
        let subscription = cx.subscribe(&picker, |modal: &mut Self, _, event, cx| match event {
            PickerEvent::QueryChanged { query, .. } => modal.search(query, cx),
            PickerEvent::FooterAction(action) => {
                let mut options = modal.state.search_options();
                match action.as_ref() {
                    "case" => options.case_sensitive = !options.case_sensitive,
                    "word" => options.whole_word = !options.whole_word,
                    "regex" => options.regex = !options.regex,
                    _ => return,
                }
                let query = modal.state.query().to_owned();
                let request = modal.state.set_query(&query, options);
                modal.queue_query(request, cx);
                modal.refresh(cx);
            }
            PickerEvent::Confirmed(selection) => modal.complete(Some(selection.id.as_ref()), cx),
            PickerEvent::Dismissed => modal.complete(None, cx),
            _ => {}
        });
        let mut modal = Self {
            state,
            picker,
            completion,
            queries,
            pending_queries: query_receiver.clone(),
            debounce: None,
            _subscription: subscription,
        };
        modal.refresh(cx);
        (modal, result, query_receiver)
    }

    pub(crate) fn token(&self) -> ModalToken {
        self.state.token()
    }

    pub(crate) fn feed(
        &mut self,
        token: ModalToken,
        items: Vec<ModalItem>,
        cx: &mut Context<Self>,
    ) {
        if self.state.feed(token, items) {
            self.refresh(cx);
        }
    }

    pub(crate) fn finish(&mut self, token: ModalToken, cx: &mut Context<Self>) {
        if self.state.finish(token) {
            self.refresh(cx);
        }
    }

    fn search(&mut self, query: &str, cx: &mut Context<Self>) {
        let request = self.state.set_query(query, self.state.search_options());
        self.queue_query(request, cx);
        self.refresh(cx);
    }

    fn queue_query(&mut self, query: Option<ModalQuery>, cx: &mut Context<Self>) {
        let Some(query) = query else {
            return;
        };
        self.debounce = Some(cx.spawn(async move |modal, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let _ = modal.update(cx, |modal, _| {
                if modal.state.token() == query.token {
                    while modal.pending_queries.try_recv().is_ok() {}
                    let _ = modal.queries.try_send(query);
                }
            });
        }));
    }

    pub(crate) fn complete(&mut self, selection: Option<&str>, cx: &mut Context<Self>) {
        if let Some(result) = self.state.complete(selection) {
            self.debounce = None;
            let _ = self.completion.try_send(result);
            self.completion.close();
            self.queries.close();
            cx.notify();
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let items: Vec<_> = self
            .state
            .visible_items()
            .map(|item| {
                let mut row = PickerRow::new(item.id.clone(), item.title.clone());
                row.detail = item.subtitle.clone().map(Into::into);
                PickerItem::Row(row)
            })
            .collect();
        let status = if let Some(error) = self.state.search_error() {
            PickerStatus::Empty(error.to_owned().into())
        } else if !items.is_empty() {
            PickerStatus::Ready
        } else if self.state.loading() {
            PickerStatus::Loading("Loading…".into())
        } else {
            PickerStatus::Empty(
                if self.state.query().is_empty() {
                    self.state.options.empty_label.clone()
                } else {
                    self.state.options.no_match_label.clone()
                }
                .into(),
            )
        };
        let options = self.state.search_options();
        let mut actions = if self.state.options.search_toolbar {
            [
                ("case", "Case", options.case_sensitive),
                ("word", "Whole word", options.whole_word),
                ("regex", "Regex", options.regex),
            ]
            .map(|(id, label, active)| {
                PickerAction::new(
                    id,
                    if active {
                        format!("✓ {label}")
                    } else {
                        label.into()
                    },
                )
            })
            .to_vec()
        } else {
            Vec::new()
        };
        if self.state.loading() && !items.is_empty() {
            actions.insert(0, PickerAction::new("loading", "Loading…").disabled(true));
        }
        self.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
            picker.set_footer_actions(actions, cx);
        });
    }
}

impl Drop for NativeModal {
    fn drop(&mut self) {
        if let Some(result) = self.state.complete(None) {
            let _ = self.completion.try_send(result);
        }
    }
}

impl Focusable for NativeModal {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.picker.focus_handle(cx)
    }
}

impl Render for NativeModal {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.picker.clone()
    }
}
