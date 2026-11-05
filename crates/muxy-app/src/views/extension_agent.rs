//! The picker that chooses which agent works on an extension, in the style of
//! the command palette.

use gpui::{AnyElement, AppContext, Context, Entity, Focusable, IntoElement, Window};
use muxy_ui::picker::{
    Picker, PickerAction, PickerConfig, PickerEvent, PickerItem, PickerLeading, PickerRow,
    PickerStatus,
};
use muxy_ui::tr;

use super::overlays::{Mark, Overlay, OverlayView};
use crate::extensions::authoring::{AgentTask, clone_name};
use crate::model::AppModel;

const CHECK_AGAIN: &str = "check-again";

pub(crate) struct ExtensionAgentPicker {
    pub(crate) task: AgentTask,
    pub(crate) picker: Entity<Picker>,
    /// The agent chosen here, while it gets ready. What it is waiting for is
    /// dropped when this changes or the picker closes.
    pub(crate) attempt: u64,
}

impl OverlayView for ExtensionAgentPicker {
    fn render(
        &self,
        _: Option<Mark>,
        _: &AppModel,
        _: &Window,
        _: &mut Context<AppModel>,
    ) -> AnyElement {
        self.picker.clone().into_any_element()
    }
}

pub(crate) fn placeholder(task: &AgentTask) -> gpui::SharedString {
    match task {
        AgentTask::Create => tr!("Create an extension with…"),
        AgentTask::Edit { name, .. } => tr!("Edit %@ with…", name),
        AgentTask::Clone { name, .. } => tr!("Clone %@ as %@ with…", name, clone_name(name)),
        AgentTask::Publish { name, .. } => tr!("Publish %@ with…", name),
    }
}

impl AppModel {
    pub(crate) fn open_extension_agent(&mut self, task: AgentTask, cx: &mut Context<Self>) {
        #[cfg(not(test))]
        self.ai.refresh_installed();
        let config = PickerConfig::new("extension-agent", placeholder(&task));
        let picker = cx.new(|cx| Picker::new(config, self.theme.clone(), self.metrics, cx));
        self.focus_owner.request(&picker.focus_handle(cx));
        self.overlay_subscription =
            Some(cx.subscribe(&picker, |model, _, event, cx| match event {
                PickerEvent::Confirmed(selection) => {
                    if let Some(agent) = crate::ai::provider(selection.id.as_ref()) {
                        model.choose_extension_agent(agent, cx);
                    }
                }
                PickerEvent::QueryChanged { .. } => model.update_extension_agents(cx),
                PickerEvent::FooterAction(action) if action.as_ref() == CHECK_AGAIN => {
                    model.refresh_extension_agents(cx);
                }
                PickerEvent::Dismissed => model.dismiss_overlay(cx),
                _ => {}
            }));
        self.open_overlay(
            Overlay::ExtensionAgent(ExtensionAgentPicker {
                task,
                picker,
                attempt: 0,
            }),
            cx,
        );
        self.update_extension_agents(cx);
        cx.notify();
    }

    /// Looks for agents again, for one the user has just installed.
    pub(crate) fn refresh_extension_agents(&mut self, cx: &mut Context<Self>) {
        #[cfg(not(test))]
        self.ai.refresh_installed();
        if let Some(Overlay::ExtensionAgent(open)) = &self.overlay {
            open.picker.update(cx, Picker::focus_query);
        }
        self.update_extension_agents(cx);
    }

    /// Lists the agents that match what was typed, or says what to install.
    pub(crate) fn update_extension_agents(&mut self, cx: &mut Context<Self>) {
        let agents = self.extension_agents();
        let Some(Overlay::ExtensionAgent(open)) = &self.overlay else {
            return;
        };
        if open.attempt != 0 {
            return;
        }
        let query = open.picker.read(cx).query().to_lowercase();
        let items = agents
            .iter()
            .filter(|agent| agent.name.to_lowercase().contains(&query))
            .map(|agent| {
                let mut row = PickerRow::new(agent.id, agent.name);
                row.leading = Some(PickerLeading::Asset(super::git::provider_icon_path(*agent)));
                PickerItem::Row(row)
            })
            .collect();
        let (status, footer) = if agents.is_empty() {
            (
                PickerStatus::Empty(tr!(
                    "No agent found. Install Claude Code, Codex, or another supported agent."
                )),
                vec![PickerAction::new(CHECK_AGAIN, tr!("Check Again"))],
            )
        } else {
            (PickerStatus::Ready, Vec::new())
        };
        open.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
            picker.set_footer_actions(footer, cx);
        });
        cx.notify();
    }
}
