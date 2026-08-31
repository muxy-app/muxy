use std::path::PathBuf;

use gpui::{Context, Task};
use muxy_proto::server::{
    CommandReply, ExtensionSnapshot, IncomingRequest, ServerConfig, ServerError, ServerLimits,
    SocketServer, SocketServerHandle,
};

use crate::socket::catalog;
use crate::socket::commands::panes::{self, PaneCommand};
use crate::socket::commands::projects::{self, ProjectCommand};
use crate::socket::commands::{CommandResult, sessions, tabs, workspaces};
use crate::socket::ingress::AgentHookResolution;
use crate::views::window::MainWindow;

pub struct SocketBootstrap {
    server: SocketServer,
    handle: SocketServerHandle,
    incoming: async_channel::Receiver<IncomingRequest>,
    socket_path: PathBuf,
}

pub struct SocketRuntime {
    _server: SocketServer,
    _handle: SocketServerHandle,
    _socket_path: PathBuf,
    _pump: Task<()>,
}

pub fn start(socket_path: PathBuf) -> Result<SocketBootstrap, ServerError> {
    let config = ServerConfig {
        socket_path: socket_path.clone(),
        recognized_command_heads: catalog::recognized_command_heads(),
        no_response_command_routes: catalog::NO_RESPONSE_ROUTES
            .into_iter()
            .map(str::to_owned)
            .collect(),
        limits: ServerLimits::default(),
        initial_extension_snapshot: ExtensionSnapshot::default(),
    };
    let (server, handle, incoming) = SocketServer::start(config)?;
    Ok(SocketBootstrap {
        server,
        handle,
        incoming,
        socket_path,
    })
}

impl SocketBootstrap {
    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }
}

impl SocketRuntime {
    pub fn attach(bootstrap: SocketBootstrap, cx: &mut Context<MainWindow>) -> Self {
        let incoming = bootstrap.incoming;
        let pump = cx.spawn(async move |window, cx| {
            while let Ok(request) = incoming.recv().await {
                if window
                    .update(cx, |window, cx| window.handle_socket_request(request, cx))
                    .is_err()
                {
                    return;
                }
            }
        });
        Self {
            _server: bootstrap.server,
            _handle: bootstrap.handle,
            _socket_path: bootstrap.socket_path,
            _pump: pump,
        }
    }
}

impl MainWindow {
    fn handle_socket_request(&mut self, request: IncomingRequest, cx: &mut Context<Self>) {
        match request {
            IncomingRequest::AppCommand(request) => {
                if request.origin.extension_id.is_some()
                    && let Some(permission) = catalog::denied_permission(
                        &request.command,
                        &request.origin.granted_permissions,
                    )
                {
                    request.responder.respond(CommandReply::new(format!(
                        "error:permission denied ({permission})"
                    )));
                    return;
                }
                let parts: Vec<&str> = request.command.split('|').collect();
                let head = parts.first().copied().unwrap_or_default();
                if let Some(command) = panes::handle(head, &parts, self) {
                    match command {
                        PaneCommand::Immediate(result) => {
                            self.finish_socket_command(result, request.responder, cx);
                        }
                        PaneCommand::Surface(command) => {
                            if !self.terminal_runtime.surfaces.request_materialization(
                                &self.state.tab_workspaces,
                                &command.pane_id,
                            ) {
                                request.responder.respond(CommandReply::new(format!(
                                    "error:pane launch context unavailable {}",
                                    command.pane_id
                                )));
                                return;
                            }
                            let active_window = self.window_handle.downcast::<MainWindow>();
                            cx.notify();
                            cx.spawn(async move |window, cx| {
                                for _ in 0..40 {
                                    if let Some(active_window) = active_window {
                                        let _ = active_window.update(
                                            cx,
                                            |window, native_window, cx| {
                                                window.reconcile_terminals(native_window, cx);
                                            },
                                        );
                                    }
                                    let reply = window
                                        .update(cx, |window, cx| {
                                            let reply = panes::perform_surface(window, &command);
                                            if reply.is_none() {
                                                cx.notify();
                                            }
                                            reply
                                        })
                                        .ok()
                                        .flatten();
                                    if let Some(reply) = reply {
                                        request.responder.respond(CommandReply::new(reply));
                                        return;
                                    }
                                    cx.background_executor()
                                        .timer(std::time::Duration::from_millis(50))
                                        .await;
                                }
                                request.responder.respond(CommandReply::new(format!(
                                    "error:pane surface not ready {} (waited 2.0s)",
                                    command.pane_id
                                )));
                            })
                            .detach();
                        }
                    }
                    return;
                }
                if let Some(result) = tabs::handle(head, &parts, self) {
                    self.finish_socket_command(result, request.responder, cx);
                    return;
                }
                if let Some(result) = workspaces::handle(head, &parts, &mut self.state) {
                    self.finish_socket_command(result, request.responder, cx);
                    return;
                }
                if let Some(result) = sessions::handle(
                    head,
                    &parts,
                    &mut self.sessions,
                    &mut self.state.tab_workspaces,
                ) {
                    self.finish_socket_command(result, request.responder, cx);
                    return;
                }
                if let Some(result) = projects::handle(head, &parts, &mut self.state) {
                    match result {
                        ProjectCommand::Immediate(result) => {
                            self.finish_socket_command(result, request.responder, cx);
                        }
                        ProjectCommand::Refresh(project) => {
                            self.request_worktree_refresh(project.id, Some(request.responder), cx);
                        }
                        ProjectCommand::Create(create) => {
                            self.request_worktree_creation(*create, request.responder, cx);
                        }
                    }
                    return;
                }
                request
                    .responder
                    .respond(CommandReply::new(catalog::deferred_reply(head)));
            }
            IncomingRequest::NoResponseCommand(command) => match command.head.as_str() {
                "open-project" => self.open_project_from_socket(&command.payload, cx),
                "install-extension" => {}
                _ => {}
            },
            IncomingRequest::LegacyNotification(notification) => {
                let record = self.state.socket_ingress.push_legacy(notification);
                let timestamp = muxy_core::store::reference_now();
                let resolved = crate::notifications::resolve_legacy_notification(
                    &record,
                    timestamp,
                    |pane_id| self.state.notification_target_for_pane(pane_id),
                    || self.state.active_first_terminal_notification_target(),
                );
                if let Some(resolved) = resolved {
                    self.submit_notification(resolved, false, cx);
                }
            }
            IncomingRequest::AgentHook(event) => {
                let resolution = resolve_agent_hook(&event, |pid| {
                    self.terminal_runtime
                        .surfaces
                        .panes_matching_foreground_pid(pid)
                });
                let record = self.state.socket_ingress.push_agent_hook(event, resolution);
                let timestamp = muxy_core::store::reference_now();
                let resolved = crate::notifications::resolve_agent_hook_notification(
                    &record,
                    timestamp,
                    |pane_id| self.state.notification_target_for_pane(pane_id),
                    || self.state.active_first_terminal_notification_target(),
                );
                if let Some(resolved) = resolved {
                    self.submit_notification(resolved, false, cx);
                }
            }
            IncomingRequest::ExtensionLocalEvent(event) => {
                self.state.socket_ingress.push_extension_event(event);
            }
        }
    }

    fn finish_socket_command(
        &mut self,
        result: CommandResult,
        responder: muxy_proto::server::CommandResponder,
        cx: &mut Context<Self>,
    ) {
        if result.changed {
            self.socket_state_changed(cx);
        }
        responder.respond(CommandReply::new(result.reply));
    }

    fn socket_state_changed(&mut self, cx: &mut Context<Self>) {
        self.state.workspace.sort();
        self.refresh_project_truth(None, cx);
        cx.set_menus(crate::views::window::menu_bar::menus(&self.state));
        cx.activate(true);
        cx.notify();
    }

    fn open_project_from_socket(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(path) = validated_project_path(path) else {
            return;
        };
        let existing = self
            .state
            .workspace
            .contains_path(&path)
            .map(|project| project.id.clone());
        let project_id = match existing {
            Some(project_id) => Some(project_id),
            None => {
                let name = muxy_api::picker::path_service::last_component(&path);
                self.state.workspace.add(name, path.clone())
            }
        };
        let Some(project_id) = project_id else {
            return;
        };
        if projects::ensure_project_context(&mut self.state, &project_id).is_err() {
            return;
        }
        if let Some(group_id) = self
            .state
            .workspace
            .active_group_id
            .clone()
            .filter(|id| self.state.workspace.groups.is_local(id))
            && let Err(error) = self
                .state
                .workspace
                .groups
                .try_add_project(&project_id, &group_id)
        {
            log::warn!("failed to save socket-opened project workspace: {error}");
        }
        self.socket_state_changed(cx);
    }
}

fn resolve_agent_hook(
    event: &muxy_proto::hook::AgentHookEvent,
    mut matching_panes: impl FnMut(u64) -> Vec<String>,
) -> AgentHookResolution {
    if event.test {
        return AgentHookResolution::Test;
    }
    if let Some(pane_id) = event.pane_id.clone() {
        return AgentHookResolution::ExplicitPane(pane_id);
    }
    event
        .pids
        .iter()
        .filter_map(|pid| u64::try_from(*pid).ok())
        .find_map(|pid| {
            matching_panes(pid)
                .into_iter()
                .next()
                .map(|pane_id| AgentHookResolution::ProcessMatch { pane_id, pid })
        })
        .unwrap_or(AgentHookResolution::Unresolved)
}

fn validated_project_path(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let standardized = crate::socket::commands::target::standardize_path(path)?;
    std::fs::metadata(&standardized)
        .ok()
        .filter(|metadata| metadata.is_dir())
        .map(|_| standardized)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn project_opening_accepts_only_existing_directories() {
        let directory = TempDir::new().unwrap();
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).unwrap();
        let input = format!("{}/./other/../nested", directory.path().display());
        assert_eq!(
            validated_project_path(&input),
            Some(nested.to_string_lossy().into_owned())
        );
        let file = directory.path().join("file");
        fs::write(&file, b"file").unwrap();
        assert_eq!(validated_project_path(&file.to_string_lossy()), None);
        assert_eq!(validated_project_path(""), None);
        assert_eq!(
            validated_project_path("/definitely/missing/muxy/path"),
            None
        );
    }

    #[test]
    fn hook_resolution_prioritizes_test_explicit_and_wire_ordered_pid_matches() {
        let mut event = muxy_proto::hook::AgentHookEvent {
            v: 3,
            kind: "agent_event".to_owned(),
            id: None,
            provider: "sample".to_owned(),
            pane_id: None,
            phase: muxy_proto::hook::AgentHookPhase::Working,
            title: String::new(),
            body: String::new(),
            pids: vec![-1, 20, 10],
            ts: 0,
            test: false,
        };
        assert_eq!(
            resolve_agent_hook(&event, |pid| match pid {
                20 => vec!["A-PANE".to_owned()],
                10 => vec!["B-PANE".to_owned()],
                _ => Vec::new(),
            }),
            AgentHookResolution::ProcessMatch {
                pane_id: "A-PANE".to_owned(),
                pid: 20
            }
        );
        event.pane_id = Some("EXPLICIT".to_owned());
        assert_eq!(
            resolve_agent_hook(&event, |_| vec!["OTHER".to_owned()]),
            AgentHookResolution::ExplicitPane("EXPLICIT".to_owned())
        );
        event.test = true;
        assert_eq!(
            resolve_agent_hook(&event, |_| vec!["OTHER".to_owned()]),
            AgentHookResolution::Test
        );
        event.test = false;
        event.pane_id = None;
        assert_eq!(
            resolve_agent_hook(&event, |_| Vec::new()),
            AgentHookResolution::Unresolved
        );
    }

    #[test]
    fn server_config_uses_production_limits_and_empty_extension_snapshot() {
        let limits = ServerLimits::default();
        assert_eq!(limits.max_input_bytes, 128 * 1024);
        assert_eq!(limits.max_in_flight_commands, 8);
        assert_eq!(limits.dropped_notification_disconnect_threshold, 100);
        assert_eq!(limits.invoke_timeout, std::time::Duration::from_secs(15));
        assert!(ExtensionSnapshot::default().entries.is_empty());
    }

    #[test]
    fn runtime_source_has_no_socket_filename_policy() {
        let source = include_str!("runtime.rs");
        assert!(!source.contains(concat!("muxy-dev", ".sock")));
        assert!(!source.contains(concat!("muxy", ".sock")));
        assert!(!source.contains(concat!("debug", "_assertions")));
    }

    #[test]
    fn path_type_is_neutral() {
        let path = Path::new("/tmp/sample");
        assert_eq!(
            crate::socket::commands::target::standardize_path(&path.to_string_lossy()),
            Some("/tmp/sample".to_owned())
        );
    }
}
