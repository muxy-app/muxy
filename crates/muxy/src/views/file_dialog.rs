use std::path::PathBuf;

pub struct FolderRequest {
    pub message: &'static str,
    pub directory: Option<String>,
}

pub struct ImageRequest {
    pub title: &'static str,
    pub directory: Option<String>,
}

const IMAGE_EXTENSIONS: [&str; 10] = [
    "png", "jpg", "jpeg", "gif", "bmp", "tiff", "tif", "webp", "svg", "heic",
];

pub async fn pick_folder(request: FolderRequest) -> Option<PathBuf> {
    let mut dialog = rfd::AsyncFileDialog::new().set_title(request.message);
    if let Some(directory) = request.directory {
        dialog = dialog.set_directory(directory);
    }
    dialog
        .pick_folder()
        .await
        .map(|handle| handle.path().to_path_buf())
}

pub async fn pick_image(request: ImageRequest) -> Option<PathBuf> {
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title(request.title)
        .add_filter("Images", &IMAGE_EXTENSIONS);
    if let Some(directory) = request.directory {
        dialog = dialog.set_directory(directory);
    }
    dialog
        .pick_file()
        .await
        .map(|handle| handle.path().to_path_buf())
}
