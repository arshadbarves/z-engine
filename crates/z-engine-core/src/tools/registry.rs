use std::collections::HashMap;
use std::sync::Arc;

use super::{
    Tool, assess_completion, bash, context_notes, edit_file, glob, grep, inspect_project,
    read_file, run_verification, task, write_file,
};

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    order: Vec<String>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        if !self.tools.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn names(&self) -> &[String] {
        &self.order
    }

    pub fn readonly_subset() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(read_file::ReadFileTool));
        reg.register(Arc::new(glob::GlobTool));
        reg.register(Arc::new(grep::GrepTool));
        reg.register(Arc::new(inspect_project::InspectProjectTool));
        reg
    }

    pub fn defs(&self) -> Vec<z_engine_provider::ToolDef> {
        self.order
            .iter()
            .filter_map(|name| self.tools.get(name))
            .map(|tool| {
                z_engine_provider::ToolDef::function(
                    tool.name(),
                    tool.description(),
                    tool.parameters_schema(),
                )
            })
            .collect()
    }

    pub fn builtins() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(read_file::ReadFileTool));
        reg.register(Arc::new(bash::BashTool));
        reg.register(Arc::new(write_file::WriteFileTool));
        reg.register(Arc::new(edit_file::EditFileTool));
        reg.register(Arc::new(glob::GlobTool));
        reg.register(Arc::new(grep::GrepTool));
        reg.register(Arc::new(context_notes::UpdateContextNotesTool));
        reg.register(Arc::new(task::TaskTool));
        reg.register(Arc::new(run_verification::RunVerificationTool));
        reg.register(Arc::new(assess_completion::AssessCompletionTool));
        reg.register(Arc::new(inspect_project::InspectProjectTool));
        reg
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.order)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_uses_main_execution_not_research_capabilities() {
        let main = ToolRegistry::builtins();
        assert!(main.get("run_verification").is_some());
        assert!(main.get("assess_completion").is_some());
        let research = ToolRegistry::readonly_subset();
        assert!(research.get("run_verification").is_none());
        assert!(research.get("assess_completion").is_none());
    }

    #[test]
    fn project_discovery_is_available_to_main_and_research() {
        for registry in [ToolRegistry::builtins(), ToolRegistry::readonly_subset()] {
            assert!(registry.get("inspect_project").is_some());
            assert_eq!(
                registry
                    .names()
                    .iter()
                    .filter(|name| *name == "inspect_project")
                    .count(),
                1
            );
        }
    }
}
