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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
        mpsc,
    };

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "lift-instance-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn socket(&self) -> PathBuf {
            self.0.join("lift.sock")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn normal_shutdown_removes_its_socket_and_releases_the_stable_lock() {
        let scratch = Scratch::new();
        let path = scratch.socket();
        let (guard, listener) = acquire_at(&path).unwrap().unwrap();
        assert!(acquire_at(&path).unwrap().is_none());
        assert!(listener.accept().is_ok());
        drop(listener);
        drop(guard);
        assert!(!path.exists());
        assert!(path.with_extension("lock").exists());
        assert!(acquire_at(&path).unwrap().is_some());
    }

    #[test]
    fn old_guard_does_not_unlink_a_replacement_listener() {
        let scratch = Scratch::new();
        let path = scratch.socket();
        let (guard, _old_listener) = acquire_at(&path).unwrap().unwrap();
        fs::remove_file(&path).unwrap();
        let replacement = UnixListener::bind(&path).unwrap();
        replacement.set_nonblocking(true).unwrap();
        drop(guard);
        let _client = UnixStream::connect(&path).unwrap();
        assert!(replacement.accept().is_ok());
    }

    #[test]
    fn old_guard_preserves_a_replacement_regular_file() {
        let scratch = Scratch::new();
        let path = scratch.socket();
        let (guard, _listener) = acquire_at(&path).unwrap().unwrap();
        fs::remove_file(&path).unwrap();
        fs::write(&path, "replacement").unwrap();
        drop(guard);
        assert_eq!(fs::read_to_string(&path).unwrap(), "replacement");
    }

    #[test]
    fn stale_socket_is_recovered_but_a_regular_file_is_not_removed() {
        let scratch = Scratch::new();
        let path = scratch.socket();
        drop(UnixListener::bind(&path).unwrap());
        let (guard, listener) = acquire_at(&path).unwrap().unwrap();
        let _client = UnixStream::connect(&path).unwrap();
        assert!(listener.accept().is_ok());
        drop(listener);
        drop(guard);
        fs::write(&path, "keep me").unwrap();
        assert!(acquire_at(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "keep me");
    }

    #[test]
    fn live_legacy_listener_is_toggled_and_preserved() {
        let scratch = Scratch::new();
        let path = scratch.socket();
        let legacy = UnixListener::bind(&path).unwrap();
        legacy.set_nonblocking(true).unwrap();
        assert!(acquire_at(&path).unwrap().is_none());
        assert!(legacy.accept().is_ok());
        assert!(path.exists());
    }

    #[test]
    fn concurrent_starts_create_one_owner_and_one_toggle() {
        let scratch = Scratch::new();
        let barrier = Arc::new(Barrier::new(3));
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let path = scratch.socket();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    acquire_at(&path).unwrap()
                })
            })
            .collect();
        barrier.wait();
        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_some()).count(), 1);
        let owner = results.into_iter().flatten().next().unwrap();
        assert!(owner.1.accept().is_ok());
    }

    #[test]
    fn startup_with_lock_before_bind_does_not_reclaim_the_owners_path() {
        let scratch = Scratch::new();
        let path = scratch.socket();
        let lock = File::create(path.with_extension("lock")).unwrap();
        lock.lock().unwrap();
        let (done, result) = mpsc::channel();
        let contender_path = path.clone();
        let contender = thread::spawn(move || {
            done.send(acquire_at(&contender_path).unwrap().is_none())
                .unwrap();
        });
        assert!(matches!(
            result.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        assert!(result.recv_timeout(Duration::from_secs(2)).unwrap());
        contender.join().unwrap();
        assert!(listener.accept().is_ok());
        assert!(path.exists());
    }

    #[test]
    fn shutdown_with_closed_listener_waits_for_the_guard_before_takeover() {
        let scratch = Scratch::new();
        let path = scratch.socket();
        let (guard, listener) = acquire_at(&path).unwrap().unwrap();
        drop(listener);
        let (done, result) = mpsc::channel();
        let contender_path = path.clone();
        let contender = thread::spawn(move || {
            done.send(acquire_at(&contender_path)).unwrap();
        });
        assert!(matches!(
            result.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        assert!(path.exists());
        drop(guard);
        let (_new_guard, listener) = result
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap()
            .unwrap();
        contender.join().unwrap();
        let _client = UnixStream::connect(&path).unwrap();
        assert!(listener.accept().is_ok());
    }

    #[test]
    #[ignore = "subprocess helper for crash recovery"]
    fn crashed_owner_child() {
        let path = PathBuf::from(std::env::var_os("HALLEY_LIFT_INSTANCE_TEST_SOCKET").unwrap());
        let (_guard, _listener) = acquire_at(&path).unwrap().unwrap();
        fs::write(path.with_extension("ready"), "ready").unwrap();
        loop {
            thread::park();
        }
    }

    #[test]
    fn killed_owner_releases_its_lock_and_leaves_a_recoverable_socket() {
        let scratch = Scratch::new();
        let path = scratch.socket();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "instance::tests::crashed_owner_child",
            ])
            .env("HALLEY_LIFT_INSTANCE_TEST_SOCKET", &path)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !path.with_extension("ready").exists() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("child did not acquire the socket");
            }
            thread::sleep(Duration::from_millis(10));
        }
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(path.exists());
        let (_guard, listener) = acquire_at(&path).unwrap().unwrap();
        let _client = UnixStream::connect(&path).unwrap();
        assert!(listener.accept().is_ok());
    }
}
