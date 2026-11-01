use muxy_app_core::project_layouts::{Config, DIRECTORY, Descriptor, discover};
use muxy_client::Client;
use muxy_protocol::{ProjectId, ServerPath};

pub(super) fn list(client: &Client, project: ProjectId) -> Result<Vec<Descriptor>, String> {
    client
        .list_directory(project, ServerPath::from(DIRECTORY))
        .map(discover)
        .map_err(|error| error.to_string())
}

pub(super) fn load(
    client: &Client,
    project: ProjectId,
    layout: &Descriptor,
) -> Result<Config, String> {
    let file = client
        .read_file(project, layout.path.clone())
        .map_err(|error| error.to_string())?;
    Config::parse(&file.content)
}
