//! The invariant tables of `docs/DATA-MODEL.md` and the `Violation` enum
//! list the same set of numbers, in the same order. A row added, removed or
//! renumbered on one side fails here.

use std::collections::BTreeSet;

const DOC: &str = include_str!("../../../docs/DATA-MODEL.md");
const SOURCE: &str = include_str!("../src/violation.rs");

fn is_code(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some('M' | 'V' | 'E' | 'L' | 'F' | 'S' | 'B'))
        && chars.clone().count() >= 1
        && chars.all(|c| c.is_ascii_digit())
}

/// Every `| <code> |` table row of the §Invariants section, in order.
fn doc_codes() -> Vec<String> {
    let section = DOC
        .split_once("## Invariants")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("## Provenance").map(|(s, _)| s))
        .expect("DATA-MODEL.md has an Invariants section before Provenance");
    section
        .lines()
        .filter_map(|line| {
            let first = line.trim().strip_prefix('|')?.split('|').next()?.trim();
            is_code(first).then(|| first.to_string())
        })
        .collect()
}

/// Every `/// **<code>** —` doc comment on a `Violation` variant, in order.
fn variant_doc_codes() -> Vec<String> {
    SOURCE
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("/// **")?;
            let (code, _) = rest.split_once("**")?;
            is_code(code).then(|| code.to_string())
        })
        .collect()
}

/// Every `=> "<code>"` arm of `Violation::code`, in order.
fn code_arms() -> Vec<String> {
    let body = SOURCE
        .split_once("pub const fn code(&self)")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("pub const fn level").map(|(s, _)| s))
        .expect("Violation::code precedes Violation::level");
    body.lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once("=> \"")?;
            let (code, _) = rest.split_once('"')?;
            is_code(code).then(|| code.to_string())
        })
        .collect()
}

#[test]
fn doc_tables_and_violation_variants_list_the_same_invariants() {
    let doc: BTreeSet<_> = doc_codes().into_iter().collect();
    let variants: BTreeSet<_> = variant_doc_codes().into_iter().collect();
    let arms: BTreeSet<_> = code_arms().into_iter().collect();
    assert!(doc.len() >= 20, "parsed too few rows from the doc: {doc:?}");
    assert_eq!(doc, variants, "doc rows vs Violation variant doc comments");
    assert_eq!(
        variants, arms,
        "Violation variant doc comments vs Violation::code arms"
    );
}

#[test]
fn variants_are_declared_in_the_doc_row_order() {
    assert_eq!(doc_codes(), variant_doc_codes());
    assert_eq!(
        doc_codes().len(),
        doc_codes().iter().collect::<BTreeSet<_>>().len(),
        "no duplicate rows"
    );
}
