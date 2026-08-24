use super::*;

pub(super) fn content(modal: &SettingsModal) -> Vec<AnyElement> {
    let style = modal.style();
    let mut sections = Vec::new();
    sections.extend(visible(
        modal,
        Category::Backup,
        "Export",
        Some("Saves your settings, projects, remote devices, shortcuts and customizations to a single .muxy file. Credentials such as SSH keys, passwords and paired mobile devices are never included."),
        true,
        vec![controls::row(
            style,
            "Export Muxy",
            controls::button(style, "backup-export", "Export…", false, |_, _, _| {}),
        )],
    ));
    sections.extend(visible(
        modal,
        Category::Backup,
        "Import",
        Some("Replaces all current Muxy data with the contents of a backup and restarts the app. Your current data is backed up first so it can be recovered."),
        false,
        vec![controls::row(
            style,
            "Import Muxy",
            controls::button(style, "backup-import", "Import…", false, |_, _, _| {}),
        )],
    ));
    sections
}
