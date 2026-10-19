//! Browser profiles: each keeps its own cookies, site data, and history.

use gpui::Context;
use muxy_ui::browser::polybrowser::{
    self, ExternalProfile, ImportOptions, Profile, ProfileId, ProfileManager,
};
use muxy_ui::tr;

use crate::model::AppModel;
use crate::views::settings::BrowserChange;
use crate::views::settings::browser::Profile as SettingsProfile;

/// The profile new tabs use when none is chosen.
const DEFAULT: &str = "Default";

impl AppModel {
    fn browser_profiles(&mut self) -> Result<&ProfileManager, String> {
        if self.browsers.profiles.is_none() {
            let manager = ProfileManager::open_at(self.path.with_file_name("browser"))
                .map_err(|error| error.to_string())?;
            self.browsers.profiles = Some(manager);
        }
        self.browsers
            .profiles
            .as_ref()
            .ok_or_else(|| "browser profiles are unavailable".into())
    }

    /// The profile named or identified by `wanted`, or the default one.
    pub(crate) fn browser_profile(&mut self, wanted: Option<&str>) -> Result<Profile, String> {
        let Some(wanted) = wanted.filter(|wanted| !wanted.is_empty()) else {
            return self.default_browser_profile();
        };
        let profiles = self.browser_profiles()?;
        let profile = if let Some(profile) = wanted
            .parse::<ProfileId>()
            .ok()
            .and_then(|id| profiles.get(id).ok())
        {
            profile
        } else {
            profiles
                .find(wanted)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| format!("no browser profile named '{wanted}'"))?
        };
        if self.browsers.retiring.contains(&profile.id().to_string()) {
            return Err("browser profile is being deleted".into());
        }
        Ok(profile)
    }

    pub(crate) fn pane_profile(&mut self, saved: Option<&str>) -> Result<Profile, String> {
        if saved.is_some_and(|id| self.browsers.retiring.contains(id)) {
            return Err("browser profile is being deleted".into());
        }
        self.browser_profile(saved)
            .or_else(|_| self.default_browser_profile())
    }

    fn default_browser_profile(&mut self) -> Result<Profile, String> {
        let chosen = self.settings.browser.default_profile.clone();
        if self.browsers.retiring.contains(&chosen) {
            return Err("browser profile is being deleted".into());
        }
        let profiles = self.browser_profiles()?;
        if let Some(profile) = chosen
            .parse::<ProfileId>()
            .ok()
            .and_then(|id| profiles.get(id).ok())
        {
            return Ok(profile);
        }
        profiles
            .get_or_create(DEFAULT)
            .map_err(|error| error.to_string())
    }

    /// Lists profiles and other browsers again, for Settings.
    pub(crate) fn refresh_browser_profiles(&mut self) {
        let _ = self.default_browser_profile();
        self.browsers.profile_list = self
            .browser_profiles()
            .and_then(|profiles| profiles.list().map_err(|error| error.to_string()))
            .map(|profiles| {
                profiles
                    .into_iter()
                    .map(|profile| (profile.id().to_string(), profile.name().to_owned()))
                    .collect()
            })
            .unwrap_or_default();
    }

    /// Profiles as Settings shows them: the default first, then by name.
    pub(crate) fn settings_browser_profiles(&self) -> Vec<SettingsProfile> {
        let chosen = &self.settings.browser.default_profile;
        let chosen_listed = self
            .browsers
            .profile_list
            .iter()
            .any(|(id, _)| id == chosen);
        let mut profiles: Vec<_> = self
            .browsers
            .profile_list
            .iter()
            .map(|(id, name)| SettingsProfile {
                default: if chosen_listed {
                    id == chosen
                } else {
                    name == DEFAULT
                },
                id: id.clone(),
                name: name.clone(),
            })
            .collect();
        profiles.sort_by_key(|profile| (!profile.default, profile.name.to_lowercase()));
        profiles
    }

    /// Carries out a profile change from Settings. Changes saved in
    /// `settings.toml` are left to the caller, which gets `false` for them.
    pub(crate) fn apply_browser_profile_change(
        &mut self,
        change: &BrowserChange,
        cx: &mut Context<Self>,
    ) -> Result<bool, String> {
        if let BrowserChange::DefaultProfile(id) = change
            && self.browsers.retiring.contains(id)
        {
            return Err("browser profile is being deleted".into());
        }
        let result = match change {
            BrowserChange::AddProfile(name) => self.add_browser_profile(name),
            BrowserChange::RenameProfile(id, name) => self.rename_browser_profile(id, name),
            BrowserChange::ClearProfile(id) => self.clear_browser_profile(id, cx),
            BrowserChange::DeleteProfile(id) => self.delete_browser_profile(id, cx),
            BrowserChange::Import { source, cookies } => {
                let default = self
                    .default_browser_profile()
                    .map(|profile| profile.id().to_string());
                match (default, self.browsers.import_sources.get(*source).cloned()) {
                    (Ok(default), Some(source)) => {
                        self.import_into_browser_profile(&default, source, *cookies, cx)
                    }
                    (Err(error), _) => Err(error),
                    (_, None) => Err(tr!("That browser is no longer available").into()),
                }
            }
            BrowserChange::DefaultProfile(id) => {
                parse_id(id)?;
                return Ok(false);
            }
            BrowserChange::Enabled(_)
            | BrowserChange::TerminalLinks(_)
            | BrowserChange::SearchEngine(_) => return Ok(false),
        };
        self.refresh_browser_profiles();
        result.map(|()| true)
    }

    pub(crate) fn add_browser_profile(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        let name = if name.is_empty() { "Profile" } else { name };
        self.browser_profiles()?
            .create(name)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn rename_browser_profile(&mut self, id: &str, name: &str) -> Result<(), String> {
        let id = parse_id(id)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(tr!("A profile needs a name").into());
        }
        self.browser_profiles()?
            .rename(id, name)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    /// Deletes a profile with its cookies, site data, and history. Its tabs
    /// browse with the default profile from then on, and keep their profile
    /// if it can't be deleted.
    pub(crate) fn delete_browser_profile(
        &mut self,
        id: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let default = self.default_browser_profile()?.id().to_string();
        if id == default {
            return Err(tr!("The default profile can't be deleted").into());
        }
        let parsed = parse_id(id)?;
        #[cfg(target_os = "linux")]
        {
            let profile = self.browser_profile(Some(id))?;
            self.browsers.retiring.insert(id.to_owned());
            let (sender, closed) = async_channel::bounded(1);
            muxy_ui::browser::close_profile(&profile, move |result| {
                let _ = sender.try_send(result);
            });
            cx.spawn(async move |model, cx| {
                let result = closed.recv().await;
                let _ = model.update(cx, |model, cx| {
                    let result = result
                        .map_err(|error| error.to_string())
                        .and_then(|result| result.map_err(|error| error.to_string()));
                    let result = result.and_then(|()| model.finish_profile_delete(parsed, cx));
                    model.browsers.retiring.remove(&parsed.to_string());
                    if let Err(error) = result {
                        model.fail(error, cx);
                    }
                    model.refresh_browser_profiles();
                    model.sync_preferences(cx);
                    cx.notify();
                });
            })
            .detach();
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        self.finish_profile_delete(parsed, cx)
    }

    fn finish_profile_delete(
        &mut self,
        parsed: ProfileId,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let id = parsed.to_string();
        let default = self.default_browser_profile()?.id().to_string();
        if id == default {
            return Err(tr!("The default profile can't be deleted").into());
        }
        let users: Vec<_> = self
            .browser_panes()
            .into_iter()
            .filter(|(.., browser)| browser.profile.as_deref() == Some(&id))
            .map(|(_, _, pane, _)| pane)
            .collect();
        for pane in &users {
            self.browsers.views.remove(pane);
        }
        self.browser_profiles()?
            .delete(parsed)
            .map_err(|error| error.to_string())?;
        for pane in &users {
            let _ = self.state.set_browser_profile(*pane, Some(default.clone()));
        }
        if !users.is_empty() {
            self.save(cx);
        }
        cx.notify();
        Ok(())
    }

    /// Signs the profile out everywhere: its cookies, site data, and history
    /// go.
    pub(crate) fn clear_browser_profile(
        &mut self,
        id: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let profile = self.browser_profile(Some(id))?;
        profile
            .history()
            .and_then(|history| history.clear())
            .map_err(|error| error.to_string())?;
        let name = profile.name().to_owned();
        let (sender, cleared) = async_channel::bounded(1);
        muxy_ui::browser::clear_site_data(muxy_ui::browser::ENGINE, &profile, move |result| {
            let _ = sender.try_send(result);
        })
        .map_err(|error| error.to_string())?;
        cx.spawn(async move |model, cx| {
            let result = cleared.recv().await;
            let _ = model.update(cx, |model, cx| match result {
                Ok(Ok(())) => {
                    let notice = tr!("Cleared browsing data for “%@”", &name).to_string();
                    model.show_notice(notice, cx);
                }
                Ok(Err(error)) => model.fail(error.to_string(), cx),
                Err(_) => {}
            });
        })
        .detach();
        Ok(())
    }

    /// Browsers on this computer whose history, bookmarks, or cookies can be
    /// imported.
    pub(crate) fn browser_import_sources() -> Vec<ExternalProfile> {
        polybrowser::import::detect()
    }

    /// Brings history and bookmarks, and with `cookies` the sign-ins, from
    /// another browser into a profile.
    pub(crate) fn import_into_browser_profile(
        &mut self,
        id: &str,
        source: ExternalProfile,
        cookies: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let profile = self.browser_profile(Some(id))?;
        let label = format!("{} — {}", source.browser(), source.name());
        let reader = profile.clone();
        let read = cx.background_executor().spawn(async move {
            if cookies {
                source
                    .read_cookies()
                    .map(Imported::Cookies)
                    .map_err(|error| error.to_string())
            } else {
                reader
                    .import_from(&source, &ImportOptions::default())
                    .map(Imported::History)
                    .map_err(|error| error.to_string())
            }
        });
        cx.spawn(async move |model, cx| {
            let outcome = read.await;
            let _ = model.update(cx, |model, cx| match outcome {
                Ok(Imported::History(report)) => model.show_notice(
                    tr!(
                        "Imported %lld visits and %lld bookmarks from %@",
                        report.visits_added,
                        report.bookmarks_added,
                        &label
                    )
                    .to_string(),
                    cx,
                ),
                Ok(Imported::Cookies(cookies)) => {
                    model.write_imported_cookies(&profile, cookies, label, cx);
                }
                Err(error) => model.fail(error, cx),
            });
        })
        .detach();
        Ok(())
    }

    fn write_imported_cookies(
        &mut self,
        profile: &Profile,
        cookies: Vec<polybrowser::Cookie>,
        label: String,
        cx: &mut Context<Self>,
    ) {
        let (sender, written) = async_channel::bounded(1);
        if let Err(error) = muxy_ui::browser::import_cookies(
            muxy_ui::browser::ENGINE,
            profile,
            cookies,
            move |result| {
                let _ = sender.try_send(result);
            },
        ) {
            self.fail(error.to_string(), cx);
            return;
        }
        let name = profile.name().to_owned();
        cx.spawn(async move |model, cx| {
            let result = written.recv().await;
            let _ = model.update(cx, |model, cx| match result {
                Ok(Ok(count)) => model.show_notice(
                    tr!(
                        "Imported %lld cookies from %@ into “%@”",
                        count,
                        &label,
                        &name
                    )
                    .to_string(),
                    cx,
                ),
                Ok(Err(error)) => model.fail(error.to_string(), cx),
                Err(_) => {}
            });
        })
        .detach();
    }
}

/// What was read from another browser.
enum Imported {
    History(polybrowser::ImportReport),
    Cookies(Vec<polybrowser::Cookie>),
}

fn parse_id(id: &str) -> Result<ProfileId, String> {
    id.parse()
        .map_err(|_| format!("no browser profile with ID {id}"))
}
