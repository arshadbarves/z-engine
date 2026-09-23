//! `InteractionPort`: questions and plan reviews through the broker (main
//! agent only), and todo lists persisted and announced per agent.

use std::sync::Arc;

use async_trait::async_trait;
use z_engine_protocol::{Event, PlanDecision, Question, QuestionAnswer, TodoItem};
use z_engine_store::LogRecord;
use z_engine_tools::{InteractionPort, ToolCtx};

use crate::broker::Broker;
use crate::hooks::notify;
use crate::session::SessionCore;

#[derive(Debug)]
pub(crate) struct Interaction {
    core: Arc<SessionCore>,
}

impl Interaction {
    pub(crate) fn new(core: Arc<SessionCore>) -> Self {
        Self { core }
    }
}

#[async_trait]
impl InteractionPort for Interaction {
    fn can_ask(&self, ctx: &ToolCtx) -> bool {
        ctx.agent_id.is_main()
    }

    async fn ask(
        &self,
        ctx: &ToolCtx,
        questions: Vec<Question>,
    ) -> Result<Option<Vec<QuestionAnswer>>, String> {
        if !self.can_ask(ctx) {
            return Err("only the main agent can ask the user".to_string());
        }
        let broker = &self.core.broker;
        let (request_id, reply) = broker
            .ask(ctx.agent_id.clone(), questions)
            .map_err(|error| error.to_string())?;
        notify(&self.core, "Z Engine has a question for you", &ctx.cancel).await;
        match Broker::wait(reply, &ctx.cancel).await {
            Some(answers) => Ok(answers),
            None => {
                broker.withdraw_question(&request_id);
                Err("cancelled by user".to_string())
            }
        }
    }

    async fn propose_plan(&self, ctx: &ToolCtx, plan: String) -> Result<PlanDecision, String> {
        if !self.can_ask(ctx) {
            return Err("only the main agent can propose a plan".to_string());
        }
        let broker = &self.core.broker;
        let (request_id, reply) = broker
            .propose_plan(ctx.agent_id.clone(), plan)
            .map_err(|error| error.to_string())?;
        notify(
            &self.core,
            "Z Engine has a plan for you to review",
            &ctx.cancel,
        )
        .await;
        match Broker::wait(reply, &ctx.cancel).await {
            Some(decision) => Ok(decision),
            None => {
                broker.withdraw_plan(&request_id);
                Err("cancelled by user".to_string())
            }
        }
    }

    fn update_todos(&self, ctx: &ToolCtx, todos: Vec<TodoItem>) {
        let agent_id = ctx.agent_id.clone();
        let persisted = self.core.journal.append_or_report(&LogRecord::Todos {
            agent_id: agent_id.clone(),
            todos: todos.clone(),
        });
        if !persisted {
            tracing::warn!(agent = %agent_id, count = todos.len(), "todo list kept in memory only");
        }
        self.core
            .with_state(|state| state.todos.insert(agent_id.clone(), todos.clone()));
        self.core
            .events
            .emit(Event::TodosUpdated { agent_id, todos });
    }
}
