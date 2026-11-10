use muxy_protocol::{HistoryCursor, ImageFrame, PlaybackState, ProjectId, Size};
use muxy_server::{Registry, ServerSettings, SessionCommand, SessionHandle};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// A red pixel that becomes blue: a Kitty animation of two frames, playing.
const ANIMATION: &str = concat!(
    "\x1b_Ga=T,i=5,f=32,s=1,v=1,q=2;/wAA/w==\x1b\\",
    "\x1b_Ga=f,i=5,f=32,s=1,v=1,z=100,q=2;AAD//w==\x1b\\",
    "\x1b_Ga=a,i=5,r=1,z=50,s=3,v=1,q=2\x1b\\",
);

fn graphics(handle: &SessionHandle) -> Result<muxy_protocol::Graphics> {
    let (reply, receive) = mpsc::channel();
    handle.send(SessionCommand::HistoryPage {
        before: HistoryCursor(0),
        max_rows: 1,
        reply,
    })?;
    let page = receive.recv_timeout(Duration::from_secs(5))??;
    Ok(page.screen.ok_or("no screen")?.graphics)
}

fn frames(handle: &SessionHandle, image: u32, keys: Vec<u64>) -> Result<Vec<ImageFrame>> {
    let (reply, receive) = mpsc::channel();
    handle.send(SessionCommand::ImageFrames { image, keys, reply })?;
    Ok(receive.recv_timeout(Duration::from_secs(5))??)
}

#[test]
fn a_session_lists_an_animation_and_hands_out_its_frames() -> Result {
    let directory = std::env::temp_dir().join(format!("muxy-images-{}", ProjectId::new()));
    fs::create_dir(&directory)?;
    fs::write(directory.join("output"), ANIMATION)?;
    let shell = directory.join("shell");
    fs::write(&shell, "#!/bin/sh\ncat output\nexec /bin/sleep 60\n")?;
    fs::set_permissions(&shell, fs::Permissions::from_mode(0o700))?;
    let (events, _) = mpsc::channel();
    let registry = Registry::new(
        ServerSettings {
            default_shell: Some(shell),
            shell_integration: false,
            ..ServerSettings::default()
        },
        events,
    );
    let info = registry.create(&directory, Size { cols: 20, rows: 5 })?;
    let handle = registry.handle(info.id).ok_or("session disappeared")?;

    let deadline = Instant::now() + Duration::from_secs(10);
    let image = loop {
        let graphics = graphics(&handle)?;
        if let Some(image) = graphics.images.first()
            && image.playback.state == PlaybackState::Running
        {
            break image.clone();
        }
        assert!(Instant::now() < deadline, "the animation never showed");
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(
        image
            .frames
            .iter()
            .map(|frame| frame.gap)
            .collect::<Vec<_>>(),
        [50, 100]
    );
    let keys: Vec<u64> = image.frame_keys().collect();
    let pixels: Vec<Vec<u8>> = frames(&handle, image.id, keys.clone())?
        .iter()
        .map(|frame| frame.rgba.to_vec())
        .collect();
    assert_eq!(pixels, [[255, 0, 0, 255], [0, 0, 255, 255]]);

    // The server plays nothing itself, so the image is the same a while later.
    thread::sleep(Duration::from_millis(200));
    assert_eq!(graphics(&handle)?.images[0], image);
    assert!(frames(&handle, image.id + 1, keys)?.is_empty());

    registry.shutdown();
    let _ = fs::remove_dir_all(directory);
    Ok(())
}
