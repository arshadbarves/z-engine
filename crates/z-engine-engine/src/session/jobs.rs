//! Background shells of one session: the host registry plus the GUI's job
//! list. Output updates are throttled per job; an exit always emits and
//! queues a "job finished" reminder for the agent that started the job.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use regex::Regex;
use z_engine_host::{BackgroundShells, BackgroundSpec, HostError, JobEvent, JobRead, JobSnapshot};
use z_engine_protocol::{AgentId, Event, JobId, JobInfo, JobKind};

use crate::session::{Emitter, ReminderBox};
use crate::sync::lock;

const OUTPUT_UPDATE_INTERVAL: Duration = Duration::from_millis(250);

pub(crate) struct JobHub {
    shells: BackgroundShells,
    events: Arc<Emitter>,
    reminders: Arc<ReminderBox>,
    last_update: Mutex<BTreeMap<JobId, Instant>>,
}

impl std::fmt::Debug for JobHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHub")
            .field("shells", &self.shells)
            .finish_non_exhaustive()
    }
}

impl JobHub {
    /// The host registry reports through a weak handle, so dropping the
    /// hub drops the registry and kills whatever still runs.
    pub(crate) fn new(events: Arc<Emitter>, reminders: Arc<ReminderBox>) -> Arc<Self> {
        Arc::new_cyclic(|hub: &Weak<JobHub>| {
            let hub = hub.clone();
            let sink = Arc::new(move |event: JobEvent| {
                if let Some(hub) = hub.upgrade() {
                    hub.on_event(event);
                }
            });
            Self {
                shells: BackgroundShells::new(Some(sink)),
                events,
                reminders,
                last_update: Mutex::new(BTreeMap::new()),
            }
        })
    }

    pub(crate) async fn spawn(&self, spec: BackgroundSpec) -> Result<JobInfo, HostError> {
        let id = self.shells.spawn(spec).await?;
        let info = self
            .info(&id)
            .ok_or_else(|| HostError::NotFound(format!("background job {id}")))?;
        self.events.emit(Event::JobUpdated { job: info.clone() });
        Ok(info)
    }

    pub(crate) fn read(&self, id: &JobId, filter: Option<&Regex>) -> Result<JobRead, HostError> {
        self.shells.read_new(id, filter)
    }

    pub(crate) async fn wait(&self, id: &JobId, timeout: Duration) -> Result<JobRead, HostError> {
        self.shells.wait(id, timeout).await
    }

    pub(crate) async fn kill(&self, id: &JobId) -> Result<(), HostError> {
        self.shells.kill(id).await
    }

    pub(crate) async fn kill_all(&self) {
        self.shells.kill_all().await;
    }

    pub(crate) fn info(&self, id: &JobId) -> Option<JobInfo> {
        self.shells.snapshot(id).as_ref().map(info_of)
    }

    pub(crate) fn list(&self) -> Vec<JobInfo> {
        self.shells.list().iter().map(info_of).collect()
    }

    fn on_event(&self, event: JobEvent) {
        match event {
            JobEvent::Output { id, .. } => {
                let now = Instant::now();
                let due = {
                    let mut last = lock(&self.last_update);
                    let due = last
                        .get(&id)
                        .is_none_or(|at| now.duration_since(*at) >= OUTPUT_UPDATE_INTERVAL);
                    if due {
                        last.insert(id.clone(), now);
                    }
                    due
                };
                if let Some(info) = due.then(|| self.info(&id)).flatten() {
                    self.events.emit(Event::JobUpdated { job: info });
                }
            }
            JobEvent::Exited { id, .. } => {
                lock(&self.last_update).remove(&id);
                if let Some(info) = self.info(&id) {
                    self.reminders
                        .push(&info.owner, z_engine_context::job_finished(&info));
                    self.events.emit(Event::JobUpdated { job: info });
                }
            }
        }
    }
}

fn info_of(snapshot: &JobSnapshot) -> JobInfo {
    JobInfo {
        job_id: snapshot.id.clone(),
        kind: JobKind::Shell,
        label: snapshot.label.clone(),
        owner: AgentId::from(snapshot.owner.as_str()),
        agent_id: None,
        status: snapshot.status,
        exit_code: snapshot.exit_code,
        started_at: snapshot.started_at,
        finished_at: snapshot.finished_at,
        output_tail: snapshot.tail.clone(),
    }
}
