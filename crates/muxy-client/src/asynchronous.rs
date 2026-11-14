use crate::{Client, ClientError, Request};
use muxy_protocol::{CatalogPage, ChannelId, MAX_PROJECTS, ProjectId, ReplyBody, RequestBody};
use std::time::Duration;

impl Client {
    pub fn project_has_sessions_async(&self, project: ProjectId) -> Request<bool> {
        self.request_async(
            RequestBody::ListProjectSessions {
                project,
                after: None,
                revision: None,
            },
            Duration::from_secs(5),
            |body| match body {
                ReplyBody::ProjectSessions(page) => {
                    Ok(!page.sessions.is_empty() || page.next.is_some())
                }
                body => Err(ClientError::UnexpectedReply(Box::new(body))),
            },
        )
    }

    pub fn exec_async(
        &self,
        request: muxy_protocol::ExecRequest,
    ) -> Request<muxy_protocol::ExecResult> {
        let timeout = Duration::from_millis(u64::from(request.timeout_ms) + 10_000);
        self.request_async(RequestBody::Exec(request), timeout, |body| match body {
            ReplyBody::Exec(value) => Ok(value),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        })
    }

    /// Detaches without blocking, such as from the thread reading events.
    pub fn detach_async(&self, channel: ChannelId) -> Request<()> {
        self.request_async(
            RequestBody::Detach(channel),
            crate::client::DEFAULT_TIMEOUT,
            |body| match body {
                ReplyBody::Detached => Ok(()),
                body => Err(ClientError::UnexpectedReply(Box::new(body))),
            },
        )
    }

    pub fn cancel_exec_async(&self, request: u64) -> Request<()> {
        let timeout = Duration::from_secs(5);
        self.request_async(
            RequestBody::CancelExec(request),
            timeout,
            |body| match body {
                ReplyBody::ExecCancelled => Ok(()),
                body => Err(ClientError::UnexpectedReply(Box::new(body))),
            },
        )
    }

    pub async fn catalog_async(&self) -> Result<CatalogPage, ClientError> {
        crate::catalog::consistent_async(|| self.catalog_snapshot_async()).await
    }

    async fn catalog_page_async(
        &self,
        after: Option<ProjectId>,
        revision: Option<u64>,
    ) -> Result<CatalogPage, ClientError> {
        let page = self
            .request_async(
                RequestBody::ReadCatalog { after, revision },
                Duration::from_secs(5),
                |body| match body {
                    ReplyBody::Catalog(page) => Ok(page),
                    body => Err(ClientError::UnexpectedReply(Box::new(body))),
                },
            )
            .await?;
        if revision.is_some_and(|revision| page.revision != revision)
            || page
                .projects
                .first()
                .is_some_and(|project| after.is_some_and(|after| project.id <= after))
        {
            return Err(ClientError::Protocol("invalid catalog pagination".into()));
        }
        Ok(page)
    }

    async fn catalog_snapshot_async(&self) -> Result<CatalogPage, ClientError> {
        let mut result = self.catalog_page_async(None, None).await?;
        while let Some(after) = result.next {
            let page = self
                .catalog_page_async(Some(after), Some(result.revision))
                .await?;
            if page.server != result.server
                || page.home != result.home
                || page.next.is_some_and(|next| next <= after)
                || result.projects.len() + page.projects.len() > MAX_PROJECTS
            {
                return Err(ClientError::Protocol("invalid catalog pagination".into()));
            }
            result.projects.extend(page.projects);
            result.next = page.next;
        }
        Ok(result)
    }
}
