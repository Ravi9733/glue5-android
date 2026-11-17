//! Compile threads: shader modules and pipelines are built off the command
//! thread (a pipeline compile takes milliseconds; hundreds arrive when a new
//! area streams in). Urgent jobs (something is waiting) run before
//! background ones (objects to free).

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

type Job = Box<dyn FnOnce() + Send>;

#[derive(Default)]
struct Queues {
    urgent: VecDeque<Job>,
    background: VecDeque<Job>,
}

pub struct Pool {
    queues: Arc<(Mutex<Queues>, Condvar)>,
}

impl Pool {
    pub fn new(name: &str, threads: usize) -> Pool {
        let thread_qos = if name == "gpu decode" { glue5_runtime::qos::Qos::Utility } else { glue5_runtime::qos::Qos::Initiated };
        let queues: Arc<(Mutex<Queues>, Condvar)> = Arc::default();
        for i in 0..threads {
            let q = queues.clone();
            std::thread::Builder::new()
                .name(format!("{name} {i}"))
                .spawn(move || {
                    glue5_runtime::qos::set(thread_qos);
                    loop {
                        let job = {
                            let (lock, cv) = &*q;
                            let mut queues = lock.lock().unwrap();
                            loop {
                                if let Some(job) = queues.urgent.pop_front().or_else(|| queues.background.pop_front()) {
                                    cv.notify_all();
                                    break job;
                                }
                                queues = cv.wait(queues).unwrap();
                            }
                        };
                        job();
                    }
                })
                .expect("spawning a compile thread");
        }
        Pool { queues }
    }

    pub fn urgent(&self, job: impl FnOnce() + Send + 'static) {
        let (lock, cv) = &*self.queues;
        lock.lock().unwrap().urgent.push_back(Box::new(job));
        cv.notify_one();
    }

    /// Queue urgent work with backpressure. Useful for texture decoding:
    /// each queued closure owns its compressed source bytes, so an unbounded
    /// queue can grow by hundreds of MiB while the GPU command thread streams.
    pub fn urgent_bounded(&self, job: impl FnOnce() + Send + 'static, max_queued: usize) {
        let (lock, cv) = &*self.queues;
        let mut queues = lock.lock().unwrap();
        while queues.urgent.len() >= max_queued.max(1) {
            queues = cv.wait(queues).unwrap();
        }
        queues.urgent.push_back(Box::new(job));
        cv.notify_one();
    }


    /// Try to queue urgent work without ever waiting for a worker to free a
    /// slot. On saturation, return ownership of the job to the caller so it
    /// can defer admission and retry from a non-render thread / later tick.
    ///
    /// This is intentionally separate from `urgent_bounded`: callers that
    /// cannot safely defer work may continue to use the blocking API.
    pub fn try_urgent_bounded(
        &self,
        job: impl FnOnce() + Send + 'static,
        max_queued: usize,
    ) -> Result<(), Box<dyn FnOnce() + Send + 'static>> {
        let (lock, cv) = &*self.queues;
        let mut queues = lock.lock().unwrap();
        if queues.urgent.len() >= max_queued.max(1) {
            return Err(Box::new(job));
        }
        queues.urgent.push_back(Box::new(job));
        cv.notify_one();
        Ok(())
    }

    pub fn background(&self, job: impl FnOnce() + Send + 'static) {
        let (lock, cv) = &*self.queues;
        lock.lock().unwrap().background.push_back(Box::new(job));
        cv.notify_one();
    }
}


#[cfg(test)]
mod tests {
    use super::Pool;
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

    #[test]
    fn try_urgent_bounded_returns_job_when_full() {
        let pool = Pool::new("test", 0);
        pool.urgent(|| {});
        let returned = pool.try_urgent_bounded(|| {}, 1);
        assert!(returned.is_err());
    }

    #[test]
    fn try_urgent_bounded_accepts_job_below_limit() {
        let pool = Pool::new("test", 0);
        let ran = Arc::new(AtomicBool::new(false));
        let marker = ran.clone();
        assert!(pool.try_urgent_bounded(move || marker.store(true, Ordering::Relaxed), 1).is_ok());
        let (lock, _) = &*pool.queues;
        let job = lock.lock().unwrap().urgent.pop_front().unwrap();
        job();
        assert!(ran.load(Ordering::Relaxed));
    }
}
