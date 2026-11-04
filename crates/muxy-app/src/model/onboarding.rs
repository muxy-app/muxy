//! The welcome tour: where it stands, the extensions it sets up, and the phone
//! it pairs. Other overlays may open over it, and it comes back when they
//! close. Extensions it started setting up finish even if it closes first.

use std::path::Path;
use std::sync::Arc;

use gpui::{AppContext as _, Context, Entity};
use muxy_app_core::qr::QrCode;
use muxy_app_core::{ProjectId, ServerId};
use muxy_protocol::{DeviceId, ListenerStatus, RemoteAccessSettings};
use muxy_ui::tr;

use super::AppModel;
use super::extensions::ActiveInstall;
use crate::boot::Work;
use crate::extensions::io;
use crate::extensions::marketplace::{self, ExtensionDetails};
use crate::views::onboarding::art::ArtView;
use crate::views::overlays::Overlay;

pub(crate) const APP_STORE_URL: &str = "https://apps.apple.com/app/muxy/id6762464046";
pub(crate) const PLAY_STORE_URL: &str =
    "https://play.google.com/store/apps/details?id=com.muxy.app";

const OFFERED: [&str; 2] = ["files", "git"];

type Fetch = fn(&str) -> Result<ExtensionDetails, String>;
type Download = fn(&ExtensionDetails, &Path) -> Result<tempfile::TempDir, String>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Step {
    Welcome,
    Server,
    Projects,
    Extensions,
    Agents,
    Remote,
    Mobile,
    Shortcuts,
}

impl Step {
    pub(crate) const ALL: [Self; 8] = [
        Self::Welcome,
        Self::Server,
        Self::Projects,
        Self::Extensions,
        Self::Agents,
        Self::Remote,
        Self::Mobile,
        Self::Shortcuts,
    ];

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    pub(crate) fn next(self) -> Option<Self> {
        Self::ALL.get(self.index() + 1).copied()
    }

    pub(crate) fn previous(self) -> Option<Self> {
        Self::ALL.get(self.index().checked_sub(1)?).copied()
    }
}

/// How far an offered extension has come. An extension that is already
/// installed needs only enabling, and one already enabled needs nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OfferState {
    Loading,
    Unavailable(String),
    Ready,
    Queued,
    Installing,
    Enabling,
    Done,
    Failed(String),
}

impl OfferState {
    /// Whether a click can set this offer up. One done earlier counts, as it
    /// may have been disabled since.
    fn can_start(&self) -> bool {
        matches!(self, Self::Ready | Self::Done | Self::Failed(_))
    }
}

pub(crate) struct Offer {
    pub(crate) name: &'static str,
    pub(crate) selected: bool,
    pub(crate) details: Option<ExtensionDetails>,
    pub(crate) state: OfferState,
}

/// Pairing a phone from the tour's mobile step.
#[derive(Default)]
pub(crate) struct PairAttempt {
    /// A code was asked for and isn't showing yet.
    pub(crate) wanted: bool,
    /// The tour turned device access on and nothing has paired yet, so leaving
    /// turns it back off.
    turned_on: bool,
    /// The devices paired before this attempt.
    known: Option<Vec<DeviceId>>,
    pub(crate) paired: Option<String>,
}

pub(crate) struct Tour {
    pub(crate) step: Step,
    pub(crate) art: Entity<ArtView>,
    pub(crate) pair: PairAttempt,
    /// The projects before the tour's button opened the picker, until a new
    /// one appears.
    known_projects: Option<Vec<ProjectId>>,
    /// The servers before the tour's button opened their form, until a new one
    /// appears.
    known_servers: Option<Vec<ServerId>>,
    /// The App Store's code, then Google Play's.
    pub(crate) store_codes: [Option<Arc<QrCode>>; 2],
    pub(crate) added_project: Option<String>,
    pub(crate) added_server: Option<String>,
}

pub(crate) struct Onboarding {
    pub(crate) tour: Option<Tour>,
    pub(crate) offers: Vec<Offer>,
    pub(super) fetch: Fetch,
    pub(super) download: Download,
}

impl Default for Onboarding {
    fn default() -> Self {
        Self {
            tour: None,
            offers: Vec::new(),
            fetch: marketplace::detail,
            download: marketplace::download,
        }
    }
}

impl Onboarding {
    /// An offered extension is on its way, so it can't be installed again yet.
    pub(crate) fn installing(&self) -> bool {
        self.offers.iter().any(|offer| {
            matches!(
                offer.state,
                OfferState::Queued | OfferState::Installing | OfferState::Enabling
            )
        })
    }
}

impl AppModel {
    /// Opens the tour where it stands, or from the start.
    pub(crate) fn open_onboarding(&mut self, cx: &mut Context<Self>) {
        if self.onboarding.tour.is_none() {
            if !self.onboarding.installing() {
                self.onboarding.offers.clear();
            }
            let model = cx.weak_entity();
            let art = cx.new(|cx| ArtView::new(model, cx));
            self.onboarding.tour = Some(Tour {
                step: Step::Welcome,
                art,
                pair: PairAttempt::default(),
                known_projects: None,
                known_servers: None,
                store_codes: [APP_STORE_URL, PLAY_STORE_URL]
                    .map(|url| QrCode::encode(url).map(Arc::new)),
                added_project: None,
                added_server: None,
            });
        }
        self.open_overlay(Overlay::Onboarding, cx);
        self.focus_owner.request(&self.overlay_focus);
        cx.notify();
    }

    fn project_ids(&self) -> Vec<ProjectId> {
        self.state
            .projects()
            .iter()
            .map(|project| project.id)
            .collect()
    }

    fn server_ids(&self) -> Vec<ServerId> {
        self.settings.servers.iter().map(|entry| entry.id).collect()
    }

    pub(crate) fn step_onboarding(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(tour) = &self.onboarding.tour else {
            return;
        };
        let target = if forward {
            tour.step.next()
        } else {
            tour.step.previous()
        };
        match target {
            Some(step) => self.show_onboarding_step(step, cx),
            None if forward => self.finish_onboarding(cx),
            None => {}
        }
    }

    pub(crate) fn show_onboarding_step(&mut self, step: Step, cx: &mut Context<Self>) {
        let Some(tour) = &mut self.onboarding.tour else {
            return;
        };
        if tour.step == step {
            return;
        }
        let left_pairing = (tour.step == Step::Mobile).then(|| std::mem::take(&mut tour.pair));
        tour.step = step;
        let art = tour.art.clone();
        if let Some(attempt) = left_pairing {
            self.leave_onboarding_pairing(attempt.turned_on, cx);
        }
        match step {
            Step::Extensions => self.load_offers(cx),
            Step::Mobile => self.read_remote_access(cx),
            _ => {}
        }
        art.update(cx, |art, cx| art.show(step, cx));
        self.focus_owner.request(&self.overlay_focus);
        cx.notify();
    }

    /// Closes the tour for good, whether finished or skipped.
    pub(crate) fn finish_onboarding(&mut self, cx: &mut Context<Self>) {
        let tour = self.onboarding.tour.take();
        if matches!(self.overlay, Some(Overlay::Onboarding)) {
            self.close_overlay(cx);
        }
        if let Some(tour) = tour {
            self.leave_onboarding_pairing(tour.pair.turned_on, cx);
        }
        if !self.appearance.onboarding_completed {
            self.appearance.onboarding_completed = true;
            self.save_appearance(cx);
        }
        cx.notify();
    }

    /// Adds a project from the tour, to confirm it once it appears.
    pub(crate) fn add_project_from_onboarding(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let known = self.project_ids();
        if let Some(tour) = &mut self.onboarding.tour {
            tour.known_projects = Some(known);
        }
        self.open_project_picker(window, cx);
    }

    /// Adds a server from the tour, to confirm it once it appears.
    pub(crate) fn add_server_from_onboarding(&mut self, cx: &mut Context<Self>) {
        let known = self.server_ids();
        if let Some(tour) = &mut self.onboarding.tour {
            tour.known_servers = Some(known);
        }
        self.open_server_form(None, cx);
    }

    /// Runs after every change. Confirms what the tour's buttons added, and
    /// brings the tour back when whatever opened over it has closed.
    pub(crate) fn sync_onboarding(&mut self, cx: &mut Context<Self>) {
        self.note_onboarding_additions();
        if self.overlay.is_some() || self.onboarding.tour.is_none() {
            return;
        }
        self.overlay = Some(Overlay::Onboarding);
        self.focus_owner.request(&self.overlay_focus);
        if self.onboarding_pairing_shown() {
            self.read_remote_access(cx);
        }
        cx.notify();
    }

    fn note_onboarding_additions(&mut self) {
        let Some(tour) = &mut self.onboarding.tour else {
            return;
        };
        if let Some(known) = &tour.known_projects
            && let Some(project) = self
                .state
                .projects()
                .iter()
                .filter(|project| !project.home)
                .rfind(|project| !known.contains(&project.id))
        {
            tour.added_project = Some(project.name.clone());
            tour.known_projects = None;
        }
        if let Some(known) = &tour.known_servers
            && let Some(entry) = self
                .settings
                .servers
                .iter()
                .rfind(|entry| !known.contains(&entry.id))
        {
            tour.added_server = Some(entry.name.clone());
            tour.known_servers = None;
        }
    }

    /// The tour's mobile step is on screen, so its pairing code may be too.
    pub(super) fn onboarding_pairing_shown(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Onboarding))
            && self
                .onboarding
                .tour
                .as_ref()
                .is_some_and(|tour| tour.step == Step::Mobile)
    }

    /// Something opened over the tour or closed it, so a code it showed is
    /// withdrawn.
    pub(crate) fn onboarding_hidden(&mut self, cx: &mut Context<Self>) {
        if !self.onboarding_pairing_shown() {
            self.withdraw_pairing(cx);
        }
    }

    /// The keyboard stays in the tour while it shows. A shortcut may have moved
    /// it to the pane behind, and the button holding it may be gone or disabled.
    pub(crate) fn hold_onboarding_focus(&self, window: &mut gpui::Window, cx: &gpui::App) {
        if matches!(self.overlay, Some(Overlay::Onboarding))
            && !self.dialogs.is_open()
            && !self.overlay_focus.contains_focused(window, cx)
        {
            self.overlay_focus.focus(window);
        }
    }

    fn load_offers(&mut self, cx: &mut Context<Self>) {
        if self.onboarding.offers.is_empty() {
            self.onboarding.offers = OFFERED
                .into_iter()
                .map(|name| Offer {
                    name,
                    selected: true,
                    details: None,
                    state: OfferState::Unavailable(String::new()),
                })
                .collect();
        }
        let fetch = self.onboarding.fetch;
        let registry = &self.extensions.registry;
        for offer in &mut self.onboarding.offers {
            if !matches!(offer.state, OfferState::Unavailable(_)) {
                continue;
            }
            if let Some(extension) = registry.extensions.get(offer.name) {
                offer.details = Some(ExtensionDetails::from(extension));
                offer.state = OfferState::Ready;
                continue;
            }
            offer.state = OfferState::Loading;
            let name = offer.name;
            cx.spawn(async move |model, cx| {
                let result = io::network(move || fetch(name)).await;
                let _ = model.update(cx, |model, cx| model.receive_offer(name, result, cx));
            })
            .detach();
        }
        cx.notify();
    }

    fn receive_offer(
        &mut self,
        name: &str,
        result: Result<ExtensionDetails, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(offer) = self
            .offer_mut(name)
            .filter(|offer| offer.state == OfferState::Loading)
        else {
            return;
        };
        match result {
            Ok(details) if details.name == name => {
                offer.details = Some(details);
                offer.state = OfferState::Ready;
            }
            Ok(_) => {
                offer.state = OfferState::Unavailable(
                    tr!("The marketplace answered with a different extension.").to_string(),
                );
            }
            Err(error) => offer.state = OfferState::Unavailable(error),
        }
        cx.notify();
    }

    fn offer_mut(&mut self, name: &str) -> Option<&mut Offer> {
        self.onboarding
            .offers
            .iter_mut()
            .find(|offer| offer.name == name)
    }

    pub(crate) fn retry_offers(&mut self, cx: &mut Context<Self>) {
        self.load_offers(cx);
    }

    pub(crate) fn toggle_offer(&mut self, name: &str, cx: &mut Context<Self>) {
        if let Some(offer) = self.offer_mut(name) {
            offer.selected = !offer.selected;
            cx.notify();
        }
    }

    /// Ticked offers that can be set up now.
    pub(crate) fn offers_to_install(&self) -> usize {
        if !self.extensions_loaded() || self.onboarding.installing() {
            return 0;
        }
        self.onboarding
            .offers
            .iter()
            .filter(|offer| {
                offer.selected
                    && offer.state.can_start()
                    && self.extensions.registry.enabled(offer.name).is_none()
            })
            .count()
    }

    /// Installs and enables the ticked offers, one at a time.
    pub(crate) fn install_offers(&mut self, cx: &mut Context<Self>) {
        if self.offers_to_install() == 0 {
            return;
        }
        let registry = &self.extensions.registry;
        for offer in &mut self.onboarding.offers {
            if offer.selected && offer.state.can_start() && registry.enabled(offer.name).is_none() {
                offer.state = OfferState::Queued;
            }
        }
        self.advance_offers(cx);
        self.focus_owner.request(&self.overlay_focus);
        cx.notify();
    }

    /// Starts the next queued offer, unless something else is still running.
    /// Another marketplace job holds it up until that job finishes.
    fn advance_offers(&mut self, cx: &mut Context<Self>) {
        loop {
            if self.extensions.installs.active.is_some()
                || self.onboarding.offers.iter().any(|offer| {
                    matches!(offer.state, OfferState::Installing | OfferState::Enabling)
                })
            {
                return;
            }
            let Some(offer) = self
                .onboarding
                .offers
                .iter()
                .find(|offer| offer.state == OfferState::Queued)
            else {
                return;
            };
            let name = offer.name;
            let details = offer.details.clone();
            let registry = &self.extensions.registry;
            let enabled = registry.enabled(name).is_some();
            let installed = registry.extensions.contains_key(name);
            if enabled {
                self.settle_offer(name, Ok(()), cx);
            } else if installed {
                self.enable_offer(name, cx);
                return;
            } else if let Some(details) = details {
                if let Some(offer) = self.offer_mut(name) {
                    offer.state = OfferState::Installing;
                }
                let download = self.onboarding.download;
                if self.install_marketplace_extension_with(details, download, cx) {
                    return;
                }
                let error = tr!("The extension could not be installed.").to_string();
                self.settle_offer(name, Err(error), cx);
            } else {
                let error = tr!("The extension could not be found.").to_string();
                self.settle_offer(name, Err(error), cx);
            }
        }
    }

    /// A marketplace job finished: an offer's goes on to be enabled, and any
    /// job makes way for the next offer.
    pub(super) fn onboarding_install_finished(&mut self, cx: &mut Context<Self>) {
        let installing = self
            .onboarding
            .offers
            .iter()
            .find(|offer| offer.state == OfferState::Installing)
            .map(|offer| offer.name);
        let Some(name) = installing else {
            if self.onboarding.installing() {
                self.surface_finished_job(cx);
            }
            self.advance_offers(cx);
            return;
        };
        let finished = self.extensions.installs.finished.as_ref();
        let ours = finished.is_some_and(|finished| {
            matches!(&finished.job, ActiveInstall::Install { name: job } | ActiveInstall::Update { name: job } if job == name)
        });
        if !ours {
            return;
        }
        let error = self
            .extensions
            .installs
            .finished
            .take()
            .and_then(|finished| finished.error);
        match error {
            Some(error) => {
                self.settle_offer(name, Err(error), cx);
                self.advance_offers(cx);
            }
            None => self.enable_offer(name, cx),
        }
    }

    /// Another job's result would be wiped when the next offer starts, before
    /// any view could show it, so it is told where the user is.
    fn surface_finished_job(&mut self, cx: &mut Context<Self>) {
        let Some(finished) = self.extensions.installs.finished.take() else {
            return;
        };
        if let Some(error) = finished.error {
            self.fail(error, cx);
        } else if let Some(status) = finished.status {
            self.show_notice(status, cx);
        }
    }

    fn enable_offer(&mut self, name: &'static str, cx: &mut Context<Self>) {
        if let Some(offer) = self.offer_mut(name) {
            offer.state = OfferState::Enabling;
        }
        let task = self.set_extension_enabled(name, true, cx);
        cx.spawn(async move |model, cx| {
            let result = task.await;
            let _ = model.update(cx, |model, cx| {
                model.settle_offer(name, result, cx);
                model.advance_offers(cx);
            });
        })
        .detach();
    }

    /// An offer is done. With the tour already closed, the result is told
    /// where the user is.
    fn settle_offer(&mut self, name: &str, result: Result<(), String>, cx: &mut Context<Self>) {
        let failure = result.as_ref().err().cloned();
        if let Some(offer) = self.offer_mut(name) {
            offer.state = match result {
                Ok(()) => OfferState::Done,
                Err(error) => OfferState::Failed(error),
            };
        }
        if self.onboarding.tour.is_none() {
            if let Some(error) = failure {
                self.fail(tr!("Could not set up %@: %@", name, error).to_string(), cx);
            } else if !self.onboarding.installing()
                && !self
                    .onboarding
                    .offers
                    .iter()
                    .any(|offer| matches!(offer.state, OfferState::Failed(_)))
            {
                self.show_notice(tr!("Your extensions are ready.").to_string(), cx);
            }
        }
        cx.notify();
    }

    /// Asks for a pairing code, turning device access on first when it is off.
    pub(crate) fn pair_phone_in_onboarding(&mut self, cx: &mut Context<Self>) {
        if !self.onboarding_pairing_shown() {
            return;
        }
        if let Some(tour) = &mut self.onboarding.tour {
            tour.pair.wanted = true;
            tour.pair.paired = None;
            tour.pair.known = None;
        }
        self.mobile.error = None;
        if self.mobile.state.is_none() {
            self.read_remote_access(cx);
        }
        self.continue_onboarding_pairing(cx);
        cx.notify();
    }

    pub(crate) fn cancel_onboarding_pairing(&mut self, cx: &mut Context<Self>) {
        if let Some(tour) = &mut self.onboarding.tour {
            tour.pair.wanted = false;
        }
        self.cancel_pairing(cx);
        self.focus_owner.request(&self.overlay_focus);
    }

    /// Quitting ends the tour without leaving its mobile step, so access it
    /// turned on goes back off now. Requests run in order, so this also follows
    /// a request to turn it on that hasn't been answered yet.
    pub(super) fn restore_device_access_before_quit(&mut self, cx: &mut Context<Self>) {
        let turned_on = self
            .onboarding
            .tour
            .as_ref()
            .is_some_and(|tour| tour.pair.turned_on);
        if !(turned_on || self.mobile.restore_off) || !self.mobile_available() {
            return;
        }
        let Some(settings) = self.mobile.state.as_ref().map(|state| state.settings) else {
            return;
        };
        self.send(
            ServerId::local(),
            Work::WriteRemoteAccess(RemoteAccessSettings {
                enabled: false,
                ..settings
            }),
            cx,
        );
    }

    /// Takes pairing one step further after every answer from the server, and
    /// notices the phone that used the code.
    pub(super) fn continue_onboarding_pairing(&mut self, cx: &mut Context<Self>) {
        if !self.onboarding_pairing_shown() {
            return;
        }
        let Some(tour) = &mut self.onboarding.tour else {
            return;
        };
        let Some(state) = &self.mobile.state else {
            return;
        };
        if tour.pair.wanted && tour.pair.known.is_none() {
            tour.pair.known = Some(state.devices.iter().map(|device| device.id).collect());
        }
        if let Some(known) = &tour.pair.known
            && let Some(device) = state
                .devices
                .iter()
                .find(|device| !known.contains(&device.id))
        {
            tour.pair.paired = Some(device.name.clone());
            tour.pair.known = None;
            tour.pair.wanted = false;
            tour.pair.turned_on = false;
        }
        if tour.pair.wanted && self.mobile.error.is_some() {
            tour.pair.wanted = false;
            return;
        }
        if !tour.pair.wanted || self.mobile.busy {
            return;
        }
        if self.mobile.pairing.is_some() {
            tour.pair.wanted = false;
            return;
        }
        if !state.settings.enabled {
            tour.pair.turned_on = true;
            self.set_device_access(true, cx);
            if !self.mobile.busy
                && let Some(tour) = &mut self.onboarding.tour
            {
                tour.pair.wanted = false;
            }
            return;
        }
        match &state.status {
            ListenerStatus::Listening => {
                tour.pair.wanted = false;
                self.show_pairing_code(cx);
            }
            ListenerStatus::Failed(message) => {
                tour.pair.wanted = false;
                self.mobile.error = Some(tr!("Not listening: %@", message).to_string());
            }
            _ => {}
        }
    }

    /// The mobile step is gone: its code is withdrawn, and device access goes
    /// back off if the tour turned it on and nothing paired.
    fn leave_onboarding_pairing(&mut self, turned_on: bool, cx: &mut Context<Self>) {
        self.withdraw_pairing(cx);
        if turned_on {
            self.mobile.restore_off = true;
            self.settle_restored_access(cx);
        }
    }
}
