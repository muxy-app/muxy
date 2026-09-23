//! `http.fetch` follows main's order: validate, refuse private hosts, ask for
//! consent per host, then run the request off the main thread.

use gpui::Context;
use muxy_app_core::extensions::Request;

use super::{AppModel, Call};
use crate::extensions::{http, io};

impl AppModel {
    pub(super) fn check_http(&mut self, call: Call, cx: &mut Context<Self>) {
        let args = call.args.clone();
        let checked = io::lookup(move || {
            let request = http::validate(&args)?;
            if http::blocked(&request.host) {
                return Err(format!(
                    "http: blocked request to private or loopback host '{}'",
                    request.host
                ));
            }
            Ok(Request::http(
                &request.host,
                &request.method,
                request.url.as_str(),
            ))
        });
        let handle = self.window;
        cx.spawn(async move |model, cx| {
            let checked = checked.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = model.update(cx, |model, cx| match checked {
                    Ok(request) if model.call_live(&call, cx) => {
                        model.gate(call, request, window, cx);
                    }
                    Ok(_) => call.reply.send(Err("extension call expired".into()), cx),
                    Err(error) => call.reply.send(Err(error), cx),
                });
            });
        })
        .detach();
    }

    pub(super) fn http_fetch(call: Call, cx: &mut Context<Self>) {
        let args = call.args.clone();
        let response = io::network(move || http::fetch(http::validate(&args)?));
        cx.spawn(async move |model, cx| {
            let response = response.await;
            let _ = model.update(cx, |model, cx| {
                if model.call_live(&call, cx) {
                    call.reply.send(response, cx);
                } else {
                    call.reply.send(Err("extension call expired".into()), cx);
                }
            });
        })
        .detach();
    }
}
