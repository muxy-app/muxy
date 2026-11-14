use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use muxy_protocol::wire::{Decoder, Encoder};
use muxy_protocol::{CONTROL, Message, ReplyBody, RequestBody, ServerInfo};

use crate::{Client, ClientError, Request};

/// A server that answers each request with the next queued reply.
pub(crate) struct Fake {
    pub(crate) client: Client,
    requests: Receiver<RequestBody>,
    replies: Sender<ReplyBody>,
}

impl Fake {
    pub(crate) fn start() -> Self {
        let (client, server) = muxy_protocol::transport::socket_pair().expect("socket pair");
        let (requests, received) = mpsc::channel();
        let (replies, queued) = mpsc::channel::<ReplyBody>();
        std::thread::spawn(move || {
            let mut decoder = Decoder::new(server.try_clone().expect("clone"));
            let mut encoder = Encoder::new(server);
            decoder.next().expect("hello");
            encoder
                .send(
                    CONTROL,
                    &Message::HelloReply {
                        versions: muxy_protocol::SUPPORTED.to_vec(),
                        server: ServerInfo::current(),
                        features: Vec::new(),
                    },
                )
                .expect("hello reply");
            while let Ok((_, Message::Request { id, body })) = decoder.next() {
                let Ok(reply) = queued.recv() else { return };
                if requests.send(body).is_err()
                    || encoder
                        .send(CONTROL, &Message::Reply { id, body: reply })
                        .is_err()
                {
                    return;
                }
            }
        });
        Self {
            client: Client::from_stream(Box::new(client)).expect("connect"),
            requests: received,
            replies,
        }
    }

    /// Queues the answer to a later request.
    pub(crate) fn queue(&self, reply: ReplyBody) {
        self.replies.send(reply).expect("queue reply");
    }

    /// The requests answered so far and not yet taken.
    pub(crate) fn answered(&self) -> Vec<RequestBody> {
        self.requests.try_iter().collect()
    }

    /// Runs `call` with `reply` queued, and returns what it sent.
    pub(crate) fn answer<T>(
        &self,
        reply: ReplyBody,
        call: impl FnOnce(&Client) -> T,
    ) -> (RequestBody, T) {
        self.queue(reply);
        let result = call(&self.client);
        let request = self
            .requests
            .recv_timeout(Duration::from_secs(5))
            .expect("request");
        (request, result)
    }
}

pub(crate) fn wait<T: Send + 'static>(request: Request<T>) -> Result<T, ClientError> {
    let (done, result) = mpsc::channel();
    request.on_complete(move |value| {
        let _ = done.send(value);
    });
    result
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or(Err(ClientError::Timeout))
}
