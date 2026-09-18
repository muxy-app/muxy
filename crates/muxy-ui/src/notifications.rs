//! Native delivery only; notification history and policy belong to callers.
use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};

use async_channel::{Receiver, Sender};
use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{NSArray, NSBundle, NSError, NSObject, NSObjectProtocol, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSound, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "MuxyNotificationDelegate"]
    #[ivars = Sender<String>]
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
            let _ = self.ivars().try_send(response.notification().request().identifier().to_string());
            completion.call(());
        }
    }
);

#[derive(Debug)]
pub struct Notifications {
    center: Retained<UNUserNotificationCenter>,
    _delegate: Retained<Delegate>,
    pending: Arc<PendingDeliveries>,
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

impl Notifications {
    pub fn clear(&self, ids: &[String]) {
        // Serialize cancellation with submission from the authorization callback.
        self.pending.cancel(ids, || {
            let ids: Vec<_> = ids.iter().map(|id| NSString::from_str(id)).collect();
            let ids = NSArray::from_retained_slice(&ids);
            self.center
                .removePendingNotificationRequestsWithIdentifiers(&ids);
            self.center
                .removeDeliveredNotificationsWithIdentifiers(&ids);
        });
    }

    /// Unbundled binaries cannot use the macOS notification service.
    pub fn new() -> Option<(Self, Receiver<String>)> {
        NSBundle::mainBundle().bundleIdentifier()?;
        let (sender, receiver) = async_channel::bounded(32);
        let delegate = Delegate::alloc().set_ivars(sender);
        // SAFETY: initialization of our NSObject subclass, with its ivars installed.
        let delegate: Retained<Delegate> = unsafe { msg_send![super(delegate), init] };
        let center = UNUserNotificationCenter::currentNotificationCenter();
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Some((
            Self {
                center,
                _delegate: delegate,
                pending: Arc::default(),
            },
            receiver,
        ))
    }

    pub fn deliver(&self, id: &str, title: &str, body: &str) {
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
        pending.register(id.clone());
        let completion = RcBlock::new(move |granted: Bool, _: *mut NSError| {
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
    use super::PendingDeliveries;
    use std::cell::Cell;

    #[test]
    fn read_acknowledgement_cancels_delivery_waiting_for_authorization() {
        let pending = PendingDeliveries::default();
        let delivered = Cell::new(false);
        pending.register("1".into());
        pending.cancel(&["1".into()], || delivered.set(false));
        pending.authorized("1", true, || delivered.set(true));
        assert!(!delivered.get());

        pending.register("2".into());
        pending.authorized("2", false, || delivered.set(true));
        assert!(!delivered.get());

        pending.register("3".into());
        pending.authorized("3", true, || delivered.set(true));
        assert!(delivered.get());
        pending.cancel(&["3".into()], || delivered.set(false));
        assert!(!delivered.get());
    }
}
