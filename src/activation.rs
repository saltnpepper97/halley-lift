use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use crate::providers::Activation;

pub struct Completion {
    pub result: Result<(), String>,
    pub exit_reason: &'static str,
}

pub struct ActivationWorker {
    pending: Option<(Receiver<Completion>, &'static str)>,
    wake: calloop::channel::Sender<()>,
}

impl ActivationWorker {
    pub fn new(wake: calloop::channel::Sender<()>) -> Self {
        Self {
            pending: None,
            wake,
        }
    }

    pub fn start(&mut self, action: Activation, exit_reason: &'static str) -> Result<bool, String> {
        self.start_job(exit_reason, move || action.execute())
    }

    fn start_job(
        &mut self,
        exit_reason: &'static str,
        job: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) -> Result<bool, String> {
        if self.pending.is_some() {
            return Ok(false);
        }
        let (tx, rx) = mpsc::channel();
        let wake = self.wake.clone();
        thread::Builder::new()
            .name("halley-lift-action".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(job))
                    .unwrap_or_else(|_| Err("Launcher action worker failed".into()));
                let _ = tx.send(Completion {
                    result,
                    exit_reason,
                });
                let _ = wake.send(());
            })
            .map_err(|error| format!("start action worker: {error}"))?;
        self.pending = Some((rx, exit_reason));
        Ok(true)
    }

    pub fn poll(&mut self) -> Option<Completion> {
        let (rx, exit_reason) = self.pending.as_ref()?;
        let completion = match rx.try_recv() {
            Ok(completion) => completion,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Completion {
                result: Err("Launcher action worker disconnected".into()),
                exit_reason,
            },
        };
        self.pending = None;
        Some(completion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn completion(worker: &mut ActivationWorker) -> Completion {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(completion) = worker.poll() {
                return completion;
            }
            assert!(Instant::now() < deadline, "worker did not finish");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn stalled_action_does_not_block_the_caller_or_run_twice() {
        let (wake, _events) = calloop::channel::channel();
        let mut worker = ActivationWorker::new(wake);
        let (release, blocked) = mpsc::channel();
        assert!(
            worker
                .start_job("activate", move || {
                    blocked.recv_timeout(Duration::from_secs(2)).unwrap();
                    Ok(())
                })
                .unwrap()
        );
        // The job has not been released: start() must already have returned.
        assert!(worker.poll().is_none());
        assert!(
            !worker
                .start_job("duplicate", || panic!("duplicate action ran"))
                .unwrap()
        );
        release.send(()).unwrap();
        let done = completion(&mut worker);
        assert!(done.result.is_ok());
        assert_eq!(done.exit_reason, "activate");
        assert!(worker.poll().is_none());
    }

    #[test]
    fn failed_worker_releases_the_pending_slot_and_wakes_the_event_loop() {
        let (wake, events) = calloop::channel::channel();
        let mut worker = ActivationWorker::new(wake);
        assert!(
            worker
                .start_job("activate", || Err("API timed out".into()))
                .unwrap()
        );
        assert_eq!(completion(&mut worker).result.unwrap_err(), "API timed out");
        let deadline = Instant::now() + Duration::from_secs(2);
        while events.try_recv().is_err() {
            assert!(
                Instant::now() < deadline,
                "worker did not wake the event loop"
            );
            thread::sleep(Duration::from_millis(5));
        }
        assert!(worker.start_job("cluster-draft", || Ok(())).unwrap());
        let done = completion(&mut worker);
        assert_eq!(done.exit_reason, "cluster-draft");
        assert!(done.result.is_ok());
    }

    #[test]
    fn panicking_action_does_not_leave_the_worker_pending() {
        let (wake, _events) = calloop::channel::channel();
        let mut worker = ActivationWorker::new(wake);
        worker
            .start_job("activate", || panic!("failed action"))
            .unwrap();
        assert!(completion(&mut worker).result.is_err());
        assert!(worker.start_job("activate", || Ok(())).unwrap());
        assert!(completion(&mut worker).result.is_ok());
    }
}
