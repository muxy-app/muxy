use std::collections::{BTreeSet, HashSet};

pub const P2_PHASE3_HEADS: [&str; 13] = [
    "list-projects",
    "switch-project",
    "list-worktrees",
    "switch-worktree",
    "refresh-worktrees",
    "list-workspaces",
    "create-workspace",
    "switch-workspace",
    "rename-workspace",
    "delete-workspace",
    "create-project",
    "attach-project",
    "detach-project",
];

pub const P2_PHASE4_HEADS: [&str; 8] = [
    "split-right",
    "split-down",
    "send",
    "send-keys",
    "read-screen",
    "close-pane",
    "rename-pane",
    "list-panes",
];

pub const P2_PHASE5_HEADS: [&str; 12] = [
    "list-tabs",
    "switch-tab",
    "new-tab",
    "next-tab",
    "previous-tab",
    "tab-rename",
    "tab-set-color",
    "tab-set-icon",
    "tab-pin",
    "tab-unpin",
    "tab-close",
    "tab-move",
];

pub const P2_PERMISSION_REQUIREMENTS: [(&str, Option<&str>); 33] = [
    ("list-projects", Some("projects:read")),
    ("switch-project", Some("projects:write")),
    ("list-worktrees", Some("worktrees:read")),
    ("switch-worktree", Some("worktrees:write")),
    ("refresh-worktrees", Some("worktrees:write")),
    ("list-workspaces", Some("projects:read")),
    ("create-workspace", Some("projects:write")),
    ("switch-workspace", Some("projects:write")),
    ("rename-workspace", Some("projects:write")),
    ("delete-workspace", Some("projects:write")),
    ("create-project", Some("projects:write")),
    ("attach-project", Some("projects:write")),
    ("detach-project", Some("projects:write")),
    ("split-right", Some("panes:write")),
    ("split-down", Some("panes:write")),
    ("send", Some("panes:write")),
    ("send-keys", Some("panes:write")),
    ("read-screen", Some("panes:read")),
    ("close-pane", Some("panes:write")),
    ("rename-pane", Some("panes:write")),
    ("list-panes", Some("panes:read")),
    ("list-tabs", Some("tabs:read")),
    ("switch-tab", Some("tabs:write")),
    ("new-tab", Some("tabs:write")),
    ("next-tab", Some("tabs:write")),
    ("previous-tab", Some("tabs:write")),
    ("tab-rename", Some("tabs:write")),
    ("tab-set-color", Some("tabs:write")),
    ("tab-set-icon", Some("tabs:write")),
    ("tab-pin", Some("tabs:write")),
    ("tab-unpin", Some("tabs:write")),
    ("tab-close", Some("tabs:write")),
    ("tab-move", Some("tabs:write")),
];

pub const ROADMAP_DEFERRED_LEGACY_HEADS: [(&str, &str); 4] = [
    ("create-worktree", "P3"),
    ("list-sessions", "P8"),
    ("kill-session", "P8"),
    ("open-tab", "P10"),
];

pub const P9_BROWSER_HEADS: [&str; 36] = [
    "browser.open",
    "browser.navigate",
    "browser.list",
    "browser.read",
    "browser.close",
    "browser.eval",
    "browser.click",
    "browser.type",
    "browser.waitFor",
    "browser.getText",
    "browser.getHTML",
    "browser.getAttribute",
    "browser.reload",
    "browser.back",
    "browser.forward",
    "browser.waitForNavigation",
    "browser.screenshot",
    "browser.storage.get",
    "browser.storage.set",
    "browser.storage.clear",
    "browser.cookies.get",
    "browser.cookies.set",
    "browser.cookies.delete",
    "browser.cookies.clear",
    "browser.wait",
    "browser.fill",
    "browser.press",
    "browser.select",
    "browser.hover",
    "browser.scrollIntoView",
    "browser.setChecked",
    "browser.is",
    "browser.getValue",
    "browser.getCount",
    "browser.find",
    "browser.snapshot",
];

pub const P10_EXTENSION_API_HEADS: [&str; 96] = [
    "exec",
    "agents.list",
    "http.fetch",
    "dialog.confirm",
    "dialog.alert",
    "dialog.prompt",
    "dialog.pickFolder",
    "shortcuts.register",
    "shortcuts.unregister",
    "shortcuts.list",
    "storage.get",
    "storage.set",
    "storage.delete",
    "storage.keys",
    "modal.open",
    "modal.feed",
    "modal.finish",
    "modal.await",
    "modal.openWebview",
    "modal.awaitWebview",
    "modal.submitWebview",
    "modal.closeWebview",
    "extension.settings.get",
    "extension.settings.set",
    "extension.statusbar.set",
    "panel.open",
    "panel.close",
    "panel.toggle",
    "popover.close",
    "popover.resize",
    "topbar.set",
    "statusbar.set",
    "tabs.open",
    "projects.delete",
    "projects.add",
    "projects.create",
    "projects.rename",
    "projects.setColor",
    "projects.setIcon",
    "projects.setLogo",
    "projects.reorder",
    "projects.attach",
    "projects.detach",
    "workspaces.list",
    "workspaces.create",
    "workspaces.switch",
    "workspaces.rename",
    "workspaces.delete",
    "lifecycle.ackBeforeClose",
    "lifecycle.resolveBeforeClose",
    "lifecycle.closeSelf",
    "files.list",
    "files.read",
    "files.stat",
    "files.write",
    "files.mkdir",
    "files.rename",
    "files.move",
    "files.delete",
    "git.status",
    "git.diff",
    "git.repoInfo",
    "git.log",
    "git.branches",
    "git.currentBranch",
    "git.aheadBehind",
    "git.pr.info",
    "git.pr.number",
    "git.pr.diff",
    "git.pr.list",
    "git.worktrees",
    "git.init",
    "git.stage",
    "git.unstage",
    "git.discard",
    "git.commit",
    "git.push",
    "git.pull",
    "git.branch.create",
    "git.branch.switch",
    "git.pr.create",
    "git.pr.merge",
    "git.pr.close",
    "git.worktree.add",
    "git.worktree.remove",
    "git.worktree.switch",
    "git.remoteBranches",
    "git.branch.delete",
    "git.branch.deleteRemote",
    "git.checkout",
    "git.cherryPick",
    "git.revert",
    "git.tag.create",
    "git.pr.checkout",
    "git.pr.checkoutWorktree",
    "gh.user",
];

pub const TRANSPORT_STICKY_HEADS: [&str; 2] = ["identify", "subscribe"];
pub const TRANSPORT_IDENTIFIED_INGRESS_HEADS: [&str; 2] = ["invoke-result", "extension-event"];
pub const NO_RESPONSE_ROUTES: [&str; 2] = ["open-project", "install-extension"];
pub const OUTBOUND_PUSH_HEADS: [&str; 5] = [
    "invoke",
    "modal-result",
    "modal-query",
    "event",
    "extension-event",
];
pub const GENERIC_INGRESS_CLASSES: [&str; 2] = ["agent-hook-v3", "legacy-notification"];
pub const AGENT_HOOK_RESPONSE_CLASSES: [&str; 1] = ["agent-hook-v3-ack"];
pub const RECOGNITION_ODDITIES: [&str; 2] = ["config-export", "config-import"];

pub fn recognized_command_heads() -> HashSet<String> {
    let recognized = P2_PHASE3_HEADS
        .into_iter()
        .chain(P2_PHASE4_HEADS)
        .chain(P2_PHASE5_HEADS)
        .chain(
            ROADMAP_DEFERRED_LEGACY_HEADS
                .into_iter()
                .map(|(head, _)| head),
        )
        .chain(P9_BROWSER_HEADS)
        .chain(P10_EXTENSION_API_HEADS)
        .map(str::to_owned)
        .collect::<HashSet<_>>();
    assert_eq!(recognized.len(), 169);
    for head in TRANSPORT_STICKY_HEADS
        .into_iter()
        .chain(TRANSPORT_IDENTIFIED_INGRESS_HEADS)
        .chain(NO_RESPONSE_ROUTES)
        .chain(OUTBOUND_PUSH_HEADS)
        .chain(GENERIC_INGRESS_CLASSES)
        .chain(AGENT_HOOK_RESPONSE_CLASSES)
        .chain(RECOGNITION_ODDITIES)
    {
        assert!(!recognized.contains(head));
    }
    recognized
}

pub fn required_permissions(command: &str) -> Vec<&'static str> {
    let parts = command.split('|').collect::<Vec<_>>();
    let head = parts.first().copied().unwrap_or_default();
    let mut permissions = P2_PERMISSION_REQUIREMENTS
        .iter()
        .find_map(|(candidate, permission)| (*candidate == head).then_some(*permission))
        .flatten()
        .into_iter()
        .collect::<Vec<_>>();
    if matches!(head, "split-right" | "split-down")
        && crate::socket::commands::panes::split_has_startup_command(&parts)
    {
        permissions.push("commands:exec");
    }
    permissions
}

pub fn denied_permission(command: &str, granted: &BTreeSet<String>) -> Option<&'static str> {
    required_permissions(command)
        .into_iter()
        .find(|permission| !granted.contains(*permission))
}

pub fn deferred_reply(head: &str) -> String {
    let owner = if P2_PHASE3_HEADS.contains(&head) {
        "P2 Phase 3"
    } else if P2_PHASE4_HEADS.contains(&head) {
        "P2 Phase 4"
    } else if P2_PHASE5_HEADS.contains(&head) {
        "P2 Phase 5"
    } else if let Some((_, owner)) = ROADMAP_DEFERRED_LEGACY_HEADS
        .iter()
        .find(|(candidate, _)| *candidate == head)
    {
        owner
    } else if P9_BROWSER_HEADS.contains(&head) {
        "P9"
    } else if P10_EXTENSION_API_HEADS.contains(&head) {
        "P10"
    } else {
        "a future phase"
    };
    format!("error:command {head} is deferred to {owner}")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn recognized_catalog_matches_the_frozen_inventory() {
        let p2 = P2_PHASE3_HEADS
            .into_iter()
            .chain(P2_PHASE4_HEADS)
            .chain(P2_PHASE5_HEADS)
            .collect::<BTreeSet<_>>();
        let legacy = p2
            .iter()
            .copied()
            .chain(
                ROADMAP_DEFERRED_LEGACY_HEADS
                    .into_iter()
                    .map(|(head, _)| head),
            )
            .collect::<BTreeSet<_>>();
        assert_eq!(P2_PHASE3_HEADS.len(), 13);
        assert_eq!(P2_PHASE4_HEADS.len(), 8);
        assert_eq!(P2_PHASE5_HEADS.len(), 12);
        assert_eq!(p2.len(), 33);
        assert_eq!(legacy.len(), 37);
        assert_eq!(P9_BROWSER_HEADS.len(), 36);
        assert_eq!(P10_EXTENSION_API_HEADS.len(), 96);
        let recognized = recognized_command_heads();
        assert_eq!(recognized.len(), 169);
        let sorted = recognized
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            sorted,
            legacy
                .into_iter()
                .chain(P9_BROWSER_HEADS)
                .chain(P10_EXTENSION_API_HEADS)
                .collect()
        );
        let fingerprint = sorted
            .iter()
            .flat_map(|head| head.bytes().chain(std::iter::once(0)))
            .fold(14_695_981_039_346_656_037_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(1_099_511_628_211)
            });
        assert_eq!(fingerprint, 16_943_492_558_170_763_262);
    }

    #[test]
    fn every_p2_head_has_its_locked_slice_owner() {
        for head in P2_PHASE3_HEADS {
            assert_eq!(
                deferred_reply(head),
                format!("error:command {head} is deferred to P2 Phase 3")
            );
        }
        for head in P2_PHASE4_HEADS {
            assert_eq!(
                deferred_reply(head),
                format!("error:command {head} is deferred to P2 Phase 4")
            );
        }
        for head in P2_PHASE5_HEADS {
            assert_eq!(
                deferred_reply(head),
                format!("error:command {head} is deferred to P2 Phase 5")
            );
        }
    }

    #[test]
    fn every_p2_head_has_an_explicit_permission_decision() {
        let p2 = P2_PHASE3_HEADS
            .into_iter()
            .chain(P2_PHASE4_HEADS)
            .chain(P2_PHASE5_HEADS)
            .collect::<BTreeSet<_>>();
        let mapped = P2_PERMISSION_REQUIREMENTS
            .into_iter()
            .map(|(head, _)| head)
            .collect::<BTreeSet<_>>();
        assert_eq!(mapped, p2);
        assert_eq!(required_permissions("list-panes"), ["panes:read"]);
        assert_eq!(
            required_permissions("split-right|echo|ready"),
            ["panes:write", "commands:exec"]
        );
        assert_eq!(required_permissions("split-right"), ["panes:write"]);
        assert_eq!(
            denied_permission("split-right|echo ready", &BTreeSet::new()),
            Some("panes:write")
        );
        assert_eq!(
            denied_permission(
                "split-right|echo ready",
                &BTreeSet::from(["panes:write".to_owned()])
            ),
            Some("commands:exec")
        );
        assert_eq!(
            denied_permission(
                "split-right|echo ready",
                &BTreeSet::from(["panes:write".to_owned(), "commands:exec".to_owned()])
            ),
            None
        );
    }

    #[test]
    fn every_roadmap_deferred_head_names_its_owner() {
        for (head, owner) in ROADMAP_DEFERRED_LEGACY_HEADS {
            assert_eq!(
                deferred_reply(head),
                format!("error:command {head} is deferred to {owner}")
            );
        }
        for head in P9_BROWSER_HEADS {
            assert!(deferred_reply(head).ends_with("P9"));
        }
        for head in P10_EXTENSION_API_HEADS {
            assert!(deferred_reply(head).ends_with("P10"));
        }
    }

    #[test]
    fn special_routes_are_exact_and_outside_the_169_app_commands() {
        assert_eq!(TRANSPORT_STICKY_HEADS, ["identify", "subscribe"]);
        assert_eq!(
            TRANSPORT_IDENTIFIED_INGRESS_HEADS,
            ["invoke-result", "extension-event"]
        );
        assert_eq!(NO_RESPONSE_ROUTES, ["open-project", "install-extension"]);
        assert_eq!(
            OUTBOUND_PUSH_HEADS,
            [
                "invoke",
                "modal-result",
                "modal-query",
                "event",
                "extension-event"
            ]
        );
        assert_eq!(
            GENERIC_INGRESS_CLASSES,
            ["agent-hook-v3", "legacy-notification"]
        );
        assert_eq!(AGENT_HOOK_RESPONSE_CLASSES, ["agent-hook-v3-ack"]);
        assert_eq!(RECOGNITION_ODDITIES, ["config-export", "config-import"]);
        let recognized = recognized_command_heads();
        for head in TRANSPORT_STICKY_HEADS
            .into_iter()
            .chain(TRANSPORT_IDENTIFIED_INGRESS_HEADS)
            .chain(NO_RESPONSE_ROUTES)
            .chain(OUTBOUND_PUSH_HEADS)
            .chain(GENERIC_INGRESS_CLASSES)
            .chain(AGENT_HOOK_RESPONSE_CLASSES)
            .chain(RECOGNITION_ODDITIES)
        {
            assert!(!recognized.contains(head), "{head}");
        }
    }
}
