//! What a loaded run reads back as.
//!
//! Every projection sorts by the sequence the recorder allocated, never
//! by append order: entries land when their work *finishes*, and under
//! concurrency that is not the order the run happened in. Exchanges
//! sort by lane first — two callers sharing one provider have an order
//! within each lane and none between them.

use z_engine_provider::RequestLane;

use super::entry::{
    CassetteEntry, CompletionRecord, EvidenceMint, GateDecision, PromptHash, ProviderExchange,
    RunMetrics, ToolOutcome,
};
use super::tape::RunCassette;

impl RunCassette {
    /// Recorded exchanges, by lane and then by request order. Entries
    /// land when a stream *ends*, so append order is neither; the lane
    /// and the sequence allocated when the request was issued are.
    pub fn exchanges(&self) -> Vec<ProviderExchange> {
        let mut out: Vec<ProviderExchange> = self
            .entries()
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Exchange(x) => Some(x.clone()),
                _ => None,
            })
            .collect();
        out.sort_by(|a, b| (&a.lane, a.sequence).cmp(&(&b.lane, b.sequence)));
        out
    }

    /// Recorded exchanges on one lane, in request order.
    pub fn exchanges_on(&self, lane: &RequestLane) -> Vec<ProviderExchange> {
        self.exchanges()
            .into_iter()
            .filter(|x| &x.lane == lane)
            .collect()
    }

    /// Every lane this run used, in a stable order.
    pub fn lanes(&self) -> Vec<RequestLane> {
        let mut out: Vec<RequestLane> = self.exchanges().into_iter().map(|x| x.lane).collect();
        out.dedup();
        out
    }

    pub fn request_hashes(&self) -> Vec<String> {
        self.exchanges()
            .into_iter()
            .map(|x| x.request_hash)
            .collect()
    }

    pub fn prompt_hashes(&self) -> Vec<String> {
        let mut out: Vec<PromptHash> = self
            .entries()
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Prompt(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        out.sort_by_key(|p| p.sequence);
        out.into_iter().map(|p| p.hash).collect()
    }

    pub fn tool_outcomes(&self) -> Vec<ToolOutcome> {
        let mut out: Vec<ToolOutcome> = self
            .entries()
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Tool(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        out.sort_by_key(|t| t.sequence);
        out
    }

    pub fn gate_decisions(&self) -> Vec<GateDecision> {
        let mut out: Vec<GateDecision> = self
            .entries()
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Gate(g) => Some(g.clone()),
                _ => None,
            })
            .collect();
        out.sort_by_key(|g| g.sequence);
        out
    }

    pub fn evidence_ids(&self) -> Vec<EvidenceMint> {
        let mut out: Vec<EvidenceMint> = self
            .entries()
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Evidence(m) => Some(m.clone()),
                _ => None,
            })
            .collect();
        out.sort_by_key(|m| m.sequence);
        out
    }

    /// The last completion recorded — a run is judged on where it ended.
    pub fn completion(&self) -> Option<CompletionRecord> {
        self.entries()
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Completion(c) => Some(c.clone()),
                _ => None,
            })
            .max_by_key(|c| c.sequence)
    }

    pub fn manifest_hash(&self) -> Option<String> {
        self.completion().map(|c| c.manifest_hash)
    }

    pub fn diff_hash(&self) -> Option<String> {
        self.completion().and_then(|c| c.diff_hash)
    }

    /// Metrics are appended once per turn; the last line is the run's.
    pub fn metrics(&self) -> Option<RunMetrics> {
        self.entries()
            .iter()
            .filter_map(|e| match e {
                CassetteEntry::Metrics(m) => Some(m.clone()),
                _ => None,
            })
            .next_back()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::entry::canonical_request;
    use crate::replay::sink::{CassetteWriter, EntrySink};
    use z_engine_provider::{ChatMessage, ChatRequest, StreamEvent};

    fn exchange(lane: RequestLane, sequence: u64, model: &str) -> CassetteEntry {
        let request = ChatRequest::new(model, vec![ChatMessage::user("hi")]);
        let (_, hash) = canonical_request(&request).unwrap();
        CassetteEntry::Exchange(ProviderExchange {
            lane,
            sequence,
            request_hash: hash,
            request,
            events: vec![StreamEvent::TextDelta("ok".into()), StreamEvent::Done],
            error: None,
        })
    }

    /// Streams finish out of order under concurrency; request order is
    /// the order the requests were *issued* in.
    #[test]
    fn exchanges_replay_in_issue_order_not_append_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let writer = CassetteWriter::create(&path).unwrap();
        writer
            .append(&exchange(RequestLane::MAIN, 1, "second"))
            .unwrap();
        writer
            .append(&exchange(RequestLane::MAIN, 0, "first"))
            .unwrap();

        let cassette = RunCassette::load(&path).unwrap();
        let models: Vec<String> = cassette
            .exchanges()
            .into_iter()
            .map(|x| x.request.model)
            .collect();
        assert_eq!(models, ["first", "second"]);
    }

    /// Two callers sharing one provider have no order relative to each
    /// other, so their requests are grouped by lane rather than raced
    /// into a single sequence.
    #[test]
    fn exchanges_are_grouped_by_lane() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let writer = CassetteWriter::create(&path).unwrap();
        let title = RequestLane::named("title");
        writer
            .append(&exchange(title.clone(), 0, "titled"))
            .unwrap();
        writer
            .append(&exchange(RequestLane::MAIN, 1, "second"))
            .unwrap();
        writer
            .append(&exchange(RequestLane::MAIN, 0, "first"))
            .unwrap();

        let cassette = RunCassette::load(&path).unwrap();
        assert_eq!(cassette.lanes(), [RequestLane::MAIN, title.clone()]);
        let main: Vec<String> = cassette
            .exchanges_on(&RequestLane::MAIN)
            .into_iter()
            .map(|x| x.request.model)
            .collect();
        assert_eq!(main, ["first", "second"]);
        assert_eq!(cassette.exchanges_on(&title).len(), 1);
        assert_eq!(
            cassette.exchanges_on(&RequestLane::named("nobody")).len(),
            0
        );
    }
}
