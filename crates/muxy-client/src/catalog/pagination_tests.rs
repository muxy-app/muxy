use super::*;
use crate::fake::Fake;
use muxy_protocol::{CATALOG_PAGE_SIZE, ProjectDescriptor, ServerIdentity, ServerPath};
use std::sync::{Arc, mpsc};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

struct Notify(mpsc::Sender<()>);

impl Wake for Notify {
    fn wake(self: Arc<Self>) {
        let _ = self.0.send(());
    }
}

fn read_async(client: &Client) -> Result<CatalogPage, ClientError> {
    let (notify, wake) = mpsc::channel();
    let waker = Waker::from(Arc::new(Notify(notify)));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(client.catalog_async());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Poll::Ready(result) = future.as_mut().poll(&mut context) {
            return result;
        }
        wake.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("catalog response");
    }
}

fn project(id: u128) -> ProjectDescriptor {
    let mut project = ProjectDescriptor::new(ServerPath(b"/project".to_vec()), []);
    project.id = ProjectId::from_u128(id);
    project.color = "#e5484d".into();
    project
}

fn page(ids: impl IntoIterator<Item = u128>, next: Option<u128>) -> CatalogPage {
    CatalogPage {
        server: ServerIdentity::from_u128(1),
        home: ProjectId::from_u128(0),
        revision: 7,
        projects: ids.into_iter().map(project).collect(),
        next: next.map(ProjectId::from_u128),
        legacy_home: Some(ProjectId::from_u128(99)),
    }
}

fn request(after: Option<u128>, revision: Option<u64>) -> RequestBody {
    RequestBody::ReadCatalog {
        after: after.map(ProjectId::from_u128),
        revision,
    }
}

fn assert_pagination_error(result: &Result<CatalogPage, ClientError>) {
    assert!(
        matches!(result, Err(ClientError::Protocol(message)) if message == "invalid catalog pagination"),
        "{result:?}"
    );
}

#[test]
fn catalog_readers_combine_pages_and_keep_first_page_metadata() {
    for read in [Client::catalog, read_async] {
        let fake = Fake::start();
        fake.queue(ReplyBody::Catalog(page([1, 2], Some(2))));
        let mut middle = page([3], Some(3));
        middle.legacy_home = None;
        fake.queue(ReplyBody::Catalog(middle));
        fake.queue(ReplyBody::Catalog(page([], None)));
        assert_eq!(read(&fake.client).expect("catalog"), page([1, 2, 3], None));
        assert_eq!(
            fake.answered(),
            [
                request(None, None),
                request(Some(2), Some(7)),
                request(Some(3), Some(7)),
            ]
        );
    }
}

#[test]
fn catalog_readers_discard_partial_pages_when_the_catalog_changes() {
    for read in [Client::catalog, read_async] {
        let fake = Fake::start();
        fake.queue(ReplyBody::Catalog(page([1], Some(1))));
        fake.queue(ReplyBody::Error(muxy_protocol::ErrorReply {
            code: ErrorCode::CatalogChanged,
            message: "changed".into(),
        }));
        let mut restarted = page([2], Some(2));
        restarted.revision = 8;
        fake.queue(ReplyBody::Catalog(restarted));
        let mut last = page([3], None);
        last.revision = 8;
        fake.queue(ReplyBody::Catalog(last));
        let mut expected = page([2, 3], None);
        expected.revision = 8;
        assert_eq!(read(&fake.client).expect("catalog"), expected);
        assert_eq!(
            fake.answered(),
            [
                request(None, None),
                request(Some(1), Some(7)),
                request(None, None),
                request(Some(2), Some(8)),
            ]
        );
    }
}

#[test]
fn catalog_readers_reject_changes_of_server_or_home() {
    for read in [Client::catalog, read_async] {
        for change in [
            |page: &mut CatalogPage| page.server = ServerIdentity::from_u128(2),
            |page: &mut CatalogPage| page.home = ProjectId::from_u128(10),
        ] {
            let fake = Fake::start();
            fake.queue(ReplyBody::Catalog(page([1], Some(1))));
            let mut next = page([2], None);
            change(&mut next);
            fake.queue(ReplyBody::Catalog(next));
            assert_pagination_error(&read(&fake.client));
            assert_eq!(fake.answered().len(), 2);
        }
    }
}

#[test]
fn catalog_readers_preserve_their_page_validation_errors() {
    for invalid in [
        {
            let mut page = page([2], None);
            page.revision += 1;
            page
        },
        page([1], Some(1)),
        page([1], None),
    ] {
        let fake = Fake::start();
        fake.queue(ReplyBody::Catalog(page([1], Some(1))));
        fake.queue(ReplyBody::Catalog(invalid.clone()));
        let result = fake.client.catalog();
        assert!(
            matches!(&result, Err(ClientError::UnexpectedReply(body)) if **body == ReplyBody::Catalog(invalid.clone())),
            "{result:?}"
        );
        assert_eq!(fake.answered().len(), 2);

        let fake = Fake::start();
        fake.queue(ReplyBody::Catalog(page([1], Some(1))));
        fake.queue(ReplyBody::Catalog(invalid));
        assert_pagination_error(&read_async(&fake.client));
        assert_eq!(fake.answered().len(), 2);
    }
}

#[test]
fn catalog_readers_accept_the_project_limit_and_reject_one_more() {
    for read in [Client::catalog, read_async] {
        for total in [MAX_PROJECTS, MAX_PROJECTS + 1] {
            let fake = Fake::start();
            let ids: Vec<_> = (1..=total as u128).collect();
            for chunk in ids.chunks(CATALOG_PAGE_SIZE) {
                let last = *chunk.last().expect("nonempty page");
                let next = (last < total as u128).then_some(last);
                fake.queue(ReplyBody::Catalog(page(chunk.iter().copied(), next)));
            }
            let result = read(&fake.client);
            if total == MAX_PROJECTS {
                assert_eq!(result.expect("catalog"), page(ids, None));
            } else {
                assert_pagination_error(&result);
            }
            assert_eq!(fake.answered().len(), total.div_ceil(CATALOG_PAGE_SIZE));
        }
    }
}
