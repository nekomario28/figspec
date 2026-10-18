//! The entry documents' links resolve.
//!
//! A document that points at a file which is not there is a document that lies to
//! whoever follows it, and nothing about the code would notice. Written after
//! inventing `docs/PLAN.md` in a README: the link looked plausible, and only a
//! reader would have found out.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Every relative link target in `text`, as a path.
fn relative_targets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("](") {
        let after = &rest[open + 2..];
        let Some(close) = after.find(')') else { break };
        let target = &after[..close];
        let target = target.split('#').next().unwrap_or(target);
        let target = target.trim();
        // Only paths into this repository: an absolute URL or an anchor is not a
        // file we can check.
        if !target.is_empty()
            && !target.contains("://")
            && !target.starts_with('#')
            && !target.starts_with("mailto:")
        {
            out.push(target.to_string());
        }
        rest = &after[close..];
    }
    out
}

#[test]
fn every_link_in_the_entry_documents_resolves() {
    let root = root();
    for document in ["README.md", "AGENTS.md", "llms.txt", "docs/CONTRACT.md"] {
        let path = root.join(document);
        let text = std::fs::read_to_string(&path).expect("a readable document");
        for target in relative_targets(&text) {
            assert!(
                root.join(&target).exists() || path.parent().unwrap().join(&target).exists(),
                "{document} links to {target}, which does not exist"
            );
        }
    }
}

#[test]
fn the_link_checker_finds_links_and_skips_what_it_cannot_check() {
    // A checker that silently finds nothing would pass on any document.
    let text = "[a](docs/CONTRACT.md) [b](https://example.com/x) [c](#anchor) [d](README.md#top)";
    assert_eq!(
        relative_targets(text),
        vec!["docs/CONTRACT.md", "README.md"]
    );
    assert!(relative_targets("no links here").is_empty());
}

#[test]
fn the_agent_code_list_matches_the_codebook() {
    // An agent branches on this advertised set. A partial list silently hides
    // findings even though both the implementation and the README know them.
    let text = std::fs::read_to_string(root().join("llms.txt")).unwrap();
    let line = text
        .lines()
        .find_map(|line| line.strip_prefix("- Codes: "))
        .unwrap();
    let listed: std::collections::BTreeSet<_> = line
        .split(|c: char| !(c.is_ascii_lowercase() || c == '-'))
        .filter(|word| word.contains('-'))
        .collect();
    let known: std::collections::BTreeSet<_> =
        figspec::CODEBOOK.iter().map(|(code, _, _)| *code).collect();
    assert_eq!(listed, known);
}
