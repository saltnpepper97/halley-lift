use std::io;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use halley_api::{Client, ClusterSummary, ConnectOptions, Event, EventTopic, NodeInfo};

const RETRY_DELAY: Duration = Duration::from_secs(1);

pub struct LiveSnapshot {
    pub nodes: Vec<NodeInfo>,
    pub clusters: Vec<ClusterSummary>,
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
    let mut subscription = match client.subscribe([EventTopic::Nodes, EventTopic::Clusters]) {
        Ok(subscription) => subscription,
        Err(_) => {
            // Older servers may not offer subscriptions. Keep refreshing their
            // query results as well; failed requests leave the last good cache.
            let snapshot = LiveSnapshot {
                nodes: client.nodes(None)?,
                clusters: client.clusters(None)?,
            };
            if !cancelled(stop) {
                publish(pending, wake, snapshot);
            }
            return Ok(());
        }
    };
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
            },
        );
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
