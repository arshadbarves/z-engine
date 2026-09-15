//! Repository instruction loading and assembly, separate from packet budgeting.

use std::path::Path;

pub fn build_system_prompt(project_root: &Path, agents_md: Option<&str>) -> String {
    let mut p = String::from(crate::prompts::SYSTEM_MAIN.trim_end_matches('\n'));
    p.push_str("\n\nProject root: ");
    p.push_str(&project_root.to_string_lossy());
    if let Some(md) = agents_md {
        p.push_str("\n\n# AGENTS.md (project instructions)\n");
        p.push_str(md.trim_end());
        p.push('\n');
    }
    p
}

pub fn load_agents_md(project_root: &Path) -> Option<String> {
    std::fs::read_to_string(project_root.join("AGENTS.md"))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_root_and_instructions() {
        let p = build_system_prompt(Path::new("/tmp/proj"), None);
        assert!(p.contains("Project root: /tmp/proj"));
        assert!(p.contains("autonomous coding agent"));
    }

    #[test]
    fn agents_md_appended_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("AGENTS.md"), "Always run clippy.\n").unwrap();
        let md = load_agents_md(tmp.path()).unwrap();
        let p = build_system_prompt(tmp.path(), Some(&md));
        assert!(p.contains("# AGENTS.md"));
        assert!(p.contains("Always run clippy."));
    }

    #[test]
    fn missing_or_blank_agents_md_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load_agents_md(tmp.path()).is_none());
        std::fs::write(tmp.path().join("AGENTS.md"), "   \n").unwrap();
        assert!(load_agents_md(tmp.path()).is_none());
    }
}
