use muxy_app_core::project_layouts::{Config, DIRECTORY, Descriptor, discover};
use muxy_client::Client;
use muxy_protocol::{FilesAction, FilesReply, FilesRequest, ProjectId, ServerPath};

pub(super) fn list(client: &Client, project: ProjectId) -> Result<Vec<Descriptor>, String> {
    match client
        .files(FilesRequest {
            project,
            action: FilesAction::List(ServerPath(DIRECTORY.as_bytes().to_vec())),
        })
        .map_err(|error| error.to_string())?
    {
        FilesReply::Entries(entries) => Ok(discover(entries)),
        _ => Err("Unexpected layout directory response".into()),
    }
}

pub(super) fn load(
    client: &Client,
    project: ProjectId,
    layout: &Descriptor,
) -> Result<Config, String> {
    match client
        .files(FilesRequest {
            project,
            action: FilesAction::Read(layout.path.clone()),
        })
        .map_err(|error| error.to_string())?
    {
        FilesReply::Content(file) => Config::parse(&file.content),
        _ => Err("Unexpected layout file response".into()),
    }
}
