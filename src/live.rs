use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use halley_api::{Client, ClusterSummary, ConnectOptions, Event, EventTopic, NodeInfo};

const RETRY_DELAY: Duration = Duration::from_secs(1);
const FALLBACK_INTERVAL: Duration = Duration::from_millis(100);
const SUBSCRIPTION_RETRY_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, PartialEq)]
pub struct LiveSnapshot {
    pub nodes: Vec<NodeInfo>,
    pub clusters: Vec<ClusterSummary>,
    pub config_path: Option<PathBuf>,
}

pub struct LiveUpdates {
    pending: Arc<Mutex<Option<LiveSnapshot>>>,
    // Dropping this sender stops retries, without waiting on API I/O in the UI.
    _stop: mpsc::Sender<()>,
}

impl LiveUpdates {
    pub fn start(
        options: ConnectOptions,
        wake: Option<calloop::channel::Sender<()>>,
    ) -> io::Result<Self> {
        let pending = Arc::new(Mutex::new(None));
        let worker_pending = pending.clone();
        let (stop, stopped) = mpsc::channel();
        thread::Builder::new()
            .name("halley-lift-live".into())
            .spawn(move || {
                run(options, worker_pending, wake, stopped, RETRY_DELAY);
            })?;
        Ok(Self {
            pending,
            _stop: stop,
        })
    }

    pub fn poll_latest(&mut self) -> Option<LiveSnapshot> {
        self.pending.lock().unwrap().take()
    }
}

fn cancelled(stop: &mpsc::Receiver<()>) -> bool {
    !matches!(stop.try_recv(), Err(mpsc::TryRecvError::Empty))
}

fn publish(
    pending: &Mutex<Option<LiveSnapshot>>,
    wake: Option<&calloop::channel::Sender<()>>,
    snapshot: LiveSnapshot,
) {
    // Keep one complete latest snapshot rather than queuing stale intermediate
    // states while the UI is busy. One wake covers every update until it polls.
    let should_wake = pending.lock().unwrap().replace(snapshot).is_none();
    if should_wake && let Some(wake) = wake {
        let _ = wake.send(());
    }
}

fn run(
    options: ConnectOptions,
    pending: Arc<Mutex<Option<LiveSnapshot>>>,
    wake: Option<calloop::channel::Sender<()>>,
    stop: mpsc::Receiver<()>,
    retry_delay: Duration,
) {
    while !cancelled(&stop) {
        // All connection, snapshot and stream reads stay on this single worker.
        // A disconnect or sequence gap invalidates the stream: resubscribe for
        // a complete snapshot instead of applying more deltas to stale state.
        let _ = refresh(&options, &pending, wake.as_ref(), &stop);
        if !matches!(
            stop.recv_timeout(retry_delay),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            return;
        }
    }
}

fn refresh(
    options: &ConnectOptions,
    pending: &Mutex<Option<LiveSnapshot>>,
    wake: Option<&calloop::channel::Sender<()>>,
    stop: &mpsc::Receiver<()>,
) -> halley_api::Result<()> {
    let client = Client::connect_with(options.clone())?;
    if cancelled(stop) {
        return Ok(());
    }
    let config_path = client.config_path().ok().flatten();
    let mut previous_fallback = None;
    let mut subscription = loop {
        match client.subscribe([EventTopic::Nodes, EventTopic::Clusters]) {
            Ok(subscription) => break subscription,
            Err(_) => {
                // Older or differently-versioned snapshots may not support a
                // usable stream. Reuse the query connection for live titles,
                // rather than sampling their animation once per second.
                poll_fallback(
                    &client,
                    pending,
                    wake,
                    stop,
                    &mut previous_fallback,
                    config_path.clone(),
                )?;
            }
        }
        if cancelled(stop) {
            return Ok(());
        }
    };
    let config_path = subscription.initial.config_path.or(config_path);
    let mut nodes = subscription.initial.nodes;
    let mut clusters = subscription.initial.clusters;
    if cancelled(stop) {
        return Ok(());
    }
    publish(
        pending,
        wake,
        LiveSnapshot {
            nodes: nodes.clone(),
            clusters: clusters.clone(),
            config_path: config_path.clone(),
        },
    );
    loop {
        let event = subscription.events.next_event()?;
        if cancelled(stop) {
            return Ok(());
        }
        match event {
            Event::NodeAdded { node, .. } | Event::NodeChanged { node, .. } => {
                upsert(&mut nodes, node, |node| node.id);
            }
            Event::NodeRemoved { id, .. } => nodes.retain(|node| node.id != id),
            Event::ClusterAdded { cluster, .. } | Event::ClusterChanged { cluster, .. } => {
                upsert(&mut clusters, cluster, |cluster| cluster.id);
            }
            Event::ClusterRemoved { id, .. } => clusters.retain(|cluster| cluster.id != id),
            _ => continue,
        }
        publish(
            pending,
            wake,
            LiveSnapshot {
                nodes: nodes.clone(),
                clusters: clusters.clone(),
                config_path: config_path.clone(),
            },
        );
    }
}

fn poll_fallback(
    client: &Client,
    pending: &Mutex<Option<LiveSnapshot>>,
    wake: Option<&calloop::channel::Sender<()>>,
    stop: &mpsc::Receiver<()>,
    previous: &mut Option<LiveSnapshot>,
    config_path: Option<PathBuf>,
) -> halley_api::Result<()> {
    let retry_at = Instant::now() + SUBSCRIPTION_RETRY_INTERVAL;
    loop {
        if cancelled(stop) {
            return Ok(());
        }
        let snapshot = LiveSnapshot {
            nodes: client.nodes(None)?,
            clusters: client.clusters(None)?,
            config_path: config_path.clone(),
        };
        if cancelled(stop) {
            return Ok(());
        }
        // Quiet windows do not cause repaints. Keep the latest complete state
        // and preserve the last good cache if a later query disconnects.
        if previous.as_ref() != Some(&snapshot) {
            publish(pending, wake, snapshot.clone());
            *previous = Some(snapshot);
        }
        let wait = FALLBACK_INTERVAL.min(retry_at.saturating_duration_since(Instant::now()));
        if !matches!(
            stop.recv_timeout(wait),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            return Ok(());
        }
        if Instant::now() >= retry_at {
            return Ok(());
        }
    }
}

fn upsert<T, K: Eq>(values: &mut Vec<T>, value: T, key: impl Fn(&T) -> K) {
    let value_key = key(&value);
    if let Some(existing) = values
        .iter_mut()
        .find(|existing| key(existing) == value_key)
    {
        *existing = value;
    } else {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halley_api::test_support::{Request, Response};
    use halley_api::{
        ClusterLayout, Error, ErrorKind, HALLEY_API_VERSION, NodeKind, NodeProtocolFamily,
        NodeRole, NodeState, ServerInfo, Snapshot,
    };
    use std::fs;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let directory = std::env::temp_dir().join(format!(
                "lift-live-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(directory.join("halley")).unwrap();
            Self(directory)
        }

        fn socket(&self) -> PathBuf {
            self.0.join("halley/halley.sock")
        }

        fn options(&self) -> ConnectOptions {
            ConnectOptions {
                socket_path: Some(self.socket()),
                read_timeout: Some(Duration::from_secs(2)),
                write_timeout: Some(Duration::from_secs(2)),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn isolated(test: &str) -> bool {
        const MARKER: &str = "HALLEY_LIFT_TEST_LIVE_RUNTIME";
        if std::env::var_os(MARKER).is_some() {
            return false;
        }
        // The SDK also resolves its default path when given an explicit path.
        // Keep this environment requirement out of the parallel test process.
        let fixture = Fixture::new();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(MARKER, "1")
            .env("XDG_RUNTIME_DIR", &fixture.0)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "{test} failed");
                return true;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("{test} did not finish");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn request(stream: &UnixStream) -> Request {
        halley_api::test_support::request(stream).unwrap()
    }

    fn reply(stream: &UnixStream, response: Response) {
        halley_api::test_support::reply(stream, response).unwrap();
    }

    fn accept(listener: &UnixListener) -> UnixStream {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(3)))
                        .unwrap();
                    return stream;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "worker did not connect");
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("accept: {error}"),
            }
        }
    }

    fn subscribe(listener: &UnixListener) -> (UnixStream, UnixStream) {
        let control = accept(listener);
        assert!(matches!(request(&control), Request::Hello { .. }));
        reply(
            &control,
            Response::Hello(ServerInfo {
                compositor_version: "test".into(),
                api_version: HALLEY_API_VERSION,
                capabilities: Vec::new(),
            }),
        );
        assert_eq!(request(&control), Request::ConfigPath);
        reply(
            &control,
            Response::ConfigPath(Some("/custom/halley.rune".into())),
        );
        let events = accept(listener);
        let Request::Subscribe {
            api_version,
            topics,
        } = request(&events)
        else {
            panic!("expected subscription");
        };
        assert_eq!(api_version, HALLEY_API_VERSION);
        assert_eq!(topics, vec![EventTopic::Nodes, EventTopic::Clusters]);
        (control, events)
    }

    fn node(id: u64, title: &str) -> NodeInfo {
        NodeInfo {
            id: id.into(),
            title: title.into(),
            app_id: Some("test".into()),
            output: Some("DP-1".into()),
            kind: NodeKind::Surface,
            state: NodeState::Active,
            visible: true,
            focused: false,
            latest: false,
            pinned: false,
            role: NodeRole::NormalToplevel,
            protocol_family: NodeProtocolFamily::XdgToplevel,
            modal: false,
            parent: None,
            transient_for: None,
            child_popup_count: 0,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }
    }

    fn cluster(id: u64, name: &str) -> ClusterSummary {
        ClusterSummary {
            id: id.into(),
            slot: None,
            name: name.into(),
            output: "DP-1".into(),
            layout: ClusterLayout::Tiling,
            member_count: 1,
            active: true,
            focused: false,
        }
    }

    fn snapshot(sequence: u64, id: u64) -> Snapshot {
        Snapshot {
            sequence,
            outputs: Vec::new(),
            nodes: vec![node(id, "initial")],
            clusters: vec![cluster(id, "initial")],
            config_path: None,
        }
    }

    fn wait_snapshot(
        updates: &mut LiveUpdates,
        check: impl Fn(&LiveSnapshot) -> bool,
    ) -> LiveSnapshot {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(snapshot) = updates.poll_latest()
                && check(&snapshot)
            {
                return snapshot;
            }
            assert!(Instant::now() < deadline, "fresh snapshot did not arrive");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn recover(gap: bool) {
        let fixture = Fixture::new();
        let listener = UnixListener::bind(fixture.socket()).unwrap();
        let (advance, steps) = mpsc::channel();
        let server = thread::spawn(move || {
            let (control, events) = subscribe(&listener);
            reply(&events, Response::Subscribed(snapshot(10, 1)));
            steps.recv_timeout(Duration::from_secs(3)).unwrap();
            reply(
                &events,
                Response::Event(Event::NodeChanged {
                    sequence: 11,
                    node: node(1, "changed"),
                }),
            );
            reply(
                &events,
                Response::Event(Event::ClusterChanged {
                    sequence: 12,
                    cluster: cluster(1, "changed"),
                }),
            );
            steps.recv_timeout(Duration::from_secs(3)).unwrap();
            if gap {
                reply(
                    &events,
                    Response::Event(Event::NodeAdded {
                        sequence: 14,
                        node: node(999, "must not be applied"),
                    }),
                );
            }
            // Keep the broken stream open for the gap case. Reconnecting must
            // be caused by sequence validation, rather than a later EOF.
            let old_connection = if gap {
                Some((control, events))
            } else {
                drop(events);
                drop(control);
                None
            };
            let retry_started = Instant::now();
            let (_control, events) = subscribe(&listener);
            drop(old_connection);
            assert!(
                retry_started.elapsed() >= RETRY_DELAY / 2,
                "retry spun without a pause"
            );
            reply(&events, Response::Subscribed(snapshot(100, 2)));
            steps.recv_timeout(Duration::from_secs(3)).unwrap();
            reply(
                &events,
                Response::Event(Event::NodeRemoved {
                    sequence: 101,
                    id: 2.into(),
                }),
            );
            steps.recv_timeout(Duration::from_secs(3)).unwrap();
        });
        let mut updates = LiveUpdates::start(fixture.options(), None).unwrap();
        wait_snapshot(&mut updates, |s| {
            s.nodes.first().is_some_and(|n| n.id.get() == 1)
        });
        advance.send(()).unwrap();
        let changed = wait_snapshot(&mut updates, |s| {
            s.clusters.first().is_some_and(|c| c.name == "changed")
        });
        assert_eq!(changed.nodes[0].title, "changed");
        assert_eq!(
            changed.config_path.as_deref(),
            Some(std::path::Path::new("/custom/halley.rune"))
        );
        advance.send(()).unwrap();
        let fresh = wait_snapshot(&mut updates, |s| {
            assert!(
                !s.nodes.is_empty(),
                "reconnection cleared the last good cache"
            );
            assert!(s.nodes.iter().all(|node| node.id.get() != 999));
            s.nodes.first().is_some_and(|n| n.id.get() == 2)
        });
        assert_eq!(fresh.nodes.len(), 1);
        assert_eq!(fresh.clusters.len(), 1);
        assert_eq!(fresh.clusters[0].id.get(), 2);
        advance.send(()).unwrap();
        wait_snapshot(&mut updates, |s| {
            s.nodes.is_empty() && s.clusters[0].id.get() == 2
        });
        drop(updates);
        advance.send(()).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn disconnect_replaces_old_state_and_resumes_deltas() {
        if !isolated("live::tests::disconnect_replaces_old_state_and_resumes_deltas") {
            recover(false);
        }
    }

    #[test]
    fn sequence_gap_replaces_old_state_and_resumes_deltas() {
        if !isolated("live::tests::sequence_gap_replaces_old_state_and_resumes_deltas") {
            recover(true);
        }
    }

    #[test]
    fn failed_initial_connection_recovers_when_server_appears() {
        if isolated("live::tests::failed_initial_connection_recovers_when_server_appears") {
            return;
        }
        let fixture = Fixture::new();
        let mut updates = LiveUpdates::start(fixture.options(), None).unwrap();
        thread::sleep(Duration::from_millis(100));
        assert!(updates.poll_latest().is_none());
        let listener = UnixListener::bind(fixture.socket()).unwrap();
        let (release, blocked) = mpsc::channel();
        let server = thread::spawn(move || {
            let (_control, events) = subscribe(&listener);
            reply(&events, Response::Subscribed(snapshot(0, 7)));
            blocked.recv_timeout(Duration::from_secs(3)).unwrap();
        });
        wait_snapshot(&mut updates, |s| {
            s.nodes.first().is_some_and(|n| n.id.get() == 7)
        });
        drop(updates);
        release.send(()).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn unsupported_subscription_keeps_query_results_fresh() {
        if isolated("live::tests::unsupported_subscription_keeps_query_results_fresh") {
            return;
        }
        let fixture = Fixture::new();
        let listener = UnixListener::bind(fixture.socket()).unwrap();
        let (advance, steps) = mpsc::channel();
        let server = thread::spawn(move || {
            let (control, events) = subscribe(&listener);
            reply(
                &events,
                Response::Error(Error::new(ErrorKind::Unsupported, "no subscriptions")),
            );
            for id in [1, 2] {
                assert!(matches!(request(&control), Request::Nodes { .. }));
                reply(&control, Response::Nodes(vec![node(id, "fallback")]));
                assert!(matches!(request(&control), Request::Clusters { .. }));
                reply(&control, Response::Clusters(vec![cluster(id, "fallback")]));
                steps.recv_timeout(Duration::from_secs(3)).unwrap();
            }
        });
        let mut updates = LiveUpdates::start(fixture.options(), None).unwrap();
        wait_snapshot(&mut updates, |s| {
            s.nodes.first().is_some_and(|n| n.id.get() == 1)
        });
        advance.send(()).unwrap();
        let fresh = wait_snapshot(&mut updates, |s| {
            s.nodes.first().is_some_and(|n| n.id.get() == 2)
        });
        assert_eq!(fresh.clusters[0].id.get(), 2);
        drop(updates);
        advance.send(()).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn latest_snapshot_coalesces_updates_and_wakes_again_after_polling() {
        let pending = Arc::new(Mutex::new(None));
        let (stop, _stopped) = mpsc::channel();
        let mut updates = LiveUpdates {
            pending: pending.clone(),
            _stop: stop,
        };
        let (wake, events) = calloop::channel::channel();
        for id in [1, 2, 3] {
            publish(
                &pending,
                Some(&wake),
                LiveSnapshot {
                    nodes: vec![node(id, "latest")],
                    clusters: Vec::new(),
                    config_path: None,
                },
            );
        }
        assert_eq!(updates.poll_latest().unwrap().nodes[0].id.get(), 3);
        assert!(events.try_recv().is_ok());
        assert!(events.try_recv().is_err());
        publish(
            &pending,
            Some(&wake),
            LiveSnapshot {
                nodes: Vec::new(),
                clusters: Vec::new(),
                config_path: None,
            },
        );
        assert!(events.try_recv().is_ok());
    }

    #[test]
    fn dropping_owner_interrupts_retry_wait() {
        if isolated("live::tests::dropping_owner_interrupts_retry_wait") {
            return;
        }
        let fixture = Fixture::new();
        let pending = Arc::new(Mutex::new(None));
        let (stop, stopped) = mpsc::channel();
        let (finished, done) = mpsc::channel();
        let worker = thread::spawn(move || {
            run(
                fixture.options(),
                pending,
                None,
                stopped,
                Duration::from_secs(30),
            );
            finished.send(()).unwrap();
        });
        thread::sleep(Duration::from_millis(50));
        drop(stop);
        done.recv_timeout(Duration::from_millis(250)).unwrap();
        worker.join().unwrap();
    }
}
