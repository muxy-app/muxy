use std::ptr::NonNull;

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, ClassType, DefinedClass, define_class, msg_send};
use objc2_foundation::{NSDictionary, NSError, NSObject, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptionNone, UNNotificationPresentationOptions, UNNotificationRequest,
    UNNotificationResponse, UNNotificationSettings, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};

use super::{
    AuthorizationResult, AuthorizationStatus, DesktopRequest, complete_response, response_action,
};

struct DelegateIvars {
    sender: async_channel::Sender<String>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "MuxyNotificationCenterDelegate"]
    #[ivars = DelegateIvars]
    struct NotificationCenterDelegate;

    impl NotificationCenterDelegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            completion_handler.call((foreground_presentation_options(),));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive_response(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion_handler: &DynBlock<dyn Fn()>,
        ) {
            let action = response.actionIdentifier().to_string();
            let notification_id = response_notification_id(response);
            complete_response(
                response_action(&action),
                notification_id.as_deref(),
                &self.ivars().sender,
                || completion_handler.call(()),
            );
        }
    }

    unsafe impl NSObjectProtocol for NotificationCenterDelegate {}
    unsafe impl UNUserNotificationCenterDelegate for NotificationCenterDelegate {}
);

impl NotificationCenterDelegate {
    fn new(sender: async_channel::Sender<String>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars { sender });
        unsafe { msg_send![super(this), init] }
    }
}

pub struct PlatformDesktopService {
    center: Retained<UNUserNotificationCenter>,
    _delegate: Retained<NotificationCenterDelegate>,
}

impl PlatformDesktopService {
    pub fn prepare(sender: async_channel::Sender<String>) -> Self {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let delegate = NotificationCenterDelegate::new(sender);
        let protocol: &ProtocolObject<dyn UNUserNotificationCenterDelegate> =
            ProtocolObject::from_ref(&*delegate);
        center.setDelegate(Some(protocol));
        Self {
            center,
            _delegate: delegate,
        }
    }

    pub fn query_authorization(&self) -> async_channel::Receiver<AuthorizationStatus> {
        let (sender, receiver) = async_channel::bounded(1);
        let completion = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            let settings = unsafe { settings.as_ref() };
            let _ = sender.try_send(map_authorization_status(settings.authorizationStatus()));
        });
        self.center
            .getNotificationSettingsWithCompletionHandler(&completion);
        receiver
    }

    pub fn request_authorization(&self) -> async_channel::Receiver<AuthorizationResult> {
        let (sender, receiver) = async_channel::bounded(1);
        let completion = RcBlock::new(move |allowed: Bool, error: *mut NSError| {
            let _ = sender.try_send(map_authorization_result(
                allowed.as_bool(),
                !error.is_null(),
            ));
        });
        self.center
            .requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert,
                &completion,
            );
        receiver
    }

    pub fn schedule(&self, request: DesktopRequest) {
        let center = self.center.clone();
        let completion = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            let settings = unsafe { settings.as_ref() };
            if !map_authorization_status(settings.authorizationStatus()).allows_scheduling() {
                return;
            }
            let native_request = native_request(&request);
            center.addNotificationRequest_withCompletionHandler(&native_request, None);
        });
        self.center
            .getNotificationSettingsWithCompletionHandler(&completion);
    }
}

fn map_authorization_status(status: UNAuthorizationStatus) -> AuthorizationStatus {
    match status {
        UNAuthorizationStatus::NotDetermined => AuthorizationStatus::NotDetermined,
        UNAuthorizationStatus::Denied => AuthorizationStatus::Denied,
        UNAuthorizationStatus::Authorized => AuthorizationStatus::Authorized,
        UNAuthorizationStatus::Provisional => AuthorizationStatus::Provisional,
        UNAuthorizationStatus::Ephemeral => AuthorizationStatus::Ephemeral,
        _ => AuthorizationStatus::Unavailable,
    }
}

fn map_authorization_result(allowed: bool, failed: bool) -> AuthorizationResult {
    if failed {
        AuthorizationResult::Failed
    } else if allowed {
        AuthorizationResult::Allowed
    } else {
        AuthorizationResult::Denied
    }
}

fn foreground_presentation_options() -> UNNotificationPresentationOptions {
    UNNotificationPresentationOptionNone
}

fn native_request(request: &DesktopRequest) -> Retained<UNNotificationRequest> {
    let identifier = NSString::from_str(&request.identifier);
    let title = NSString::from_str(&request.title);
    let body = NSString::from_str(&request.body);
    let key = NSString::from_str("notificationID");
    let value = NSString::from_str(&request.notification_id);
    let user_info = NSDictionary::<NSString, NSString>::from_slices(&[&*key], &[&*value]);
    let user_info: Retained<NSDictionary> = unsafe { Retained::cast_unchecked(user_info) };
    let content = UNMutableNotificationContent::new();
    content.setTitle(&title);
    content.setBody(&body);
    unsafe { content.setUserInfo(&user_info) };
    unsafe {
        msg_send![
            UNNotificationRequest::class(),
            requestWithIdentifier: &*identifier,
            content: &*content,
            trigger: Option::<&AnyObject>::None
        ]
    }
}

fn response_notification_id(response: &UNNotificationResponse) -> Option<String> {
    let user_info = response.notification().request().content().userInfo();
    let key = NSString::from_str("notificationID");
    user_info
        .objectForKey(&key)
        .and_then(|value| value.downcast::<NSString>().ok())
        .map(|value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_desktop_native_status_mapping_is_complete() {
        assert_eq!(
            map_authorization_status(UNAuthorizationStatus::NotDetermined),
            AuthorizationStatus::NotDetermined
        );
        assert_eq!(
            map_authorization_status(UNAuthorizationStatus::Denied),
            AuthorizationStatus::Denied
        );
        assert_eq!(
            map_authorization_status(UNAuthorizationStatus::Authorized),
            AuthorizationStatus::Authorized
        );
        assert_eq!(
            map_authorization_status(UNAuthorizationStatus::Provisional),
            AuthorizationStatus::Provisional
        );
        assert_eq!(
            map_authorization_status(UNAuthorizationStatus::Ephemeral),
            AuthorizationStatus::Ephemeral
        );
        assert_eq!(
            map_authorization_status(UNAuthorizationStatus(99)),
            AuthorizationStatus::Unavailable
        );
    }

    #[test]
    fn notifications_desktop_native_authorization_result_and_foreground_policy_are_exact() {
        assert_eq!(
            map_authorization_result(true, false),
            AuthorizationResult::Allowed
        );
        assert_eq!(
            map_authorization_result(false, false),
            AuthorizationResult::Denied
        );
        assert_eq!(
            map_authorization_result(true, true),
            AuthorizationResult::Failed
        );
        assert_eq!(
            map_authorization_result(false, true),
            AuthorizationResult::Failed
        );
        assert_eq!(
            foreground_presentation_options(),
            UNNotificationPresentationOptionNone
        );
    }

    #[test]
    fn notifications_desktop_native_request_contains_only_the_navigation_id() {
        let request = DesktopRequest::new("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee", "Title", "Body")
            .expect("request");
        let native = native_request(&request);
        assert_eq!(native.identifier().to_string(), request.identifier);
        let content = native.content();
        assert_eq!(content.title().to_string(), "Title");
        assert_eq!(content.body().to_string(), "Body");
        let user_info = content.userInfo();
        assert_eq!(user_info.len(), 1);
        let key = NSString::from_str("notificationID");
        let value = user_info
            .objectForKey(&key)
            .expect("notification ID")
            .downcast::<NSString>()
            .expect("string");
        assert_eq!(value.to_string(), request.notification_id);
    }
}
