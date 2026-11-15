use std::collections::HashSet;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, PoisonError};

use async_channel::{Receiver, Sender};
use block2::{DynBlock, RcBlock};
use muxy_core::capability::Capability;
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{NSArray, NSBundle, NSError, NSObject, NSObjectProtocol, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSettings, UNNotificationSound, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};

use super::Event;

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "MuxyNotificationDelegate"]
    #[ivars = Sender<Event>]
    #[derive(Debug)]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn present(&self, _: &UNUserNotificationCenter, _: &UNNotification, completion: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>) {
            completion.call((UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::Sound,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn respond(&self, _: &UNUserNotificationCenter, response: &UNNotificationResponse, completion: &DynBlock<dyn Fn()>) {
            let _ = self.ivars().try_send(Event::Opened(response.notification().request().identifier().to_string()));
            completion.call(());
        }
    }
);

pub const CAPABILITY: Capability = Capability::Supported;

#[derive(Debug)]
pub(super) struct Service {
    center: Retained<UNUserNotificationCenter>,
    _delegate: Retained<Delegate>,
    pending: Arc<PendingDeliveries>,
    permission: Arc<Permission>,
}

#[derive(Debug)]
struct Permission {
    denied: Mutex<bool>,
    events: Sender<Event>,
}

impl Permission {
    fn capability(denied: bool) -> Capability {
        if denied {
            Capability::Unavailable(
                crate::tr!("Notifications are turned off for Muxy in System Settings.").to_string(),
            )
        } else {
            Capability::Supported
        }
    }

    fn current(&self) -> Capability {
        Self::capability(*self.denied.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn report(&self, denied: bool) {
        let mut current = self.denied.lock().unwrap_or_else(PoisonError::into_inner);
        if *current != denied {
            *current = denied;
            let _ = self
                .events
                .try_send(Event::Capability(Self::capability(denied)));
        }
    }
}

#[derive(Debug, Default)]
struct PendingDeliveries(Mutex<HashSet<String>>);

impl PendingDeliveries {
    fn register(&self, id: String) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id);
    }

    fn cancel(&self, ids: &[String], clear: impl FnOnce()) {
        let mut pending = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        pending.retain(|id| !ids.contains(id));
        clear();
    }

    fn authorized(&self, id: &str, granted: bool, submit: impl FnOnce()) {
        let mut pending = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if pending.remove(id) && granted {
            submit();
        }
    }
}

impl Service {
    pub(super) fn clear(&self, ids: &[String]) {
        self.pending.cancel(ids, || {
            let ids: Vec<_> = ids.iter().map(|id| NSString::from_str(id)).collect();
            let ids = NSArray::from_retained_slice(&ids);
            self.center
                .removePendingNotificationRequestsWithIdentifiers(&ids);
            self.center
                .removeDeliveredNotificationsWithIdentifiers(&ids);
        });
    }

    pub(super) fn capability(&self) -> Capability {
        self.permission.current()
    }

    pub(super) fn refresh(&self) {
        let permission = Arc::clone(&self.permission);
        let completion = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            let status = unsafe { settings.as_ref() }.authorizationStatus();
            permission.report(status == UNAuthorizationStatus::Denied);
        });
        self.center
            .getNotificationSettingsWithCompletionHandler(&completion);
    }

    pub(super) fn start() -> Result<(Self, Receiver<Event>), Capability> {
        if NSBundle::mainBundle().bundleIdentifier().is_none() {
            return Err(Capability::Unavailable(
                crate::tr!("Desktop notifications need Muxy to run from its app bundle.")
                    .to_string(),
            ));
        }
        let (sender, receiver) = async_channel::bounded(32);
        let permission = Arc::new(Permission {
            denied: Mutex::new(false),
            events: sender.clone(),
        });
        let delegate = Delegate::alloc().set_ivars(sender);
        let delegate: Retained<Delegate> = unsafe { msg_send![super(delegate), init] };
        let center = UNUserNotificationCenter::currentNotificationCenter();
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        let service = Self {
            center,
            _delegate: delegate,
            pending: Arc::default(),
            permission,
        };
        service.refresh();
        Ok((service, receiver))
    }

    pub(super) fn inform(&self, id: &str, title: &str, body: &str) {
        self.deliver(id, title, body);
    }

    pub(super) fn deliver(&self, id: &str, title: &str, body: &str) {
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        content.setSound(Some(&UNNotificationSound::defaultSound()));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(id),
            &content,
            None,
        );
        let center = self.center.clone();
        let id = id.to_owned();
        let pending = Arc::clone(&self.pending);
        let permission = Arc::clone(&self.permission);
        pending.register(id.clone());
        let completion = RcBlock::new(move |granted: Bool, _: *mut NSError| {
            permission.report(!granted.as_bool());
            pending.authorized(&id, granted.as_bool(), || {
                center.addNotificationRequest_withCompletionHandler(&request, None);
            });
        });
        self.center
            .requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
                &completion,
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_denied_permission_is_reported_once_and_lifted_when_allowed_again() {
        let (events, reported) = async_channel::bounded(4);
        let permission = Permission {
            denied: Mutex::new(false),
            events,
        };
        permission.report(false);
        assert!(reported.try_recv().is_err(), "nothing changed");
        permission.report(true);
        permission.report(true);
        assert!(matches!(
            reported.try_recv(),
            Ok(Event::Capability(Capability::Unavailable(_)))
        ));
        assert!(reported.try_recv().is_err(), "reported once");
        assert!(!permission.current().is_supported());
        permission.report(false);
        assert_eq!(
            reported.try_recv(),
            Ok(Event::Capability(Capability::Supported))
        );
    }
}
