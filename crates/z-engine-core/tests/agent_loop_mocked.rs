//! Integration tests: the full agent loop against a mocked
//! OpenAI-compatible provider serving canned SSE (spec §10).
//!
//! The mock is an axum app with a scripted queue of responses; every
//! request body is captured so tests can assert on what the model received.

#[path = "support/mock_loop.rs"]
mod mock_loop;

#[path = "agent_loop/cancellation.rs"]
mod cancellation;
#[path = "agent_loop/compaction.rs"]
mod compaction;
#[path = "agent_loop/editing.rs"]
mod editing;
#[path = "agent_loop/mcp.rs"]
mod mcp;
#[path = "agent_loop/permissions.rs"]
mod permissions;
#[path = "agent_loop/repo_map.rs"]
mod repo_map;
#[path = "agent_loop/research.rs"]
mod research;
#[path = "agent_loop/review.rs"]
mod review;
#[path = "agent_loop/streaming.rs"]
mod streaming;
