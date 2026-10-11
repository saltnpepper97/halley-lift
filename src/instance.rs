use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

#[derive(PartialEq, Eq)]
struct SocketIdentity {
    device: u64,
    inode: u64,
}

fn socket_identity(path: &Path) -> io::Result<SocketIdentity> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "path is not a socket",
        ));
    }
    Ok(SocketIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

pub struct InstanceGuard {
    // Keep this stable lock file open through socket cleanup. Never unlink it:
    // two processes locking different generations of that file would both win.
    _lock: File,
    socket_path: PathBuf,
    identity: SocketIdentity,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if socket_identity(&self.socket_path).is_ok_and(|identity| identity == self.identity) {
            let _ = fs::remove_file(&self.socket_path);
        }
    }
}

pub fn acquire() -> Result<Option<(InstanceGuard, UnixListener)>, String> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| "XDG_RUNTIME_DIR is not set".to_string())?;
    let directory = PathBuf::from(runtime).join("halley");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("create {}: {error}", directory.display()))?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("chmod {}: {error}", directory.display()))?;
    acquire_at(&directory.join("halley-lift.sock"))
}

fn toggle(path: &Path) -> io::Result<bool> {
    match UnixStream::connect(path) {
        Ok(_) => Ok(true),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

fn acquire_at(path: &Path) -> Result<Option<(InstanceGuard, UnixListener)>, String> {
    let lock_path = path.with_extension("lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&lock_path)
        .map_err(|error| format!("open {}: {error}", lock_path.display()))?;
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(TryLockError::Error(error)) => {
                return Err(format!("lock {}: {error}", lock_path.display()));
            }
            Err(TryLockError::WouldBlock) => {
                if toggle(path).map_err(|error| format!("toggle instance: {error}"))? {
                    return Ok(None);
                }
                // The owner may not have bound yet, or may have closed its
                // listener before dropping its guard. Never reclaim its path.
                if Instant::now() >= deadline {
                    return Err(
                        "another Lift instance is starting or shutting down; try again".into(),
                    );
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    for _ in 0..2 {
        match UnixListener::bind(path) {
            Ok(listener) => {
                let guard = InstanceGuard {
                    _lock: lock,
                    socket_path: path.to_owned(),
                    identity: socket_identity(path)
                        .map_err(|error| format!("inspect instance socket: {error}"))?,
                };
                listener
                    .set_nonblocking(true)
                    .map_err(|error| format!("set instance socket nonblocking: {error}"))?;
                return Ok(Some((guard, listener)));
            }
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                // Preserve toggling an older Lift that does not use the lock.
                if toggle(path).map_err(|error| format!("toggle instance: {error}"))? {
                    return Ok(None);
                }
                match socket_identity(path) {
                    Ok(_) => fs::remove_file(path)
                        .map_err(|error| format!("remove stale instance socket: {error}"))?,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!(
                            "inspect stale instance socket {}: {error}",
                            path.display()
                        ));
                    }
                }
            }
            Err(error) => return Err(format!("bind instance socket {}: {error}", path.display())),
        }
    }
    Err(format!(
        "bind instance socket {} after stale cleanup",
        path.display()
    ))
}
