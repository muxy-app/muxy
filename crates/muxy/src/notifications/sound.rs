use std::collections::HashMap;

pub trait SoundBackend {
    type Player;

    fn resolve(&mut self, name: &str) -> Option<Self::Player>;
    fn stop(&mut self, player: &mut Self::Player);
    fn play(&mut self, player: &mut Self::Player);
}

pub struct PlatformSoundBackend;

impl PlatformSoundBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlatformSoundBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "macos")]
impl SoundBackend for PlatformSoundBackend {
    type Player = objc2::rc::Retained<objc2_app_kit::NSSound>;

    fn resolve(&mut self, name: &str) -> Option<Self::Player> {
        objc2_app_kit::NSSound::soundNamed(&objc2_foundation::NSString::from_str(name))
    }

    fn stop(&mut self, player: &mut Self::Player) {
        let _ = player.stop();
    }

    fn play(&mut self, player: &mut Self::Player) {
        let _ = player.play();
    }
}

#[cfg(not(target_os = "macos"))]
impl SoundBackend for PlatformSoundBackend {
    type Player = ();

    fn resolve(&mut self, _name: &str) -> Option<Self::Player> {
        None
    }

    fn stop(&mut self, _player: &mut Self::Player) {}

    fn play(&mut self, _player: &mut Self::Player) {}
}

pub struct NotificationSoundPlayer<B: SoundBackend = PlatformSoundBackend> {
    backend: B,
    cache: HashMap<String, B::Player>,
}

impl NotificationSoundPlayer<PlatformSoundBackend> {
    pub fn new() -> Self {
        Self::with_backend(PlatformSoundBackend::new())
    }
}

impl Default for NotificationSoundPlayer<PlatformSoundBackend> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: SoundBackend> NotificationSoundPlayer<B> {
    pub fn with_backend(backend: B) -> Self {
        Self {
            backend,
            cache: HashMap::new(),
        }
    }

    pub fn play(&mut self, name: &str) -> bool {
        if name == "None" || !muxy_core::prefs::settings::NOTIFICATION_SOUNDS.contains(&name) {
            return false;
        }
        if !self.cache.contains_key(name) {
            let Some(player) = self.backend.resolve(name) else {
                return false;
            };
            self.cache.insert(name.to_owned(), player);
        }
        let player = self.cache.get_mut(name).expect("cached sound");
        self.backend.stop(player);
        self.backend.play(player);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct Calls {
        resolved: Vec<String>,
        stopped: Vec<String>,
        played: Vec<String>,
    }

    struct FakeBackend {
        calls: Rc<RefCell<Calls>>,
    }

    impl SoundBackend for FakeBackend {
        type Player = String;

        fn resolve(&mut self, name: &str) -> Option<Self::Player> {
            self.calls.borrow_mut().resolved.push(name.to_owned());
            Some(name.to_owned())
        }

        fn stop(&mut self, player: &mut Self::Player) {
            self.calls.borrow_mut().stopped.push(player.clone());
        }

        fn play(&mut self, player: &mut Self::Player) {
            self.calls.borrow_mut().played.push(player.clone());
        }
    }

    #[test]
    fn notifications_sound_allows_exact_catalog_and_rejects_none_and_unknown() {
        let calls = Rc::new(RefCell::new(Calls::default()));
        let mut player = NotificationSoundPlayer::with_backend(FakeBackend {
            calls: calls.clone(),
        });
        for name in muxy_core::prefs::settings::NOTIFICATION_SOUNDS
            .iter()
            .copied()
            .filter(|name| *name != "None")
        {
            assert!(player.play(name));
        }
        assert!(!player.play("None"));
        assert!(!player.play("Unknown"));
        let calls = calls.borrow();
        assert_eq!(calls.resolved.len(), 14);
        assert_eq!(calls.stopped.len(), 14);
        assert_eq!(calls.played.len(), 14);
    }

    #[test]
    fn notifications_sound_reuses_cache_and_stops_before_every_play() {
        let calls = Rc::new(RefCell::new(Calls::default()));
        let mut player = NotificationSoundPlayer::with_backend(FakeBackend {
            calls: calls.clone(),
        });
        assert!(player.play("Funk"));
        assert!(player.play("Funk"));
        let calls = calls.borrow();
        assert_eq!(calls.resolved, ["Funk"]);
        assert_eq!(calls.stopped, ["Funk", "Funk"]);
        assert_eq!(calls.played, ["Funk", "Funk"]);
    }
}
