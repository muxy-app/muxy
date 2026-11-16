use std::fs;
use std::io;
use std::path::Path;

pub(crate) const AGENT_SKILLS: [&str; 2] = [".claude/skills", ".agents/skills"];

pub(crate) fn refuse_links_on_the_way(folder: &Path, relative: &str) -> io::Result<()> {
    let mut path = folder.to_owned();
    for part in Path::new(relative)
        .parent()
        .into_iter()
        .flat_map(Path::components)
    {
        path.push(part);
        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
            return Err(io::Error::other(format!(
                "{} is a link; Muxy keeps its skill in real folders",
                path.display()
            )));
        }
    }
    Ok(())
}

pub(crate) fn write_if_changed(path: &Path, contents: &str) -> io::Result<()> {
    if fs::read(path).is_ok_and(|current| current == contents.as_bytes()) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    muxy_core::fs::write_atomic(path, contents.as_bytes(), 0o644)?;
    Ok(())
}
