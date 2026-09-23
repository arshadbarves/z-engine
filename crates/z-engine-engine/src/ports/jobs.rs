//! `JobPort`: background shells through the session's job hub.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use z_engine_host::BackgroundSpec;
use z_engine_protocol::{JobId, JobKind};
use z_engine_tools::{JobOutput, JobPort, ToolCtx};

use crate::session::SessionCore;

#[derive(Debug)]
pub(crate) struct Jobs {
    core: Arc<SessionCore>,
}

impl Jobs {
    pub(crate) fn new(core: Arc<SessionCore>) -> Self {
        Self { core }
    }
}

#[async_trait]
impl JobPort for Jobs {
    async fn spawn_shell(
        &self,
        ctx: &ToolCtx,
        command: String,
        description: Option<String>,
    ) -> Result<JobId, String> {
        let label = description
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| command.clone());
        let spec = BackgroundSpec {
            command,
            cwd: ctx.current_cwd(),
            shell: ctx.shell.spec.clone(),
            env: ctx.shell.env.clone(),
            label,
            owner: ctx.agent_id.to_string(),
        };
        let info = self
            .core
            .jobs
            .spawn(spec)
            .await
            .map_err(|error| error.to_string())?;
        Ok(info.job_id)
    }

    async fn output(
        &self,
        ctx: &ToolCtx,
        job: &JobId,
        filter: Option<String>,
        wait: Option<Duration>,
    ) -> Result<JobOutput, String> {
        let filter = filter
            .map(|pattern| Regex::new(&pattern))
            .transpose()
            .map_err(|error| format!("invalid filter: {error}"))?;
        let jobs = &self.core.jobs;
        let read = match wait {
            Some(timeout) => {
                let waited = ctx.until_cancelled(jobs.wait(job, timeout)).await;
                let mut read = waited
                    .map_err(|error| error.to_string())?
                    .map_err(|error| error.to_string())?;
                if let Some(filter) = &filter {
                    read.output = keep_matching(&read.output, filter);
                }
                read
            }
            None => jobs
                .read(job, filter.as_ref())
                .map_err(|error| error.to_string())?,
        };
        Ok(JobOutput {
            status: read.status,
            exit_code: read.exit_code,
            output: read.output,
            kind: JobKind::Shell,
        })
    }

    async fn kill(&self, _ctx: &ToolCtx, job: &JobId) -> Result<(), String> {
        self.core
            .jobs
            .kill(job)
            .await
            .map_err(|error| error.to_string())
    }
}

fn keep_matching(output: &str, filter: &Regex) -> String {
    output
        .lines()
        .filter(|line| filter.is_match(line))
        .map(|line| format!("{line}\n"))
        .collect()
}
