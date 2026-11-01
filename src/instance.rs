//! One UI per application data directory; later launches activate the first.
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    time::Duration,
};
pub enum Start {
    Primary(Instance),
    Activated,
}
pub struct Instance {
    _lease: File,
    socket: PathBuf,
    listener: Option<UnixListener>,
    events: Receiver<()>,
    sender: mpsc::Sender<()>,
}
impl Instance {
    pub fn acquire() -> io::Result<Start> {
        let dirs = directories::ProjectDirs::from("", "", "battlenet-linux-launcher")
            .ok_or_else(|| io::Error::other("Cannot resolve application data."))?;
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        Self::acquire_at(dirs.data_local_dir().join("instance"), runtime)
    }
    fn acquire_at(data: PathBuf, base: PathBuf) -> io::Result<Start> {
        fs::create_dir_all(&data)?;
        fs::set_permissions(&data, fs::Permissions::from_mode(0o700))?;
        let tag = format!(
            "bnet-{:x}",
            md5::compute(data.as_os_str().as_encoded_bytes())
        );
        let directory = base.join(tag);
        fs::create_dir_all(&directory)?;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        let socket = directory.join("a.sock");
        let lease = File::options()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(data.join("ui.lock"))?;
        if lease.try_lock().is_err() {
            let mut attempts = 0;
            let mut stream = loop {
                match UnixStream::connect(&socket) {
                    Ok(stream) => break stream,
                    Err(error) if attempts < 20 => {
                        attempts += 1;
                        std::thread::sleep(Duration::from_millis(50));
                        let _ = error;
                    }
                    Err(error) => return Err(error),
                }
            };
            stream.set_write_timeout(Some(Duration::from_secs(1)))?;
            stream.write_all(b"show")?;
            return Ok(Start::Activated);
        }
        if socket.exists() {
            fs::remove_file(&socket)?;
        }
        let listener = UnixListener::bind(&socket)?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
        let (sender, events) = mpsc::channel();
        Ok(Start::Primary(Self {
            _lease: lease,
            socket,
            listener: Some(listener),
            events,
            sender,
        }))
    }
    pub fn attach(&mut self, ctx: eframe::egui::Context) {
        let Some(listener) = self.listener.take() else {
            return;
        };
        let tx = self.sender.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else {
                    break;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let mut request = [0; 4];
                if stream.read_exact(&mut request).is_ok() && request == *b"show" {
                    let _ = tx.send(());
                    ctx.request_repaint();
                }
            }
        });
    }
    pub fn activation_requested(&self) -> bool {
        self.events.try_iter().count() > 0
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket);
        if let Some(dir) = self.socket.parent() {
            let _ = fs::remove_dir(dir);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn second_launch_activates_primary_and_lock_is_released_on_exit() {
        let data = std::env::temp_dir().join(format!(
            "bni-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let Start::Primary(mut primary) = Instance::acquire_at(data.clone(), data.clone()).unwrap()
        else {
            panic!("No primary");
        };
        assert!(matches!(
            Instance::acquire_at(data.clone(), data.clone()).unwrap(),
            Start::Activated
        ));
        let (mut stream, _) = primary.listener.as_ref().unwrap().accept().unwrap();
        let mut bytes = [0; 4];
        stream.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"show");
        primary.listener.take();
        drop(primary);
        assert!(matches!(
            Instance::acquire_at(data.clone(), data.clone()).unwrap(),
            Start::Primary(_)
        ));
        fs::remove_dir_all(data).unwrap();
    }
}
