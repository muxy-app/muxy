//! Remote servers: other computers' servers, which the sidebar's Remote
//! section adds, edits, tests, and forgets. Their entries live in
//! `settings.toml`; passwords are only ever in memory.

use gpui::{AnyWindowHandle, Context};
use muxy_app_core::ServerId;
use muxy_app_core::settings::ServerEntry;
use muxy_client::SshTarget;
use muxy_ui::tr;

use super::{AppModel, ConnectionState, Quitting, ServerStatus};

/// One remote server as the Remote section lists it.
#[derive(Clone, Debug)]
pub(crate) struct RemoteServer {
    pub(crate) entry: ServerEntry,
    /// It logs in with a password that hasn't been typed this session.
    pub(crate) needs_password: bool,
    pub(crate) status: ServerStatus,
    /// Why it isn't connected, when it failed.
    pub(crate) error: Option<String>,
    /// Muxy isn't installed there.
    pub(crate) missing: bool,
    /// Muxy there can't talk to this app.
    pub(crate) incompatible: bool,
    pub(crate) install: Option<Install>,
}

/// Installing Muxy on a remote server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Install {
    Running,
    Failed(String),
}

/// This app's version when it is a published release, which remote servers
/// can install. Local builds have none.
pub(crate) fn released_version() -> Option<&'static str> {
    let version = env!("CARGO_PKG_VERSION");
    (!cfg!(debug_assertions) && version != "2.0.0-beta-0").then_some(version)
}

/// Runs this release's installer there, which downloads it from GitHub,
/// checks it, and puts `muxy` and `muxy-server` in `~/.local/bin`.
pub(super) fn install_command(version: &str) -> String {
    crate::remote_install::Source::Release(version.into()).command()
}

/// The command that replaces Muxy on another computer with this release.
pub(crate) fn update_command(version: &str) -> String {
    format!(
        "curl -fsSL {}/v{version}/install-muxy.sh | sh -s -- --version {version} --replace",
        crate::updater::RELEASES
    )
}

/// Prints the version of Muxy installed there, found as the bridge finds it.
const VERSION_COMMAND: &str = "sh -c 'export PATH=\"$PATH:$HOME/.local/bin\"; exec muxy --version'";

/// Why Muxy on another computer can't talk to this app, and how to fix it.
/// `there` is the version installed there, when it could be read. When it is
/// this app's own, the server still running there is an older one.
pub(crate) fn incompatible_guidance(name: &str, there: Option<&str>) -> String {
    let here = env!("CARGO_PKG_VERSION");
    if there == Some(here) {
        return tr!(
            "Muxy on %@ is %@, but its running server is another version. Restart it there, which ends its terminals: pkill -x muxy-server",
            name,
            here
        )
        .to_string();
    }
    let there = there.map_or_else(
        || tr!("Muxy on %@ is another version", name),
        |version| tr!("Muxy on %@ is %@", name, version),
    );
    let protocol = muxy_protocol::CURRENT.0;
    let fix = released_version().map_or_else(
        || tr!("Install a matching build there by hand, from scripts/build-linux-dev.sh."),
        |version| tr!("Update Muxy on %@: %@", name, update_command(version)),
    );
    tr!(
        "%@; this app needs a version that speaks protocol V%lld, like %@. %@",
        &there,
        protocol,
        here,
        &fix
    )
    .to_string()
}

/// What the server form submits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DeviceForm {
    pub(crate) name: String,
    /// A host or `~/.ssh/config` alias, or a whole destination when the user
    /// and port fields are empty.
    pub(crate) host: String,
    pub(crate) user: String,
    pub(crate) port: String,
    pub(crate) identity_file: String,
    pub(crate) password_login: bool,
    /// Kept in memory until Muxy quits, so connecting doesn't ask; empty
    /// asks when connecting.
    pub(crate) password: String,
}

impl DeviceForm {
    /// The settings entry for this form: the device `id`, or a new one.
    fn entry(&self, id: Option<ServerId>) -> Result<ServerEntry, String> {
        let ssh = join_destination(&self.host, &self.user, &self.port)?;
        let name = match self.name.trim() {
            "" => ssh.clone(),
            name => name.to_owned(),
        };
        let mut entry = ServerEntry::new(name, ssh);
        if let Some(id) = id {
            entry.id = id;
        }
        entry.identity_file =
            Some(self.identity_file.trim().to_owned()).filter(|path| !path.is_empty());
        entry.password_login = self.password_login;
        entry.validate().map_err(|error| error.to_string())?;
        Ok(entry)
    }
}

/// An SSH destination from the editor's host, user, and port. With no user
/// and port, the host may itself be `user@host` or `ssh://user@host:port`.
pub(crate) fn join_destination(host: &str, user: &str, port: &str) -> Result<String, String> {
    let (host, user, port) = (host.trim(), user.trim(), port.trim());
    let invalid =
        || tr!("Enter an SSH host: a ~/.ssh/config alias, a host name, or an address").to_string();
    let destination = if user.is_empty() && port.is_empty() {
        host.to_owned()
    } else {
        if host.contains('@') || host.starts_with("ssh://") {
            return Err(
                tr!("Put the user and port either in the host or in their own fields").to_string(),
            );
        }
        if user.contains('@') || user.contains(':') {
            return Err(tr!("Enter a user name without @ or :").to_string());
        }
        let login = if user.is_empty() {
            host.to_owned()
        } else {
            format!("{user}@{host}")
        };
        if port.is_empty() {
            login
        } else {
            let port: u16 = port
                .parse()
                .ok()
                .filter(|port| *port > 0)
                .ok_or_else(|| tr!("Use a port from 1 to 65535").to_string())?;
            let login = if host.contains(':') && !host.starts_with('[') {
                login.replacen(host, &format!("[{host}]"), 1)
            } else {
                login
            };
            format!("ssh://{login}:{port}")
        }
    };
    SshTarget::new(&destination).map_err(|_| invalid())?;
    Ok(destination)
}

/// The editor's host, user, and port for a destination.
pub(crate) fn split_destination(destination: &str) -> (String, String, String) {
    let Some(rest) = destination.strip_prefix("ssh://") else {
        return match destination.split_once('@') {
            Some((user, host)) => (host.into(), user.into(), String::new()),
            None => (destination.into(), String::new(), String::new()),
        };
    };
    let (user, address) = rest.rsplit_once('@').unwrap_or(("", rest));
    let (host, port) = match address.rsplit_once(':') {
        Some((host, port)) if port.parse::<u16>().is_ok() && !host.ends_with(':') => {
            (host.trim_start_matches('[').trim_end_matches(']'), port)
        }
        _ => (address, ""),
    };
    (host.into(), user.into(), port.into())
}

/// A Test Connection in Settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ConnectionTest {
    Testing,
    UntrustedHost(String),
    Installing,
    NotInstalled(String),
    InstallFailed(String),
    /// It connected; this is the server's Muxy version.
    Succeeded(String),
    Failed(String),
}

impl AppModel {
    pub(crate) fn remote_servers(&self) -> Vec<RemoteServer> {
        self.settings
            .servers
            .iter()
            .map(|entry| {
                let runtime = self.servers.get(entry.id);
                RemoteServer {
                    entry: entry.clone(),
                    needs_password: entry.password_login && !self.servers.passwords.has(entry.id),
                    status: self.status_of(entry.id),
                    error: runtime.and_then(|runtime| runtime.error.clone()),
                    missing: runtime.is_some_and(|runtime| {
                        runtime.failure == Some(muxy_client::RemoteReason::NotInstalled)
                    }),
                    incompatible: runtime.is_some_and(|runtime| {
                        runtime.failure == Some(muxy_client::RemoteReason::Incompatible)
                    }),
                    install: runtime.and_then(|runtime| runtime.install.clone()),
                }
            })
            .collect()
    }

    /// The last Test Connection, if it tried `destination`.
    pub(crate) fn server_probe(&self, destination: &str) -> Option<&ConnectionTest> {
        self.server_probe
            .as_ref()
            .filter(|(tried, _)| tried == destination.trim())
            .map(|(_, probe)| probe)
    }

    /// Adds a remote server, or edits the one with `id`, and connects it. A
    /// typed password stays in memory, never in `settings.toml`. It is kept
    /// first: if it can't be, nothing is saved, so saving again can't add the
    /// server twice.
    pub(crate) fn save_remote_server(
        &mut self,
        id: Option<ServerId>,
        form: &DeviceForm,
        cx: &mut Context<Self>,
    ) -> Result<ServerId, String> {
        if self.quitting != Quitting::Idle {
            return Err(tr!("Muxy is quitting").to_string());
        }
        if id.is_some_and(|id| self.settings.server(id).is_none()) {
            return Err(tr!("This remote server is no longer listed").to_string());
        }
        let entry = form.entry(id)?;
        let server = entry.id;
        if form.password_login && !form.password.is_empty() {
            self.servers
                .passwords
                .set(server, form.password.clone())
                .map_err(|error| error.to_string())?;
        }
        let path = self.path.with_file_name("settings.toml");
        let result = if id.is_some() {
            self.settings.update_server(entry, &path)
        } else {
            self.settings.add_server(entry, &path)
        };
        if let Err(error) = result {
            if id.is_none() {
                self.servers.passwords.forget(server);
            }
            return Err(error.to_string());
        }
        if !form.password_login {
            self.servers.passwords.forget(server);
        }
        self.sync_servers(cx);
        Ok(server)
    }

    /// Matches the running servers to the list in settings, which also takes
    /// in hand edits made since launch. A new server, or one reached in a new
    /// way, gets a worker and connects. One no longer listed stops, and its
    /// projects stay hidden until it returns.
    fn sync_servers(&mut self, cx: &mut Context<Self>) {
        let listed = self.settings.servers.clone();
        let unlisted: Vec<_> = self
            .servers
            .ids()
            .into_iter()
            .filter(|server| !server.is_local() && self.settings.server(*server).is_none())
            .collect();
        for server in unlisted {
            self.disconnect(server, cx);
            self.servers.remove(server);
        }
        for entry in listed {
            let target = self.servers.target(&entry).ok();
            let Some(runtime) = self.servers.get(entry.id) else {
                self.servers.restart(&entry);
                self.connect_server(entry.id, cx);
                continue;
            };
            if runtime.target == target {
                continue;
            }
            self.disconnect(entry.id, cx);
            self.servers.restart(&entry);
            self.connect_server(entry.id, cx);
        }
        self.sync_preferences(cx);
        cx.notify();
    }

    pub(crate) fn connect_remote_server(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if server.is_local() {
            return;
        }
        if self.servers.get(server).is_none() {
            self.sync_servers(cx);
        }
        self.connect_server(server, cx);
    }

    /// Runs a Test Connection off the UI thread with what the form holds,
    /// before it is saved. It starts that computer's server if needed, as
    /// connecting would, and then hangs up. A typed password is answered
    /// only while the test runs.
    pub(crate) fn test_remote_server(
        &mut self,
        id: Option<ServerId>,
        form: &DeviceForm,
        cx: &mut Context<Self>,
    ) {
        self.run_server_probe(id, form, None, None, cx);
    }

    pub(crate) fn trust_test_server(
        &mut self,
        id: Option<ServerId>,
        form: &DeviceForm,
        cx: &mut Context<Self>,
    ) {
        let Ok(destination) = join_destination(&form.host, &form.user, &form.port) else {
            return;
        };
        let Some(ConnectionTest::UntrustedHost(prompt)) = self.server_probe(&destination) else {
            return;
        };
        self.run_server_probe(id, form, None, Some(prompt.clone()), cx);
    }

    pub(crate) fn clear_server_probe(&mut self) {
        self.server_probe = None;
        self.server_probe_generation = self.server_probe_generation.wrapping_add(1);
    }

    pub(crate) fn install_test_server(
        &mut self,
        id: Option<ServerId>,
        form: &DeviceForm,
        source: crate::remote_install::Source,
        cx: &mut Context<Self>,
    ) {
        self.run_server_probe(id, form, Some(source), None, cx);
    }

    fn run_server_probe(
        &mut self,
        id: Option<ServerId>,
        form: &DeviceForm,
        install: Option<crate::remote_install::Source>,
        trusted_host: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.clear_server_probe();
        let generation = self.server_probe_generation;
        let entry = match form.entry(id) {
            Ok(entry) => entry,
            Err(error) => {
                let tried = join_destination(&form.host, &form.user, &form.port)
                    .unwrap_or_else(|_| form.host.trim().to_owned());
                self.server_probe = Some((tried, ConnectionTest::Failed(error)));
                self.sync_preferences(cx);
                return;
            }
        };
        let destination = entry.ssh.clone();
        let target = match self.test_target(id, &entry, form, trusted_host) {
            Ok(target) => target,
            Err(error) => {
                self.server_probe = Some((destination, ConnectionTest::Failed(error)));
                self.sync_preferences(cx);
                return;
            }
        };
        let (target, once) = target;
        let answers = self.servers.passwords.shared();
        let pending = if install.is_some() {
            ConnectionTest::Installing
        } else {
            ConnectionTest::Testing
        };
        self.server_probe = Some((destination.clone(), pending.clone()));
        self.sync_preferences(cx);
        let probe = self.servers.probe();
        let run = self.servers.run();
        let result = cx.background_executor().spawn(async move {
            let installed = install.map_or(Ok(()), |source| {
                run(&target, &source.command(), source.archive()).map(drop)
            });
            let result = match installed {
                Err(error) => ConnectionTest::InstallFailed(error),
                Ok(()) => match probe(&target) {
                    Ok(version) => ConnectionTest::Succeeded(version),
                    Err(crate::boot::ProbeFailure::NotInstalled(error)) => {
                        ConnectionTest::NotInstalled(error)
                    }
                    Err(crate::boot::ProbeFailure::Other(error)) => ConnectionTest::Failed(error),
                },
            };
            if let Some(prompt) = crate::askpass::Passwords::finish_test(&answers, &once) {
                return ConnectionTest::UntrustedHost(prompt);
            }
            result
        });
        cx.spawn(async move |model, cx| {
            let result = result.await;
            let _ = model.update(cx, |model, cx| {
                if model.server_probe_generation == generation {
                    model.server_probe = Some((destination, result));
                    model.sync_preferences(cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// How a Test Connection reaches `entry`: with the typed password, else
    /// the one typed this session, and the one-off token to forget after.
    fn test_target(
        &mut self,
        id: Option<ServerId>,
        entry: &ServerEntry,
        form: &DeviceForm,
        trusted_host: Option<String>,
    ) -> Result<(SshTarget, String), String> {
        let mut target = SshTarget::new(&entry.ssh)
            .map_err(|error| error.to_string())?
            .with_host_key_prompt();
        if let Some(identity) = &entry.identity_file {
            target = target.with_identity(identity);
        }
        let (password, login) = if form.password_login {
            let password = if form.password.is_empty() {
                id.and_then(|id| self.servers.passwords.password(id))
            } else {
                Some(form.password.clone())
            }
            .ok_or_else(|| tr!("Enter the password to test logging in with it.").to_string())?;
            (
                Some(password),
                Some(target.login().map_err(|error| error.to_string())?),
            )
        } else {
            (None, None)
        };
        let (token, askpass) = self
            .servers
            .passwords
            .for_test(password, login, trusted_host)
            .map_err(|error| error.to_string())?;
        Ok((target.with_askpass(askpass), token))
    }

    pub(crate) fn confirm_forget_server(
        &mut self,
        server: ServerId,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = self.server_name(server).map(str::to_owned) else {
            return;
        };
        if self.close_prompt.is_some() || self.quitting != Quitting::Idle {
            return;
        }
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = crate::views::confirm::prompt_forget_server(window, &name, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.close_prompt = None;
                let result = match response {
                    Ok(true) => model.forget_remote_server(server, cx),
                    Ok(false) => Ok(()),
                    Err(error) => Err(error),
                };
                if let Err(error) = result {
                    model.fail(error, cx);
                }
                cx.notify();
            });
        }));
    }

    /// Removes a remote server from settings and its projects and layouts from
    /// this app. Nothing on that computer changes.
    pub(crate) fn forget_remote_server(
        &mut self,
        server: ServerId,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.quitting != Quitting::Idle {
            return Err(tr!("Muxy is quitting").to_string());
        }
        let path = self.path.with_file_name("settings.toml");
        self.settings
            .remove_server(server, &path)
            .map_err(|error| error.to_string())?;
        self.servers.passwords.forget(server);
        self.sync_servers(cx);
        self.edit_project(|state| state.forget_server(server), cx);
        Ok(())
    }

    pub(crate) fn confirm_install_server(
        &mut self,
        server: ServerId,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let (Some(name), Some(version)) = (
            self.server_name(server).map(str::to_owned),
            released_version(),
        ) else {
            return;
        };
        if self.close_prompt.is_some() || self.quitting != Quitting::Idle {
            return;
        }
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let source = crate::remote_install::Source::Release(version.into());
            let response =
                crate::views::confirm::prompt_install_server(window, &name, &source, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.close_prompt = None;
                match response {
                    Ok(true) => model.install_server(server, version, cx),
                    Ok(false) => {}
                    Err(error) => model.fail(error, cx),
                }
                cx.notify();
            });
        }));
    }

    /// Installs release `version` on a remote server over its SSH login,
    /// then connects. It runs off the UI thread; downloading takes a while.
    pub(crate) fn install_server(
        &mut self,
        server: ServerId,
        version: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        let Some(target) = runtime.target.clone() else {
            return;
        };
        if runtime.install == Some(Install::Running) {
            return;
        }
        runtime.install = Some(Install::Running);
        self.sync_preferences(cx);
        let run = self.servers.run();
        let command = install_command(version);
        let result = cx
            .background_executor()
            .spawn(async move { run(&target, &command, None).map(drop) });
        cx.spawn(async move |model, cx| {
            let result = result.await;
            let _ = model.update(cx, |model, cx| {
                let Some(runtime) = model.servers.get_mut(server) else {
                    return;
                };
                match result {
                    Ok(()) => {
                        runtime.install = None;
                        runtime.failure = None;
                        runtime.error = None;
                        model.connect_server(server, cx);
                    }
                    Err(error) => runtime.install = Some(Install::Failed(error)),
                }
                model.sync_preferences(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Explains a version mismatch on the server's own status, then reads
    /// the version installed there to name it. The app never updates it.
    pub(super) fn explain_incompatible(&mut self, server: ServerId, cx: &mut Context<Self>) {
        let name = self.server_label(server);
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        runtime.error = Some(incompatible_guidance(&name, None));
        let Some(target) = runtime.target.clone() else {
            return;
        };
        let generation = runtime.generation;
        let run = self.servers.run();
        let version = cx
            .background_executor()
            .spawn(async move { run(&target, VERSION_COMMAND, None) });
        cx.spawn(async move |model, cx| {
            let Ok(printed) = version.await else {
                return;
            };
            let version = printed.trim().trim_start_matches("muxy").trim().to_owned();
            let _ = model.update(cx, |model, cx| {
                let Some(runtime) = model.servers.get_mut(server) else {
                    return;
                };
                if runtime.generation == generation
                    && runtime.failure == Some(muxy_client::RemoteReason::Incompatible)
                    && !version.is_empty()
                {
                    runtime.error = Some(incompatible_guidance(&name, Some(&version)));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Copies the command that updates Muxy on an incompatible server.
    pub(crate) fn copy_update_command(&mut self, cx: &mut Context<Self>) {
        if let Some(version) = released_version() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(update_command(version)));
            self.show_notice(tr!("Copied the update command").to_string(), cx);
        }
    }

    /// Connects, or connects again: a fresh SSH login and server check.
    pub(crate) fn reconnect_remote_server(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if self.connection(server) != ConnectionState::Disconnected {
            self.disconnect(server, cx);
        }
        self.connect_remote_server(server, cx);
    }

    /// Keeps a typed password until Muxy quits, then connects with it.
    pub(crate) fn remember_password(
        &mut self,
        server: ServerId,
        password: String,
        cx: &mut Context<Self>,
    ) {
        if let Some((pending, asked)) = &mut self.pending_remote_picker
            && *pending == server
        {
            *asked = std::time::Instant::now();
        }
        match self.servers.passwords.set(server, password) {
            Ok(()) => self.connect_remote_server(server, cx),
            Err(error) => {
                if let Some(runtime) = self.servers.get_mut(server) {
                    runtime.error =
                        Some(tr!("Could not take the password: %@", error.to_string()).to_string());
                }
                self.sync_preferences(cx);
                cx.notify();
            }
        }
    }

    /// Whether a password prompt or form may open now.
    pub(crate) fn accepts_prompts(&self) -> bool {
        self.quitting == Quitting::Idle && self.close_prompt.is_none()
    }

    /// The server refused the last login, so its password was wrong.
    pub(crate) fn server_failed_login(&self, server: ServerId) -> bool {
        self.servers.get(server).is_some_and(|runtime| {
            runtime.failure == Some(muxy_client::RemoteReason::AuthenticationFailed)
        })
    }

    /// What the status bar and Settings show for a server.
    pub(crate) fn status_of(&self, server: ServerId) -> ServerStatus {
        if server.is_local() {
            if self.updates.replacing() {
                return ServerStatus::Restarting;
            }
            if self.server_preferences.control_busy {
                return ServerStatus::Stopping;
            }
        } else if self
            .servers
            .get(server)
            .is_some_and(|runtime| runtime.stopping)
        {
            return ServerStatus::Stopping;
        }
        match self.connection(server) {
            ConnectionState::Ready => ServerStatus::Connected,
            ConnectionState::Connecting => ServerStatus::Connecting,
            ConnectionState::Disconnected => ServerStatus::Disconnected,
        }
    }
}
