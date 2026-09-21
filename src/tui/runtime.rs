//! Input ownership and frame scheduling for the message-driven UI thread.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle, Thread};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event};

const INPUT_POLL: Duration = Duration::from_millis(50);
const FRAME_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 60);

struct InputWakeOnDrop(Thread);

impl Drop for InputWakeOnDrop {
    fn drop(&mut self) {
        self.0.unpark();
    }
}

/// The sole reader of terminal input. Only the UI thread writes the terminal.
pub(super) struct InputWorker {
    receiver: Option<Receiver<io::Result<Event>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl InputWorker {
    pub fn start(wakeup: Thread) -> Self {
        Self::spawn(wakeup, || {
            if event::poll(INPUT_POLL)? {
                event::read().map(Some)
            } else {
                Ok(None)
            }
        })
    }

    fn spawn(
        wakeup: Thread,
        mut read: impl FnMut() -> io::Result<Option<Event>> + Send + 'static,
    ) -> Self {
        let (sender, receiver) = mpsc::sync_channel(128);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            let _wake = InputWakeOnDrop(wakeup.clone());
            let result_sender = sender;
            while !worker_stop.load(Ordering::Acquire) {
                let result = match read() {
                    Ok(None) => continue,
                    Ok(Some(event)) => Ok(event),
                    Err(error) => Err(error),
                };
                let failed = result.is_err();
                if result_sender.send(result).is_err() {
                    break;
                }
                wakeup.unpark();
                if failed {
                    break;
                }
            }
        });
        Self {
            receiver: Some(receiver),
            stop,
            thread: Some(worker),
        }
    }

    pub fn try_recv(&self) -> io::Result<Option<Event>> {
        match self.receiver.as_ref().unwrap().try_recv() {
            Ok(event) => event.map(Some),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err(io::Error::other("Terminal input worker stopped"))
            }
        }
    }
}

impl Drop for InputWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Unblock a sender waiting on a full queue before joining it.
        self.receiver.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) struct FrameSchedule {
    dirty: bool,
    next_frame: Instant,
}

impl FrameSchedule {
    pub fn new(now: Instant) -> Self {
        Self {
            dirty: true,
            next_frame: now,
        }
    }
    pub fn changed(&mut self, changed: bool) {
        self.dirty |= changed;
    }
    pub fn ready(&self, now: Instant) -> bool {
        self.dirty && now >= self.next_frame
    }
    pub fn rendered(&mut self, now: Instant) {
        self.dirty = false;
        self.next_frame = now + FRAME_INTERVAL;
    }
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.dirty
            .then(|| self.next_frame.saturating_duration_since(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_coalesced_and_idle_never_redraws() {
        let now = Instant::now();
        let mut schedule = FrameSchedule::new(now);
        assert!(schedule.ready(now));
        schedule.rendered(now);
        assert_eq!(schedule.wait(now + Duration::from_secs(1)), None);
        assert!(!schedule.ready(now + Duration::from_secs(1)));
        schedule.changed(true);
        schedule.changed(false);
        assert!(!schedule.ready(now));
        assert_eq!(schedule.wait(now), Some(FRAME_INTERVAL));
        assert!(schedule.ready(now + FRAME_INTERVAL));
    }

    #[test]
    fn input_errors_reach_the_ui() {
        let worker = InputWorker::spawn(thread::current(), || {
            Err(io::Error::other("injected input failure"))
        });
        let result = worker
            .receiver
            .as_ref()
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert_eq!(result.unwrap_err().to_string(), "injected input failure");
    }

    #[test]
    fn full_input_queue_does_not_block_shutdown() {
        let (sent, received) = mpsc::channel();
        let worker = InputWorker::spawn(thread::current(), move || {
            let _ = sent.send(());
            Ok(Some(Event::Resize(80, 24)))
        });
        // The next send will block because the bounded event queue is full.
        for _ in 0..129 {
            received.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        let (done, finished) = mpsc::channel();
        thread::spawn(move || {
            drop(worker);
            let _ = done.send(());
        });
        finished.recv_timeout(Duration::from_secs(2)).unwrap();
    }
}
