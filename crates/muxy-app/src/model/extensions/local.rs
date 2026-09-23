use std::collections::BTreeMap;
use std::path::Path;
use std::sync::mpsc::{SyncSender, sync_channel};

use muxy_app_core::extensions::{Grants, Registry, Settings, Storage};
use serde_json::{Map, Value};

pub(super) struct State {
    revision: u64,
    pub registry: Registry,
    pub grants: Result<Grants, String>,
    pub storage: Storage,
    pub settings: Settings,
}

impl State {
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            revision: self.revision,
            registry: self.registry.clone(),
            grants: self.grants.clone(),
            settings: self
                .registry
                .extensions
                .keys()
                .filter_map(|name| Some((name.clone(), self.settings.values(name).ok()?)))
                .collect(),
        }
    }
}

pub(super) struct Snapshot {
    pub revision: u64,
    pub registry: Registry,
    pub grants: Result<Grants, String>,
    pub settings: BTreeMap<String, Map<String, Value>>,
}

type Job = Box<dyn FnOnce(&mut State) + Send>;

#[derive(Clone)]
pub(super) struct Local {
    sender: Result<SyncSender<Job>, String>,
}

impl Local {
    pub(super) fn new(profile: &Path) -> Self {
        let profile = profile.to_owned();
        let (sender, jobs) = sync_channel::<Job>(64);
        let worker = std::thread::Builder::new()
            .name("extension-storage".into())
            .spawn(move || {
                let mut state = State {
                    revision: 0,
                    registry: Registry::load(&profile),
                    grants: Grants::load(&profile),
                    storage: Storage::new(&profile),
                    settings: Settings::new(&profile),
                };
                for job in jobs {
                    state.revision += 1;
                    job(&mut state);
                }
            });
        Self {
            sender: worker.map(|_| sender).map_err(|error| error.to_string()),
        }
    }

    pub(super) fn submit<
        T: Send + 'static,
        F: FnOnce(&mut State) -> Result<T, String> + Send + 'static,
    >(
        &self,
        job: F,
    ) -> impl Future<Output = Result<T, String>> + use<T, F> {
        let (reply, result) = async_channel::bounded(1);
        let failed = reply.clone();
        let queued = self
            .sender
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|sender| {
                sender
                    .try_send(Box::new(move |state| {
                        let _ = reply.try_send(job(state));
                    }))
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = queued {
            let _ = failed.try_send(Err(error));
        }
        async move {
            result
                .recv()
                .await
                .unwrap_or_else(|_| Err("extension worker stopped".into()))
        }
    }
}
