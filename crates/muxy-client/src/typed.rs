//! Typed requests on a project: one method per action, sync and async, each
//! returning what that action answers.

/// Reply decoders that accept only the expected variants of one reply kind.
macro_rules! decoders {
    ($kind:ident; $($name:ident -> $output:ty { $($pattern:pat => $value:expr),+ $(,)? })*) => {
        $(
            fn $name(
                body: muxy_protocol::ReplyBody,
            ) -> Result<$output, crate::ClientError> {
                match body {
                    $(muxy_protocol::ReplyBody::$kind($pattern) => Ok($value),)+
                    body => Err(crate::ClientError::UnexpectedReply(Box::new(body))),
                }
            }
        )*
    };
}

/// A sync and an async method for each action, sent with `$send` and
/// `$send_async` and read with the named decoder.
macro_rules! typed_requests {
    ($send:ident, $send_async:ident; $(
        $(#[$doc:meta])*
        $name:ident, $name_async:ident($($arg:ident: $type:ty),*) -> $output:ty
            = $action:expr => $decode:ident;
    )*) => {
        impl crate::Client {
            $(
                $(#[$doc])*
                pub fn $name(
                    &self,
                    project: muxy_protocol::ProjectId,
                    $($arg: $type),*
                ) -> Result<$output, crate::ClientError> {
                    self.$send(project, $action, $decode)
                }

                $(#[$doc])*
                pub fn $name_async(
                    &self,
                    project: muxy_protocol::ProjectId,
                    $($arg: $type),*
                ) -> crate::Request<$output> {
                    self.$send_async(project, $action, $decode)
                }
            )*
        }
    };
}

pub(crate) use {decoders, typed_requests};
