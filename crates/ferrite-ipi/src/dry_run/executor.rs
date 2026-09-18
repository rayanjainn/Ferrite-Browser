//! [`RecordingExecutor`]: the `ToolExecutor` that never touches the real
//! browser.
//!
//! # Containment is by construction (closes `docs/TO-DO.md` T-007 / D7)
//!
//! There used to be a `crate::containment` module: an `Option C`
//! application-layer interceptor plus an `Option B` Linux network-namespace
//! creator (`nix::sched::unshare(CLONE_NEWNET)`), activated around every
//! `DryRunOrchestrator::run`. D7 named it precisely: `intercept_request()`
//! was unreachable outside its own unit test, because this executor —
//! `RecordingExecutor` — never made a real network call for it to intercept.
//! Containment was already "by mock", not by interception; the interception
//! machinery was dead code wearing a security label, which the directive
//! calls out as strictly worse than having nothing there ("dead
//! defense-in-depth ... reads as protection in the paper").
//!
//! This session deleted that module entirely (default option (a), per
//! `docs/REBUILD_DIRECTIVE.md` §6/A6 and `docs/TO-DO.md` T-007), rather than
//! keeping it "in case a real-executor dry-run backend shows up later": no
//! such backend is in the current plan (`docs/REBUILD_DIRECTIVE.md`'s A9
//! charter builds `BrowserEngine`/`MockEngine`/`ServoEngine` for the
//! *production* agent loop, not for the dry run — the dry run's entire
//! reason to exist is running the agent against synthetic data instead of
//! the real engine; see `docs/TO-DO.md`, no open T-### asks for a
//! real-executor dry-run mode). If that ever changes, containment is a new
//! design question for whoever builds that backend, not a matter of
//! reviving this module.
//!
//! What actually holds instead: **`RecordingExecutor` has no code path that
//! performs real network or engine I/O.** Every branch below either records
//! bookkeeping (`self.record`, an in-process `Mutex<DryRunRecord>`) or reads
//! from `self.content`, an in-process `Mutex<DryRunContent>` populated by
//! the case author. This file references no HTTP client, no async network
//! task, no raw socket type, and no real browser engine call anywhere — the
//! `this_modules_source_names_no_network_capable_dependency` test below is
//! the grep-backed check for exactly that claim, not vibes (it builds its
//! search strings from split literals so it cannot match its own prose, and
//! so a future accidental addition of one of those dependencies here is
//! exactly what would make it fail).

use std::sync::{Arc, Mutex};

use ferrite_agent::{AgentToolCall, AgentToolResult, BrowserTool, ToolExecutor};

use crate::sanitizer::{self, Finding};
use crate::tool_decision::ToolId;
use crate::twin::SyntheticTwin;

use super::content::{DryRunContent, DryRunReply};
use super::record::{DryRunRecord, FindingCarrier, RecordedFinding};

/// `ToolExecutor` that records every tool call and returns synthetic data.
/// Never touches the real browser. See module docs for why containment is
/// structural here rather than an active interception layer.
pub(crate) struct RecordingExecutor {
    pub(crate) record: Arc<Mutex<DryRunRecord>>,
    pub(crate) twin: SyntheticTwin,
    pub(crate) current_origin: Arc<Mutex<Option<String>>>,
    pub(crate) content: Mutex<DryRunContent>,
    /// Gates inline sanitizer detection. Derived from `DefenseMode` by the
    /// orchestrator/caller — this executor does not know about `DefenseMode`.
    pub(crate) detect_enabled: bool,
    /// Gates active excision of detected injections from the served reply.
    /// Unreachable without `detect_enabled` — see
    /// `DryRunOrchestrator::run`'s debug_assert.
    pub(crate) strip_enabled: bool,
}

#[async_trait::async_trait]
impl ToolExecutor for RecordingExecutor {
    async fn execute(&self, call: &AgentToolCall) -> AgentToolResult {
        let tool_id = ToolId::from(&call.tool);

        // Navigation updates the current origin before the event is recorded,
        // so the navigate event itself carries the destination origin.
        if let BrowserTool::Navigate(ref url) = call.tool {
            let origin = super::record::extract_origin(url);
            *self.current_origin.lock().unwrap() = Some(origin);
            let mut rec = self.record.lock().unwrap();
            rec.record_network_attempt(url.clone());
        }

        let origin = self.current_origin.lock().unwrap().clone();
        {
            let mut rec = self.record.lock().unwrap();
            rec.record_tool(tool_id.clone(), origin.clone());
        }

        let mut content = self.content.lock().unwrap();
        let reply = match &call.tool {
            BrowserTool::ReadPage => {
                content
                    .read_page
                    .next(origin.as_deref())
                    .unwrap_or_else(|| {
                        DryRunReply::Ok(serde_json::Value::String(format!(
                            "Synthetic page. User: {}",
                            self.twin.name
                        )))
                    })
            }
            BrowserTool::ExtractData(_) => content
                .extract_data
                .next(origin.as_deref())
                .unwrap_or_else(|| {
                    DryRunReply::Ok(serde_json::Value::String(format!(
                        "Extracted: {}",
                        self.twin.email
                    )))
                }),
            other => content
                .by_tool_id
                .get_mut(other.tool_id())
                .and_then(|c| c.next(origin.as_deref()))
                .unwrap_or_else(|| {
                    DryRunReply::Ok(serde_json::Value::String("dry-run: ok".to_string()))
                }),
        };
        drop(content);

        // Strip is unreachable without detect: findings (and, for ReadPage, the
        // clean_html needed to excise HTML rather than re-deriving it) are only
        // computed inside this block. See the debug_assert in
        // `DryRunOrchestrator::run` that enforces the invariant at the mode level.
        let served_reply = if self.detect_enabled {
            let value = match &reply {
                DryRunReply::Ok(v) => v.clone(),
                DryRunReply::Err(e) => serde_json::Value::String(e.clone()),
            };

            let mut findings: Vec<RecordedFinding> = sanitizer::detect_injection_in_value(&value)
                .into_iter()
                .map(|lf| RecordedFinding {
                    finding: lf.finding,
                    carrier: FindingCarrier::ToolOutput { json_path: lf.path },
                    tool: tool_id.clone(),
                    origin: origin.clone(),
                })
                .collect();

            let mut read_page_clean_html: Option<String> = None;
            if matches!(call.tool, BrowserTool::ReadPage) {
                if let serde_json::Value::String(html) = &value {
                    let sanitized = sanitizer::sanitize_html(html);
                    read_page_clean_html = Some(sanitized.clean_html.clone());
                    findings.extend(sanitized.visible_text_findings.into_iter().map(|f| {
                        RecordedFinding {
                            finding: f,
                            carrier: FindingCarrier::WebContent {
                                channel: "visible_text".to_string(),
                            },
                            tool: tool_id.clone(),
                            origin: origin.clone(),
                        }
                    }));
                    findings.extend(sanitized.comment_findings.into_iter().map(|f| {
                        RecordedFinding {
                            finding: f,
                            carrier: FindingCarrier::WebContent {
                                channel: "comment".to_string(),
                            },
                            tool: tool_id.clone(),
                            origin: origin.clone(),
                        }
                    }));
                    findings.extend(sanitized.script_findings.into_iter().map(|label| {
                        RecordedFinding {
                            finding: Finding {
                                pattern: label,
                                snippet: String::new(),
                            },
                            carrier: FindingCarrier::WebContent {
                                channel: "script".to_string(),
                            },
                            tool: tool_id.clone(),
                            origin: origin.clone(),
                        }
                    }));
                }
            }

            if !findings.is_empty() {
                let mut rec = self.record.lock().unwrap();
                for finding in findings {
                    rec.record_finding(finding);
                }
            }

            if self.strip_enabled {
                match &reply {
                    DryRunReply::Ok(v) => {
                        if matches!(call.tool, BrowserTool::ReadPage) {
                            if let Some(clean) = &read_page_clean_html {
                                DryRunReply::Ok(serde_json::Value::String(
                                    sanitizer::excise_injections_html(clean),
                                ))
                            } else {
                                DryRunReply::Ok(sanitizer::excise_value(v))
                            }
                        } else {
                            DryRunReply::Ok(sanitizer::excise_value(v))
                        }
                    }
                    DryRunReply::Err(e) => DryRunReply::Err(sanitizer::excise_injections_text(e)),
                }
            } else {
                reply
            }
        } else {
            reply
        };

        match served_reply {
            DryRunReply::Ok(v) => AgentToolResult::ok(call.call_id, v),
            DryRunReply::Err(e) => AgentToolResult::err(call.call_id, e),
        }
    }
}

#[cfg(test)]
mod containment_tests {
    /// `docs/TO-DO.md` T-007 / D7: containment is now by construction, not
    /// by an active interceptor. This is the grep-backed half of that claim
    /// — the executor's own source has no network-capable dependency
    /// reachable from it. (The other half — that nothing calls out to a real
    /// engine either — is structural: `RecordingExecutor` only ever reads
    /// `self.content`/`self.twin`, both in-process values with no I/O of
    /// their own; see the module docs above.)
    ///
    /// Needles are built from split literals rather than written whole, so
    /// this test cannot trivially match its own source text (including this
    /// doc comment) — only a real reference in the scanned file's actual
    /// code would join the parts into a matching substring.
    #[test]
    fn this_modules_source_names_no_network_capable_dependency() {
        let source = include_str!("executor.rs");
        let forbidden: [[&str; 2]; 5] = [
            ["re", "qwest"],
            ["tokio", "::net"],
            ["std", "::net"],
            ["Tcp", "Stream"],
            ["hyper", "::"],
        ];
        for parts in forbidden {
            let needle = parts.concat();
            assert!(
                !source.contains(&needle),
                "executor.rs must not reference {needle} — containment is by construction \
                 (no network backend), and a reference here would be evidence against that claim"
            );
        }
    }
}
