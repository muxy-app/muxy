use gpui::{Entity, TestAppContext, VisualTestContext};
use muxy_app_core::AppState;
use muxy_app_core::extensions::{Extension, Manifest};
use serde_json::json;

use super::super::{Call, Placement, Reply};
use crate::model::AppModel;
use crate::model::preferences::Change;
use crate::model::tests::stub_boot;

fn open(cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        let manifest: Manifest = serde_json::from_value(json!({
            "permissions": ["panels:write"],
            "commands": [{"id": "toggle", "title": "Toggle"}],
            "topbarItems": [{"id": "star", "icon": "star", "command": "toggle"}],
            "railItems": [
                {"id": "notes", "icon": "note.text", "command": "toggle", "tooltip": "Notes"},
                {"id": "later", "icon": "bolt", "command": "toggle", "visible": false},
            ],
        }))
        .expect("manifest");
        let registry = &mut model.extensions.registry;
        registry.extensions.insert(
            "sample".into(),
            Extension {
                name: "sample".into(),
                version: "1.0.0".into(),
                directory: std::env::temp_dir(),
                manifest,
            },
        );
        registry.set_enabled("sample", true).expect("enabled");
    });
    cx.simulate_resize(gpui::size(gpui::px(1200.0), gpui::px(800.0)));
    cx.run_until_parked();
    (view, cx)
}

fn keys(items: &[super::super::items::Item]) -> Vec<&str> {
    items.iter().map(|item| item.key.as_str()).collect()
}

fn set_rail_item(
    view: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    view.update(cx, |model, cx| {
        let (reply, _replies) = std::sync::mpsc::sync_channel(1);
        let call = Call {
            owner: "sample".into(),
            epoch: model.extensions.epoch_for("sample"),
            generation: model.servers.local.generation,
            project: model.state.current_project().id,
            verb: "rail.set".into(),
            args,
            reply: Reply::Script(1, reply),
            approved: true,
        };
        model.set_item(&call, cx)
    })
}

#[gpui::test]
fn rail_items_stay_off_the_top_bar_and_the_rail_starts_collapsed(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.read_with(cx, |model, _| {
        assert!(model.appearance.rail_enabled);
        assert!(!model.appearance.rail_expanded);
        assert_eq!(keys(&model.topbar_items()), ["top:sample:star"]);
        assert_eq!(keys(&model.rail_items()), ["rail:sample:notes"]);
        assert_eq!(model.rail_items()[0].placement, Placement::Rail);
        assert!(!model.rail_shown());
    });
    assert!(cx.debug_bounds("extension-toolbar-item").is_some());
    assert!(cx.debug_bounds("rail").is_none());
    let toggle = cx.debug_bounds("toggle-rail").expect("rail toggle");
    let sessions = cx.debug_bounds("toggle-agent-sessions").expect("sessions");
    assert!(
        toggle.left() > sessions.left(),
        "the toggle ends the top bar"
    );

    cx.simulate_click(toggle.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.rail_shown());
        assert!(
            model.settings.appearance.rail_expanded,
            "the choice is saved"
        );
    });
    let rail = cx.debug_bounds("rail").expect("open rail");
    let item = cx.debug_bounds("rail-item").expect("rail item");
    assert!(rail.contains(&item.center()));
    assert!(
        rail.right() >= gpui::px(1199.0),
        "the rail sits on the right edge"
    );

    cx.simulate_click(toggle.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(!model.rail_shown());
        assert!(!model.settings.appearance.rail_expanded);
    });
}

#[gpui::test]
fn turning_the_rail_off_moves_its_items_to_the_top_bar(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.update(cx, |model, cx| {
        model.toggle_rail(cx);
        model.change_preference(Change::Rail(false), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(
            !model.settings.appearance.rail_enabled,
            "the choice is saved"
        );
        assert_eq!(
            keys(&model.topbar_items()),
            ["top:sample:star", "rail:sample:notes"]
        );
        assert!(model.rail_items().is_empty());
        assert!(!model.rail_shown());
        assert!(model.extension_item_shown("rail:sample:notes", Placement::Topbar));
        assert!(
            !model.extension_item_shown("rail:sample:notes", Placement::Rail),
            "a popover opened from the rail closes once its item moves"
        );
    });
}

#[gpui::test]
fn rail_items_change_at_runtime_and_the_toggle_follows(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.update(cx, AppModel::toggle_rail);
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, _| {
        model.extension_item_shown("rail:sample:notes", Placement::Rail)
    }));

    set_rail_item(&view, cx, json!({"id": "later", "visible": true})).expect("show");
    set_rail_item(&view, cx, json!({"id": "notes", "visible": false})).expect("hide");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(keys(&model.rail_items()), ["rail:sample:later"]);
        assert!(!model.extension_item_shown("rail:sample:notes", Placement::Rail));
    });

    set_rail_item(&view, cx, json!({"id": "later", "visible": false})).expect("hide");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(
            model.rail_items().is_empty(),
            "the toggle hides with nothing to show"
        );
        assert!(!model.rail_shown());
    });

    assert_eq!(
        set_rail_item(&view, cx, json!({"id": "star", "visible": true})),
        Err("unknown rail item 'star'".into()),
        "topbar items are not rail items"
    );
}
