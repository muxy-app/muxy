use std::future::Future;

use crate::{Client, ClientError};
use muxy_protocol::{
    CatalogPage, ErrorCode, MAX_PROJECTS, ProjectId, ProjectIntent, ProjectSession,
    ProjectSessions, ReplyBody, RequestBody, SessionId, SessionStatus,
};

/// How many times a paged read starts over when the catalog changes between
/// its pages.
const PAGED_ATTEMPTS: usize = 8;
/// The most terminals one listing returns.
const MAX_LISTED_SESSIONS: usize = 16_384;

fn catalog_changed<T>(result: &Result<T, ClientError>) -> bool {
    matches!(result, Err(ClientError::Server(error)) if error.code == ErrorCode::CatalogChanged)
}

/// Reads every page again until no page saw the catalog change, a bounded
/// number of times.
pub(crate) fn consistent<T>(
    mut read: impl FnMut() -> Result<T, ClientError>,
) -> Result<T, ClientError> {
    for _ in 0..PAGED_ATTEMPTS {
        let result = read();
        if !catalog_changed(&result) {
            return result;
        }
    }
    Err(ClientError::Timeout)
}

/// Like `consistent`, without blocking.
pub(crate) async fn consistent_async<T, F: Future<Output = Result<T, ClientError>>>(
    mut read: impl FnMut() -> F,
) -> Result<T, ClientError> {
    for _ in 0..PAGED_ATTEMPTS {
        let result = read().await;
        if !catalog_changed(&result) {
            return result;
        }
    }
    Err(ClientError::Timeout)
}

fn available(session: &ProjectSession) -> bool {
    !session.attached
        && matches!(
            session.status,
            SessionStatus::Live | SessionStatus::Starting
        )
}

impl Client {
    pub fn exec(
        &self,
        request: muxy_protocol::ExecRequest,
    ) -> Result<muxy_protocol::ExecResult, ClientError> {
        let timeout = std::time::Duration::from_millis(u64::from(request.timeout_ms) + 10_000);
        match self.request_with_timeout(RequestBody::Exec(request), timeout)? {
            ReplyBody::Exec(result) => Ok(result),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    pub fn cancel_exec(&self, job: u64) -> Result<(), ClientError> {
        match self.request(RequestBody::CancelExec(job))? {
            ReplyBody::ExecCancelled => Ok(()),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    pub fn activity(&self) -> Result<muxy_protocol::ActivitySnapshot, ClientError> {
        match self.request(RequestBody::ReadActivity)? {
            ReplyBody::Activity(snapshot) => Ok(snapshot),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    pub fn acknowledge_activity(&self, events: Vec<u64>) -> Result<(), ClientError> {
        match self.request(RequestBody::AcknowledgeActivity(events))? {
            ReplyBody::ActivityAcknowledged => Ok(()),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    pub fn claim_activity(&self, events: Vec<u64>) -> Result<Vec<u64>, ClientError> {
        match self.request(RequestBody::ClaimActivity(events))? {
            ReplyBody::ActivityClaimed(events) => Ok(events),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    /// The folders directly inside `directory`, an absolute path on the
    /// server's computer. Servers from before this request answer
    /// `Unsupported`.
    pub fn list_folders(
        &self,
        directory: muxy_protocol::ServerPath,
    ) -> Result<Vec<muxy_protocol::ServerPath>, ClientError> {
        match self.request(RequestBody::ListFolders(directory))? {
            ReplyBody::Folders(names) => Ok(names),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    /// Sends `bytes` as a file named `name` for `session` to keep on the
    /// server's computer, in chunks, and returns its absolute path there.
    /// Servers from before uploads answer `Unsupported`.
    pub fn upload(
        &self,
        session: SessionId,
        name: &str,
        bytes: &[u8],
    ) -> Result<muxy_protocol::ServerPath, ClientError> {
        if u64::try_from(bytes.len())
            .ok()
            .is_none_or(|length| length > muxy_protocol::MAX_UPLOAD_BYTES)
        {
            return Err(ClientError::Invalid(ErrorCode::BadRequest));
        }
        let upload = muxy_protocol::OperationId::new();
        let mut chunks = bytes.chunks(muxy_protocol::MAX_UPLOAD_CHUNK).peekable();
        let mut offset = 0;
        loop {
            let chunk = chunks.next().unwrap_or_default();
            let last = chunks.peek().is_none();
            let reply = self.request_with_timeout(
                RequestBody::Upload(muxy_protocol::UploadChunk {
                    session,
                    upload,
                    name: name.into(),
                    offset,
                    bytes: chunk.to_vec(),
                    last,
                }),
                std::time::Duration::from_secs(60),
            )?;
            offset += chunk.len() as u64;
            match (reply, last) {
                (ReplyBody::Uploaded(Some(path)), true) => return Ok(path),
                (ReplyBody::Uploaded(None), false) => {}
                (body, _) => return Err(ClientError::UnexpectedReply(Box::new(body))),
            }
        }
    }

    pub fn cancel_creation(
        &self,
        operation: muxy_protocol::OperationId,
    ) -> Result<(), ClientError> {
        match self.request(RequestBody::CancelCreation(operation))? {
            ReplyBody::CreationCancelled => Ok(()),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    pub fn catalog_page(
        &self,
        after: Option<ProjectId>,
        revision: Option<u64>,
    ) -> Result<CatalogPage, ClientError> {
        match self.request(RequestBody::ReadCatalog { after, revision })? {
            ReplyBody::Catalog(page)
                if revision.is_none_or(|revision| page.revision == revision)
                    && page
                        .projects
                        .first()
                        .is_none_or(|project| after.is_none_or(|after| project.id > after)) =>
            {
                Ok(page)
            }
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    /// Every project, from one consistent catalog revision.
    pub fn catalog(&self) -> Result<CatalogPage, ClientError> {
        consistent(|| self.catalog_snapshot())
    }

    fn catalog_snapshot(&self) -> Result<CatalogPage, ClientError> {
        let mut result = self.catalog_page(None, None)?;
        while let Some(after) = result.next {
            let page = self.catalog_page(Some(after), Some(result.revision))?;
            if page.server != result.server
                || page.home != result.home
                || page.next.is_some_and(|next| next <= after)
                || page
                    .projects
                    .first()
                    .is_some_and(|project| project.id <= after)
                || result.projects.len() + page.projects.len() > MAX_PROJECTS
            {
                return Err(ClientError::Protocol("invalid catalog pagination".into()));
            }
            result.projects.extend(page.projects);
            result.next = page.next;
        }
        Ok(result)
    }

    pub fn mutate_project(&self, intent: ProjectIntent) -> Result<u64, ClientError> {
        match self.request(RequestBody::MutateProject(intent))? {
            ReplyBody::ProjectMutated { revision } => Ok(revision),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    /// The project's terminals that are running and shown nowhere.
    pub fn available_project_sessions(
        &self,
        project: ProjectId,
    ) -> Result<ProjectSessions, ClientError> {
        self.all_project_sessions(project, available)
    }

    /// Every terminal of the project that `keep` accepts, from one consistent
    /// listing.
    pub fn all_project_sessions(
        &self,
        project: ProjectId,
        keep: impl Fn(&ProjectSession) -> bool,
    ) -> Result<ProjectSessions, ClientError> {
        consistent(|| self.project_sessions_snapshot(project, &keep))
    }

    fn project_sessions_snapshot(
        &self,
        project: ProjectId,
        keep: &impl Fn(&ProjectSession) -> bool,
    ) -> Result<ProjectSessions, ClientError> {
        let mut result = self.project_sessions(project, None, None)?;
        result.sessions.retain(|session| keep(session));
        while let Some(after) = result.next {
            let page = self.project_sessions(project, Some(after), Some(result.revision))?;
            if page.next.is_some_and(|next| next <= after) {
                return Err(ClientError::Protocol("invalid session pagination".into()));
            }
            result
                .sessions
                .extend(page.sessions.into_iter().filter(|session| keep(session)));
            if result.sessions.len() > MAX_LISTED_SESSIONS {
                return Err(ClientError::Protocol("too many terminals".into()));
            }
            result.next = page.next;
        }
        Ok(result)
    }

    pub fn project_sessions(
        &self,
        project: ProjectId,
        after: Option<SessionId>,
        revision: Option<u64>,
    ) -> Result<ProjectSessions, ClientError> {
        match self.request(RequestBody::ListProjectSessions {
            project,
            after,
            revision,
        })? {
            ReplyBody::ProjectSessions(page)
                if revision.is_none_or(|revision| page.revision == revision)
                    && page
                        .sessions
                        .iter()
                        .all(|session| session.info.project == project)
                    && page.sessions.first().is_none_or(|session| {
                        after.is_none_or(|after| session.info.id > after)
                    }) =>
            {
                Ok(page)
            }
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::Fake;
    use muxy_protocol::{ErrorReply, ProjectSession, ServerPath, SessionInfo};

    fn changed() -> ReplyBody {
        ReplyBody::Error(ErrorReply {
            code: ErrorCode::CatalogChanged,
            message: "changed".into(),
        })
    }

    fn page(revision: u64, sessions: Vec<ProjectSession>, next: Option<SessionId>) -> ReplyBody {
        ReplyBody::ProjectSessions(ProjectSessions {
            revision,
            sessions,
            next,
        })
    }

    fn session(project: ProjectId, id: u64, attached: bool) -> ProjectSession {
        ProjectSession {
            info: SessionInfo {
                id: SessionId::new(id).expect("id"),
                project,
                directory: ServerPath(b"/tmp".to_vec()),
            },
            status: SessionStatus::Live,
            attached,
            owner: None,
        }
    }

    #[test]
    fn a_listing_starts_over_when_the_catalog_changes_between_pages() {
        let fake = Fake::start();
        let project = ProjectId::new();
        let first = session(project, 1, false);
        let second = session(project, 2, true);
        let third = session(project, 3, false);
        fake.queue(page(1, vec![first.clone()], Some(first.info.id)));
        fake.queue(changed());
        fake.queue(page(
            2,
            vec![first.clone(), second],
            Some(SessionId::new(2).expect("id")),
        ));
        fake.queue(page(2, vec![third.clone()], None));
        let listing = fake
            .client
            .available_project_sessions(project)
            .expect("listing");
        assert_eq!(listing.revision, 2);
        assert_eq!(listing.sessions, [first, third]);
        assert_eq!(fake.answered().len(), 4);
    }

    #[test]
    fn a_listing_gives_up_after_eight_changes() {
        let fake = Fake::start();
        for _ in 0..PAGED_ATTEMPTS {
            fake.queue(changed());
        }
        let listing = fake.client.all_project_sessions(ProjectId::new(), |_| true);
        assert!(matches!(listing, Err(ClientError::Timeout)), "{listing:?}");
        assert_eq!(fake.answered().len(), PAGED_ATTEMPTS);
    }

    #[test]
    fn other_errors_are_not_retried() {
        let fake = Fake::start();
        fake.queue(ReplyBody::Error(ErrorReply {
            code: ErrorCode::BadRequest,
            message: "bad".into(),
        }));
        let listing = fake.client.all_project_sessions(ProjectId::new(), |_| true);
        assert!(
            matches!(listing, Err(ClientError::Server(_))),
            "{listing:?}"
        );
        assert_eq!(fake.answered().len(), 1);
    }
}
