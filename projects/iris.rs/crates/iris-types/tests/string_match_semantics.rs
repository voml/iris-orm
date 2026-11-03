//! Reference-backend semantics for `contains` / `starts_with` / `ends_with`:
//! case-sensitive UTF-8 substring matching, empty-needle behaviour, prefix and
//! suffix boundaries, and a structured rejection for non-text values.

use iris_ir::{
    EffectKind, IrEnvelope, IrVersion, PhysicalOp, PhysicalPlan, PlannedNode, Pred, RealizationClass, SchemaFingerprint, SemanticHash,
    StringMatchOp,
};
use iris_types::{Error, ReferenceStore, Value, row_from_pairs};

fn plan(op: StringMatchOp, needle: &str) -> PhysicalPlan {
    PhysicalPlan {
        envelope: IrEnvelope {
            vos_contract_version: "vos-dev".into(),
            ir_version: IrVersion::PHASE1,
            schema_fingerprint: SchemaFingerprint::unbound(),
            operation_id: "string-match".into(),
            effect: EffectKind::Read,
            required_capabilities: Vec::new(),
            span_start: 0,
            span_end: 0,
            semantic_hash: SemanticHash(0),
        },
        nodes: vec![
            PlannedNode { op: PhysicalOp::Scan { table: "Note".into() }, realization: RealizationClass::Native, note: None },
            PlannedNode {
                op: PhysicalOp::Filter { predicate: Pred::StringMatch { field: "title".into(), op, needle: needle.into() } },
                realization: RealizationClass::Native,
                note: None,
            },
            PlannedNode { op: PhysicalOp::Collect, realization: RealizationClass::Native, note: None },
        ],
    }
}

fn store() -> ReferenceStore {
    let mut store = ReferenceStore::new();
    store.seed(
        "Note",
        vec![
            row_from_pairs(&[("id", Value::Int(1)), ("title", Value::Str("alpha".into()))]),
            row_from_pairs(&[("id", Value::Int(2)), ("title", Value::Str("beta".into()))]),
            row_from_pairs(&[("id", Value::Int(3)), ("title", Value::Str("".into()))]),
            row_from_pairs(&[("id", Value::Int(4)), ("title", Value::Null)]),
            row_from_pairs(&[("id", Value::Int(5)), ("title", Value::Str("hello 世界".into()))]),
        ],
    );
    store
}

fn poisoned_store() -> ReferenceStore {
    let mut store = store();
    store.seed("Note", vec![row_from_pairs(&[("id", Value::Int(6)), ("title", Value::Int(7))])]);
    store
}

fn ids(rows: &[iris_types::Row]) -> Vec<i64> {
    let mut ids: Vec<i64> = rows
        .iter()
        .filter_map(|row| match row.get("id") {
            Some(Value::Int(id)) => Some(*id),
            _ => None,
        })
        .collect();
    ids.sort_unstable();
    ids
}

#[test]
fn contains_matches_substrings_and_the_empty_needle_matches_every_text() {
    let results = store().execute_plan(&plan(StringMatchOp::Contains, "lph")).expect("contains");
    assert_eq!(ids(&results), vec![1]);

    let empty = store().execute_plan(&plan(StringMatchOp::Contains, "")).expect("empty contains");
    assert_eq!(ids(&empty), vec![1, 2, 3, 5], "the empty needle matches every text row and never a null row");
}

#[test]
fn starts_with_and_ends_with_respect_boundaries() {
    let prefix = store().execute_plan(&plan(StringMatchOp::StartsWith, "al")).expect("prefix");
    assert_eq!(ids(&prefix), vec![1]);

    let suffix = store().execute_plan(&plan(StringMatchOp::EndsWith, "ta")).expect("suffix");
    assert_eq!(ids(&suffix), vec![2]);

    let whole = store().execute_plan(&plan(StringMatchOp::StartsWith, "alpha")).expect("whole prefix");
    assert_eq!(ids(&whole), vec![1], "a needle equal to the whole text is both a prefix and a suffix");

    let whole_suffix = store().execute_plan(&plan(StringMatchOp::EndsWith, "alpha")).expect("whole suffix");
    assert_eq!(ids(&whole_suffix), vec![1]);

    let longer = store().execute_plan(&plan(StringMatchOp::StartsWith, "alphabet")).expect("longer prefix");
    assert!(longer.is_empty(), "a needle longer than the text is never a prefix");

    let longer_suffix = store().execute_plan(&plan(StringMatchOp::EndsWith, "alphabet")).expect("longer suffix");
    assert!(longer_suffix.is_empty(), "a needle longer than the text is never a suffix");

    let empty_prefix = store().execute_plan(&plan(StringMatchOp::StartsWith, "")).expect("empty prefix");
    assert_eq!(ids(&empty_prefix), vec![1, 2, 3, 5], "every text including the empty string starts with the empty needle");

    let empty_suffix = store().execute_plan(&plan(StringMatchOp::EndsWith, "")).expect("empty suffix");
    assert_eq!(ids(&empty_suffix), vec![1, 2, 3, 5], "every text including the empty string ends with the empty needle");
}

#[test]
fn matching_is_case_sensitive_and_unicode_code_point_based() {
    let upper = store().execute_plan(&plan(StringMatchOp::Contains, "Alpha")).expect("case");
    assert!(upper.is_empty(), "matching is case-sensitive");

    let unicode = store().execute_plan(&plan(StringMatchOp::Contains, "世界")).expect("unicode");
    assert_eq!(ids(&unicode), vec![5]);

    let unicode_prefix = store().execute_plan(&plan(StringMatchOp::StartsWith, "hello 世")).expect("unicode prefix");
    assert_eq!(ids(&unicode_prefix), vec![5]);
}

#[test]
fn planner_lowers_text_predicates_instead_of_rejecting_them() {
    let contains = store().interpret_source("Note.filter(x => x.title.contains(\"lph\")).collect()").expect("contains");
    assert_eq!(ids(&contains), vec![1]);

    let prefix = store().interpret_source("Note.filter(x => x.title.startsWith(\"be\")).collect()").expect("prefix");
    assert_eq!(ids(&prefix), vec![2]);

    let suffix = store().interpret_source("Note.filter(x => x.title.endsWith(\"ta\")).collect()").expect("suffix");
    assert_eq!(ids(&suffix), vec![2]);

    let combined =
        store().interpret_source("Note.filter(x => x.title.contains(\"a\") && x.title.startsWith(\"al\")).collect()").expect("combined");
    assert_eq!(ids(&combined), vec![1]);
}

#[test]
fn unsupported_predicate_methods_are_plan_rejections() {
    let error = store().interpret_source("Note.filter(x => x.title.matches(\"a\")).collect()").expect_err("matches is unsupported");
    let Error::Diagnostic(diagnostic) = error
    else {
        panic!("expected a structured diagnostic, got {error}");
    };
    assert_eq!(diagnostic.code, "IRIS-PLAN-REJECTED");
    assert!(diagnostic.message.contains("matches"), "message names the rejected method: {}", diagnostic.message);
}

#[test]
fn non_text_fields_are_rejected_with_a_structured_code() {
    let error = poisoned_store().execute_plan(&plan(StringMatchOp::Contains, "a")).expect_err("numeric field must be rejected");
    let Error::Diagnostic(diagnostic) = error
    else {
        panic!("expected a structured diagnostic, got {error}");
    };
    assert_eq!(diagnostic.code, "IRIS-STRING-MATCH-NOT-TEXT");
    assert!(diagnostic.message.contains("`title`"), "message names the offending field: {}", diagnostic.message);
    assert!(diagnostic.message.contains("i64"), "message names the offending value kind: {}", diagnostic.message);
}
