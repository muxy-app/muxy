pub mod support;

use muxy_proto::session::{Resize, WindowSize};
use muxy_session::{RendererClient, RendererEvent, identity_is_alive};
use std::time::{Duration, Instant};
use support::{TestDaemon, read_until, renderer};

#[test]
fn shell_survives_detach_and_reattaches_with_replay_and_live_output() {
    let daemon = TestDaemon::start();
    let mut control = daemon.client();
    let descriptor = control
        .create_session(daemon.request("detach-reattach"))
        .unwrap();
    let shell = descriptor.shell;

    let mut first = renderer(&daemon.socket, descriptor.session_id, 1);
    disable_echo(&mut first);
    first.send_input(b"printf 'P8_FIRST\\n'\n").unwrap();
    read_until(&mut first, b"P8_FIRST");
    first.disconnect().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(identity_is_alive(shell));

    let mut second = renderer(&daemon.socket, descriptor.session_id, 2);
    read_until(&mut second, b"P8_FIRST");
    second.send_input(b"printf 'P8_SECOND\\n'\n").unwrap();
    read_until(&mut second, b"P8_SECOND");

    let second_generation = second.attached.attachment_generation;
    let mut replacement = renderer(&daemon.socket, descriptor.session_id, 2);
    let replacement_generation = replacement.attached.attachment_generation;
    assert_ne!(second_generation, replacement_generation);
    replacement
        .send_resize(Resize {
            attachment_generation: replacement_generation,
            resize_generation: 1,
            size: WindowSize::new(100, 30),
        })
        .unwrap();
    let _ = second.send_resize(Resize {
        attachment_generation: second_generation,
        resize_generation: 99,
        size: WindowSize::new(40, 10),
    });
    replacement
        .send_input(b"stty size; printf 'P8_REPLACED\\n'\n")
        .unwrap();
    let replacement_output = read_until(&mut replacement, b"P8_REPLACED");
    assert!(
        replacement_output
            .windows(b"30 100".len())
            .any(|window| window == b"30 100")
    );
    assert!(second.next_event().unwrap().is_none());

    replacement.disconnect().unwrap();
    control.end_session(descriptor.session_id).unwrap();
    assert!(!identity_is_alive(shell));
    drop(control);
    daemon.finish(false);
}

#[test]
fn non_reading_renderer_disconnects_at_cap_without_blocking_pty_or_end() {
    let daemon = TestDaemon::start();
    let mut control = daemon.client();
    let descriptor = control
        .create_session(daemon.request("output-cap"))
        .unwrap();
    let mut non_reading = renderer(&daemon.socket, descriptor.session_id, 1);
    disable_echo(&mut non_reading);
    non_reading
        .send_input(
            b"/usr/bin/yes P8_CAP | /usr/bin/head -c 10485760; : > p8-output-complete; printf '\\nP8_DRAINED\\n'\n",
        )
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let attached = control
            .get_session(descriptor.session_id)
            .unwrap()
            .unwrap()
            .renderer_attached;
        if !attached {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "non-reading renderer was not disconnected"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    control.ping().unwrap();
    drop(non_reading);
    let marker = daemon.root.path().join("p8-output-complete");
    while !marker.exists() {
        assert!(
            Instant::now() < deadline,
            "PTY did not drain after renderer disconnection"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    let mut replacement = renderer(&daemon.socket, descriptor.session_id, 2);
    read_ordered_until(&mut replacement, b"P8_DRAINED");
    replacement
        .send_input(b"printf 'P8_AFTER_CAP\\n'\n")
        .unwrap();
    read_ordered_until(&mut replacement, b"P8_AFTER_CAP");
    replacement.disconnect().unwrap();
    control.end_session(descriptor.session_id).unwrap();
    drop(control);
    daemon.finish(false);
}

fn disable_echo(renderer: &mut RendererClient) {
    renderer.send_input(b"stty -echo\n").unwrap();
    std::thread::sleep(Duration::from_millis(100));
}

fn read_ordered_until(renderer: &mut RendererClient, needle: &[u8]) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut previous = None;
    let mut bytes = Vec::new();
    while Instant::now() < deadline {
        match renderer.next_event() {
            Ok(Some(RendererEvent::Output {
                sequence,
                bytes: output,
            })) => {
                if let Some(previous) = previous {
                    assert!(sequence > previous, "output sequence did not increase");
                }
                previous = Some(sequence);
                bytes.extend_from_slice(&output);
                if bytes.windows(needle.len()).any(|window| window == needle) {
                    return;
                }
            }
            Ok(Some(RendererEvent::Exited(_))) | Ok(None) => break,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => panic!("renderer read failed: {error}"),
        }
    }
    panic!("renderer output did not contain {:?}", needle)
}
