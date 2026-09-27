//! The queue that video imports wait in (see [`crate::video`]). Watching a video is heavy:
//! the download, ffmpeg, whisper.cpp and the frames for Wee Chef take ~400 MB at their peak.
//! So a fixed number of workers (`VIDEO_WORKERS`) take jobs from one queue shared by every
//! household, at most `VIDEO_QUEUE_MAX` more may wait, and past that an import is refused
//! straight away (429). Peak memory is then about the base plus workers × ~400 MB.
//!
//! The Add box gets a job id at once and polls it (`GET /api/import/jobs/{id}`); MCP awaits
//! the same job. The heavy part of a job holds a permit of [`VideoJobs::heavy`], which the
//! headless Chromium fallback shares, so the two never stack beyond the budget.

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use sentry::SentryFutureExt;
use serde_json::{Value, json};
use tokio::sync::{Notify, Semaphore, watch};

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::households::HouseholdId;
use crate::model::Recipe;

/// A job still waiting after this gives up.
pub const WAIT_LIMIT: Duration = Duration::from_secs(10 * 60);
/// How long a finished job can still be asked about.
const KEEP_FINISHED: Duration = Duration::from_secs(15 * 60);

const FULL: &str = "Wee Chef is watching a few videos already. Try again in a minute.";
const WAITED_TOO_LONG: &str =
    "Wee Chef couldn't get to that video in time. Try again in a few minutes.";

/// What a job gives: the recipe, and whether it's new.
pub type Outcome = AppResult<(Recipe, bool)>;
type Run =
    Arc<dyn Fn(AppState, String) -> Pin<Box<dyn Future<Output = Outcome> + Send>> + Send + Sync>;

/// The settings the queue runs with.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub workers: usize,
    pub queue_max: usize,
    pub wait_limit: Duration,
}

impl Limits {
    pub fn from_config(config: &crate::config::Config) -> Self {
        Self {
            workers: config.video_workers.max(1),
            queue_max: config.video_queue_max,
            wait_limit: WAIT_LIMIT,
        }
    }
}

/// Where a job is.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// Waiting for a worker; `position` 1 is next.
    Queued {
        position: usize,
    },
    Running,
    Done {
        id: i64,
        title: String,
        is_new: bool,
    },
    Failed(AppError),
}

impl Status {
    /// As `GET /api/import/jobs/{id}` returns it.
    pub fn to_json(&self, job: &str) -> Value {
        match self {
            Status::Queued { position } => {
                json!({"id": job, "status": "queued", "position": position})
            }
            Status::Running => json!({"id": job, "status": "running"}),
            Status::Done { id, title, is_new } => json!({
                "id": job,
                "status": "done",
                "recipe": {"id": id, "title": title, "isNew": is_new},
            }),
            Status::Failed(err) => json!({
                "id": job,
                "status": "failed",
                "statusCode": err.status.as_u16(),
                "message": err.message,
            }),
        }
    }
}

struct Job {
    household: HouseholdId,
    url: String,
    /// The household's state, until a worker takes the job.
    state: Option<AppState>,
    queued_at: Instant,
    running: bool,
    finished_at: Option<Instant>,
    result: watch::Sender<Option<Outcome>>,
    /// The request's Sentry span, so the job's transaction joins its trace.
    parent: Option<sentry::TransactionOrSpan>,
}

#[derive(Default)]
struct Inner {
    waiting: VecDeque<String>,
    jobs: HashMap<String, Job>,
    running: usize,
}

/// Video imports: the queue, its workers and the heavy-work budget.
pub struct VideoJobs {
    limits: Limits,
    run: Run,
    inner: Mutex<Inner>,
    ready: Notify,
    started: OnceLock<()>,
    /// One permit per worker, for the heavy part of a job and for headless Chromium.
    pub heavy: Arc<Semaphore>,
}

impl VideoJobs {
    pub fn new(limits: Limits) -> Self {
        Self::with_runner(limits, |state, url| {
            Box::pin(async move { crate::video::import(&state, &url).await })
        })
    }

    /// A queue whose jobs run `run` (tests stand in for the video pipeline).
    pub fn with_runner<F>(limits: Limits, run: F) -> Self
    where
        F: Fn(AppState, String) -> Pin<Box<dyn Future<Output = Outcome> + Send>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            heavy: Arc::new(Semaphore::new(limits.workers)),
            limits,
            run: Arc::new(run),
            inner: Mutex::default(),
            ready: Notify::new(),
            started: OnceLock::new(),
        }
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Queues a video for `state`'s household. The same video already queued or running
    /// for that household gives its job; a full queue is refused (429).
    pub fn submit(self: &Arc<Self>, state: &AppState, url: &str) -> AppResult<Ticket> {
        self.start_workers();
        let host = crate::telemetry::host_of(url);
        let mut inner = self.lock();
        self.sweep(&mut inner);
        if let Some((id, job)) = inner.jobs.iter().find(|(_, j)| {
            j.household == state.household && j.url == url && j.finished_at.is_none()
        }) {
            return Ok(Ticket {
                id: id.clone(),
                jobs: self.clone(),
                household: job.household,
                result: job.result.subscribe(),
            });
        }
        if inner.waiting.len() >= self.limits.queue_max {
            tracing::warn!(
                "[video] {host}: refused, the queue is full ({} running, {} waiting)",
                inner.running,
                inner.waiting.len()
            );
            return Err(AppError::new(429, FULL));
        }
        let id = format!("{:016x}", rand::random::<u64>());
        let (result, rx) = watch::channel(None);
        inner.jobs.insert(
            id.clone(),
            Job {
                household: state.household,
                url: url.to_string(),
                state: Some(state.clone()),
                queued_at: Instant::now(),
                running: false,
                finished_at: None,
                result,
                parent: sentry::configure_scope(|scope| scope.get_span()),
            },
        );
        inner.waiting.push_back(id.clone());
        tracing::info!(
            "[video] {host}: queued ({} running, {} waiting)",
            inner.running,
            inner.waiting.len()
        );
        drop(inner);
        self.ready.notify_one();
        Ok(Ticket {
            id,
            jobs: self.clone(),
            household: state.household,
            result: rx,
        })
    }

    /// Where household `household`'s job `id` is; None for an unknown one (or another
    /// household's, or one finished long ago).
    pub fn status(&self, household: HouseholdId, id: &str) -> Option<Status> {
        let mut inner = self.lock();
        self.sweep(&mut inner);
        let job = inner.jobs.get(id).filter(|j| j.household == household)?;
        if let Some(outcome) = job.result.borrow().as_ref() {
            return Some(match outcome {
                Ok((recipe, is_new)) => Status::Done {
                    id: recipe.id,
                    title: recipe.title.clone(),
                    is_new: *is_new,
                },
                Err(err) => Status::Failed(err.clone()),
            });
        }
        if job.running {
            return Some(Status::Running);
        }
        let position = inner
            .waiting
            .iter()
            .position(|w| w == id)
            .map_or(1, |p| p + 1);
        Some(Status::Queued { position })
    }

    /// Gives up on jobs that waited too long and forgets finished ones nobody asked about.
    fn sweep(&self, inner: &mut Inner) {
        let limit = self.limits.wait_limit;
        let expired: Vec<String> = inner
            .waiting
            .iter()
            .filter(|id| {
                inner
                    .jobs
                    .get(*id)
                    .is_some_and(|j| j.queued_at.elapsed() >= limit)
            })
            .cloned()
            .collect();
        for id in expired {
            inner.waiting.retain(|w| *w != id);
            if let Some(job) = inner.jobs.get_mut(&id) {
                tracing::warn!(
                    "[video] {}: gave up after waiting {} s",
                    crate::telemetry::host_of(&job.url),
                    job.queued_at.elapsed().as_secs()
                );
                job.state = None;
                job.finished_at = Some(Instant::now());
                job.result
                    .send_replace(Some(Err(AppError::new(503, WAITED_TOO_LONG))));
            }
        }
        inner
            .jobs
            .retain(|_, j| j.finished_at.is_none_or(|t| t.elapsed() < KEEP_FINISHED));
    }

    fn start_workers(self: &Arc<Self>) {
        self.started.get_or_init(|| {
            for _ in 0..self.limits.workers {
                tokio::spawn(self.clone().work());
            }
        });
    }

    async fn work(self: Arc<Self>) {
        loop {
            let (id, state, url, waited, depth, parent) = self.next().await;
            let host = crate::telemetry::host_of(&url);
            tracing::info!(
                "[video] {host}: started after waiting {} ms ({depth} still waiting)",
                waited.as_millis()
            );
            let span = job_span(parent, &id, waited, depth, self.limits.workers);
            let started = Instant::now();
            let hub = Arc::new(sentry::Hub::new_from_top(sentry::Hub::current()));
            if let Some(span) = &span {
                let span = span.clone();
                hub.configure_scope(|scope| scope.set_span(Some(span)));
            }
            // Its own task, so a panic fails the job rather than the worker
            let outcome = tokio::spawn((self.run)(state, url).bind_hub(hub))
                .await
                .unwrap_or_else(|err| Err(AppError::internal(format!("video job: {err}"))));
            let took = started.elapsed();
            match &outcome {
                Ok(_) => tracing::info!("[video] {host}: done in {} ms", took.as_millis()),
                Err(err) => tracing::info!(
                    "[video] {host}: failed in {} ms ({})",
                    took.as_millis(),
                    err.status
                ),
            }
            if let Some(span) = span {
                span.set_data("video.processing_ms", (took.as_millis() as u64).into());
                span.set_status(match &outcome {
                    Ok(_) => sentry::protocol::SpanStatus::Ok,
                    Err(err) if err.status.is_client_error() => {
                        sentry::protocol::SpanStatus::InvalidArgument
                    }
                    Err(_) => sentry::protocol::SpanStatus::InternalError,
                });
                span.finish();
            }
            let mut inner = self.lock();
            inner.running -= 1;
            if let Some(job) = inner.jobs.get_mut(&id) {
                job.finished_at = Some(Instant::now());
                job.result.send_replace(Some(outcome));
            }
        }
    }

    /// The next job, marked running: its id, state, link, wait, the queue left behind it
    /// and the span it was queued under.
    #[allow(clippy::type_complexity)]
    async fn next(
        &self,
    ) -> (
        String,
        AppState,
        String,
        Duration,
        usize,
        Option<sentry::TransactionOrSpan>,
    ) {
        loop {
            // Registered before looking, so a job queued in between still wakes this worker
            let notified = self.ready.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let mut inner = self.lock();
                self.sweep(&mut inner);
                while let Some(id) = inner.waiting.pop_front() {
                    let depth = inner.waiting.len();
                    let Some(job) = inner.jobs.get_mut(&id) else {
                        continue;
                    };
                    let Some(state) = job.state.take() else {
                        continue;
                    };
                    job.running = true;
                    let taken = (
                        id,
                        state,
                        job.url.clone(),
                        job.queued_at.elapsed(),
                        depth,
                        job.parent.take(),
                    );
                    inner.running += 1;
                    return taken;
                }
            }
            notified.await;
        }
    }
}

/// The job as a Sentry `queue.process` transaction (in the trace of the request that
/// queued it), carrying its wait and the queue's depth. None without Sentry.
fn job_span(
    parent: Option<sentry::TransactionOrSpan>,
    id: &str,
    waited: Duration,
    depth: usize,
    workers: usize,
) -> Option<sentry::TransactionOrSpan> {
    sentry::Hub::current().client()?;
    let ctx =
        sentry::TransactionContext::continue_from_span("video import", "queue.process", parent);
    let span: sentry::TransactionOrSpan = sentry::start_transaction(ctx).into();
    span.set_data("messaging.system", "crumb".into());
    span.set_data("messaging.destination.name", "video".into());
    span.set_data("messaging.message.id", id.into());
    span.set_data(
        "messaging.message.receive.latency",
        (waited.as_millis() as u64).into(),
    );
    span.set_data("video.queue_depth", depth.into());
    span.set_data("video.workers", workers.into());
    Some(span)
}

/// A queued job: its id (for polling) and a way to wait for it.
pub struct Ticket {
    pub id: String,
    jobs: Arc<VideoJobs>,
    household: HouseholdId,
    result: watch::Receiver<Option<Outcome>>,
}

impl Ticket {
    /// Where the job is now.
    pub fn status(&self) -> Status {
        self.jobs
            .status(self.household, &self.id)
            .unwrap_or(Status::Failed(AppError::new(
                404,
                "That import has expired",
            )))
    }

    /// Waits for the job to finish (or to give up waiting for a worker).
    pub async fn wait(mut self) -> Outcome {
        let deadline = tokio::time::Instant::now() + self.jobs.limits.wait_limit;
        let mut swept = false;
        loop {
            tokio::select! {
                done = self.result.wait_for(Option::is_some) => {
                    return match done {
                        Ok(v) => (*v).clone().unwrap_or_else(|| Err(AppError::internal("video job lost"))),
                        Err(_) => Err(AppError::internal("video job lost")),
                    };
                }
                _ = tokio::time::sleep_until(deadline), if !swept => {
                    // Fails the job if it's still waiting; a running one is let finish
                    let mut inner = self.jobs.lock();
                    self.jobs.sweep(&mut inner);
                    swept = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::Browser;
    use crate::config::Config;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn state() -> AppState {
        AppState::new(
            crate::db::open_in_memory().unwrap(),
            Config::default(),
            Browser::disabled(),
        )
    }

    fn recipe(state: &AppState, title: &str) -> Outcome {
        let fields = crate::model::RecipeFields {
            title: title.into(),
            ingredients: vec![crate::model::Section::unnamed(vec!["1 egg".into()])],
            ..Default::default()
        };
        crate::recipes::create_recipe(&state.db.lock(), fields, "video")
    }

    /// Jobs that take `ms` each and count how many run at once.
    fn slow_jobs(limits: Limits, ms: u64) -> (Arc<VideoJobs>, Arc<AtomicUsize>) {
        let most = Arc::new(AtomicUsize::new(0));
        let now = Arc::new(AtomicUsize::new(0));
        let (m, n) = (most.clone(), now.clone());
        let jobs = VideoJobs::with_runner(limits, move |state, url| {
            let (m, n) = (m.clone(), n.clone());
            Box::pin(async move {
                let running = n.fetch_add(1, Ordering::SeqCst) + 1;
                m.fetch_max(running, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(ms)).await;
                n.fetch_sub(1, Ordering::SeqCst);
                recipe(&state, &url)
            })
        });
        (Arc::new(jobs), most)
    }

    fn limits(workers: usize, queue_max: usize) -> Limits {
        Limits {
            workers,
            queue_max,
            wait_limit: WAIT_LIMIT,
        }
    }

    #[tokio::test]
    async fn runs_as_many_at_once_as_there_are_workers() {
        let (jobs, most) = slow_jobs(limits(2, 8), 150);
        let s = state();
        let started = Instant::now();
        let a = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/1")
            .unwrap();
        let b = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/2")
            .unwrap();
        let (a, b) = tokio::join!(a.wait(), b.wait());
        assert!(a.is_ok() && b.is_ok());
        assert_eq!(most.load(Ordering::SeqCst), 2);
        assert!(
            started.elapsed() < Duration::from_millis(290),
            "ran side by side"
        );
    }

    #[tokio::test]
    async fn one_worker_takes_them_in_turn_with_positions() {
        let (jobs, most) = slow_jobs(limits(1, 4), 80);
        let s = state();
        let a = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/1")
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let b = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/2")
            .unwrap();
        let c = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/3")
            .unwrap();
        assert_eq!(a.status(), Status::Running);
        assert_eq!(b.status(), Status::Queued { position: 1 });
        assert_eq!(c.status(), Status::Queued { position: 2 });
        let id = c.id.clone();
        let (_, _, c) = tokio::join!(a.wait(), b.wait(), c.wait());
        assert_eq!(most.load(Ordering::SeqCst), 1);
        let (saved, is_new) = c.unwrap();
        assert!(is_new);
        assert_eq!(
            jobs.status(s.household, &id),
            Some(Status::Done {
                id: saved.id,
                title: saved.title,
                is_new: true
            })
        );
    }

    #[tokio::test]
    async fn a_full_queue_refuses_and_the_rest_finish() {
        let (jobs, _) = slow_jobs(limits(1, 2), 60);
        let s = state();
        let mut queued = vec![
            jobs.submit(&s, "https://www.tiktok.com/@a/video/1")
                .unwrap(),
        ];
        // The worker takes the first, then two may wait
        tokio::time::sleep(Duration::from_millis(10)).await;
        for i in 2..=3 {
            queued.push(
                jobs.submit(&s, &format!("https://www.tiktok.com/@a/video/{i}"))
                    .unwrap(),
            );
        }
        let refused = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/9")
            .err()
            .unwrap();
        assert_eq!(refused.status.as_u16(), 429);
        assert!(refused.message.contains("Wee Chef"));
        for t in queued {
            assert!(t.wait().await.is_ok());
        }
    }

    #[tokio::test]
    async fn the_same_video_twice_is_one_job() {
        let (jobs, _) = slow_jobs(limits(1, 4), 30);
        let s = state();
        let a = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/1")
            .unwrap();
        let b = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/1")
            .unwrap();
        assert_eq!(a.id, b.id);
    }

    #[tokio::test]
    async fn jobs_belong_to_their_household() {
        let (jobs, _) = slow_jobs(limits(1, 4), 30);
        let home = state();
        let other = home.for_household(2).unwrap();
        let t = jobs
            .submit(&other, "https://www.tiktok.com/@a/video/1")
            .unwrap();
        assert!(jobs.status(home.household, &t.id).is_none());
        assert!(jobs.status(2, &t.id).is_some());
        let (saved, _) = t.wait().await.unwrap();
        // Saved in household 2's box, not the home one
        assert!(
            crate::recipes::get_recipe(&other.db.lock(), saved.id)
                .unwrap()
                .is_some()
        );
        let home_count: i64 = home
            .db
            .lock()
            .query_row("SELECT count(*) FROM recipes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(home_count, 0);
    }

    #[tokio::test]
    async fn a_job_waiting_too_long_gives_up() {
        let (jobs, _) = slow_jobs(
            Limits {
                workers: 1,
                queue_max: 4,
                wait_limit: Duration::from_millis(40),
            },
            150,
        );
        let s = state();
        let first = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/1")
            .unwrap();
        let second = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/2")
            .unwrap();
        let err = second.wait().await.err().unwrap();
        assert_eq!(err.message, WAITED_TOO_LONG);
        assert!(first.wait().await.is_ok(), "the running one still finishes");
    }

    #[tokio::test]
    async fn a_panicking_job_fails_without_stopping_its_worker() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let jobs = Arc::new(VideoJobs::with_runner(limits(1, 4), move |state, url| {
            let first = c.fetch_add(1, Ordering::SeqCst) == 0;
            Box::pin(async move {
                if first {
                    panic!("boom");
                }
                recipe(&state, &url)
            })
        }));
        let s = state();
        let a = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/1")
            .unwrap();
        assert_eq!(a.wait().await.err().unwrap().status.as_u16(), 500);
        let b = jobs
            .submit(&s, "https://www.tiktok.com/@a/video/2")
            .unwrap();
        assert!(b.wait().await.is_ok());
    }

    #[test]
    fn status_json_for_the_add_box() {
        assert_eq!(
            Status::Queued { position: 2 }.to_json("ab"),
            json!({"id": "ab", "status": "queued", "position": 2})
        );
        assert_eq!(
            Status::Failed(AppError::new(422, "No recipe")).to_json("ab")["message"],
            "No recipe"
        );
    }
}
