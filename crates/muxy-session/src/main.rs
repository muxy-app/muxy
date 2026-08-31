use muxy_proto::session::SessionId;
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("muxy-session: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [command] if command == "build-mode" => {
            let mode = match muxy_session::current_build_mode() {
                muxy_proto::session::BuildMode::Development => "debug",
                muxy_proto::session::BuildMode::Production => "release",
            };
            println!("{mode}");
            Ok(())
        }
        [command, flag, socket]
            if command == "daemon" && flag == "--socket" =>
        {
            muxy_session::run_daemon(PathBuf::from(socket))?;
            Ok(())
        }
        [command, socket_flag, socket, session_flag, session_id]
            if command == "attach"
                && socket_flag == "--socket"
                && session_flag == "--session-id" =>
        {
            let session_id = session_id
                .to_str()
                .ok_or("session ID is not UTF-8")?
                .parse::<SessionId>()?;
            muxy_session::run_attach(&PathBuf::from(socket), session_id)?;
            Ok(())
        }
        _ => Err("usage: muxy-session build-mode | muxy-session daemon --socket PATH | muxy-session attach --socket PATH --session-id UUID".into()),
    }
}
