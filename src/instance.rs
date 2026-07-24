use std::{
    net::UdpSocket,
    path::PathBuf,
    sync::mpsc::{self, Receiver, TrySendError},
    time::Duration,
};

const INSTANCE_ADDRESS: &str = "127.0.0.1:45872";
const ACKNOWLEDGEMENT: &[u8] = b"aurora-tlk-explorer-ok";
const MAX_PENDING_REQUESTS: usize = 32;

/// Become the primary Aurora process, or forward paths to the existing one.
///
/// Local UDP keeps this dependency-free and works on both Windows and Linux.
/// The acknowledgement prevents an unrelated process using the port from
/// suppressing a new Aurora launch.
pub fn acquire_or_forward(paths: Vec<PathBuf>) -> Result<Option<Receiver<Vec<PathBuf>>>, String> {
    let socket = match UdpSocket::bind(INSTANCE_ADDRESS) {
        Ok(socket) => socket,
        Err(_) => return forward_to_primary(paths).map(|()| None),
    };
    let (sender, receiver) = mpsc::sync_channel(MAX_PENDING_REQUESTS);
    std::thread::spawn(move || {
        loop {
            let mut buffer = [0_u8; 65_535];
            let Ok((length, peer)) = socket.recv_from(&mut buffer) else {
                continue;
            };
            let paths = serde_json::from_slice::<Vec<String>>(&buffer[..length])
                .map(|paths| paths.into_iter().map(PathBuf::from).collect::<Vec<_>>());
            if let Ok(paths) = paths {
                match sender.try_send(paths) {
                    Ok(()) | Err(TrySendError::Full(_)) => {
                        let _ = socket.send_to(ACKNOWLEDGEMENT, peer);
                    }
                    Err(TrySendError::Disconnected(_)) => break,
                }
            }
        }
    });
    Ok(Some(receiver))
}

fn forward_to_primary(paths: Vec<PathBuf>) -> Result<(), String> {
    let socket = UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    socket
        .set_read_timeout(Some(Duration::from_millis(750)))
        .map_err(|error| error.to_string())?;
    let paths: Vec<String> = paths
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    let request = serde_json::to_vec(&paths).map_err(|error| error.to_string())?;
    socket
        .send_to(&request, INSTANCE_ADDRESS)
        .map_err(|error| error.to_string())?;
    let mut acknowledgement = [0_u8; 64];
    let length = socket
        .recv(&mut acknowledgement)
        .map_err(|_| "Aurora is already starting; please try again in a moment".to_owned())?;
    if acknowledgement[..length] != *ACKNOWLEDGEMENT {
        return Err("Another application is using Aurora's local handoff port".to_owned());
    }
    Ok(())
}
