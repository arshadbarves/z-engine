export type HeroStarter = {
  id: string;
  title: string;
  desc: string;
  prompt: string;
  iconName: "Search" | "Sparkles" | "Wrench" | "Workflow";
};

export const HERO_STARTERS: HeroStarter[] = [
  {
    id: "explore",
    title: "Explore Architecture",
    desc: "Map codebase structure, modules, and entry points",
    prompt: "Give me an architectural overview of this codebase: key modules, data flow, and entry points.",
    iconName: "Search",
  },
  {
    id: "refactor",
    title: "Refactor & Clean Code",
    desc: "Simplify logic, remove dead code, and clean technical debt",
    prompt: "Analyze the codebase for areas that can be refactored, simplified, or optimized.",
    iconName: "Sparkles",
  },
  {
    id: "fix",
    title: "Diagnose & Fix Bugs",
    desc: "Scan for edge cases, broken invariants, and runtime errors",
    prompt: "Review recent changes and critical paths for potential bugs, error cases, and fixes.",
    iconName: "Wrench",
  },
  {
    id: "test",
    title: "Write Test Suites",
    desc: "Generate verification tests and edge-case test coverage",
    prompt: "Create comprehensive tests covering the primary workflows and edge cases.",
    iconName: "Workflow",
  },
];

export const HERO_EXAMPLES = HERO_STARTERS.map((s) => s.prompt);
