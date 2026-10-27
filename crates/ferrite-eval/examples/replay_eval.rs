//! `just replay-eval` — the oracle replay (ADR-026): AgentDojo's own
//! ground-truth calls through the runtime guard, under each predictor and
//! gating. Prints the report and writes `target/eval-report/REPLAY_REPORT.md`.
//! No network, no API key, no model.

use std::path::PathBuf;

use ferrite_eval::replay::{render, Replay};

fn main() {
    let tests = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let replay = match Replay::load(
        &tests.join("agentdojo_full_calls.json"),
        &tests.join("agentdojo_full"),
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("replay-eval: {e}");
            std::process::exit(1);
        }
    };
    let md = render(&replay);
    print!("{md}");
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/eval-report");
    if let Err(e) = std::fs::create_dir_all(&out)
        .and_then(|()| std::fs::write(out.join("REPLAY_REPORT.md"), &md))
    {
        eprintln!("replay-eval: could not write the report: {e}");
        std::process::exit(1);
    }
}
