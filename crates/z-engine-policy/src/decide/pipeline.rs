//! The decision order shared by every action.

use z_engine_protocol::PermissionMode;

use super::execute::{self, CommandFacts};
use super::{files, other};
use crate::action::Action;
use crate::engine::{Decision, Policy};
use crate::rules::Rule;

const PLAN_MODE: &str = "plan mode is read-only; use ExitPlanMode to propose the plan";

impl Policy {
    /// Decides one tool call, in this order:
    ///
    /// 1. a matching deny rule denies (for shell commands, any command in
    ///    the line, including substitutions and wrapped commands);
    /// 2. plan mode denies anything that changes state;
    /// 3. bypass mode allows;
    /// 4. a matching ask rule asks;
    /// 5. matching allow rules, configured or granted this session, allow
    ///    (for shell commands, every command in the line must be covered);
    /// 6. otherwise the per-action default applies.
    pub fn decide(&self, tool: &str, action: &Action, mode: PermissionMode) -> Decision {
        let command = match action {
            Action::Execute { command } => Some(CommandFacts::new(command)),
            _ => None,
        };
        if let Some(rule) = self.restricting(&self.deny, tool, action, command.as_ref()) {
            return Decision::Deny {
                reason: format!("denied by rule {rule}"),
            };
        }
        let mutates = match &command {
            Some(facts) => !facts.read_only(),
            None => action.mutates(),
        };
        if mode == PermissionMode::Plan && mutates {
            return Decision::Deny {
                reason: PLAN_MODE.into(),
            };
        }
        if mode == PermissionMode::Bypass {
            return Decision::allow("bypass mode");
        }
        if let Some(rule) = self.restricting(&self.ask, tool, action, command.as_ref()) {
            let can_persist = !files::touches_outside(self, action);
            return Decision::ask(format!("rule {rule} requires approval"), None, can_persist);
        }
        match (action, &command) {
            (Action::Read { paths } | Action::Write { paths }, _) => {
                files::decide(self, tool, action, paths, mode)
            }
            (Action::Execute { .. }, Some(facts)) => execute::decide(self, tool, facts, mode),
            _ => other::decide(self, tool, action),
        }
    }

    fn restricting<'a>(
        &self,
        rules: &'a [Rule],
        tool: &str,
        action: &Action,
        command: Option<&CommandFacts>,
    ) -> Option<&'a Rule> {
        rules.iter().find(|rule| match (action, command) {
            (Action::Read { paths } | Action::Write { paths }, _) => {
                files::restricts(self, rule, tool, action, paths)
            }
            (Action::Execute { .. }, Some(facts)) => execute::restricts(self, rule, tool, facts),
            _ => rule.matches_call(tool, action),
        })
    }
}
