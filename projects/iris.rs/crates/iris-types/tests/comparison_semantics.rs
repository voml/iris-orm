//! Formal comparison semantics: `file` is content-hash identity and `vector`
//! comparison is similarity under an explicit query-side metric.

use iris_types::{FileDescriptor, FileManifestRef, SimilarityMetric, Value, VectorSimilarityError, VectorValue, similarity_meets_threshold};

fn digest(byte: u8) -> String {
    std::iter::repeat_n(format!("{byte:02x}"), 32).collect()
}

fn file_value(digest: &str, name: Option<&str>, media_type: Option<&str>) -> Value {
    Value::File(
        FileDescriptor::new(
            digest.to_owned(),
            4,
            name.map(str::to_owned),
            media_type.map(str::to_owned),
            FileManifestRef { hash: digest.to_owned(), size: 4 },
        )
        .expect("descriptor"),
    )
}

/// Two files are the same file exactly when their content digests are equal;
/// metadata labels never participate in the comparison.
#[test]
fn file_equality_is_content_hash_equality() {
    let left = file_value(&digest(0xaa), Some("report.pdf"), Some("application/pdf"));
    let same_content = file_value(&digest(0xaa), Some("renamed.bin"), None);
    let other_content = file_value(&digest(0xbb), Some("report.pdf"), Some("application/pdf"));

    assert_eq!(left.cmp(&same_content), std::cmp::Ordering::Equal, "equal digests are the same file");
    assert_ne!(left.cmp(&other_content), std::cmp::Ordering::Equal, "distinct digests are distinct files");
    assert_eq!(left.cmp(&other_content), std::cmp::Ordering::Less, "ordering follows the digest");
}

fn vector(components: &[f32]) -> VectorValue {
    VectorValue::new(components.len() as u32, components.to_vec()).expect("vector")
}

#[test]
fn cosine_similarity_matches_known_angles() {
    let east = vector(&[1.0, 0.0]);
    let north = vector(&[0.0, 1.0]);
    let east_again = vector(&[2.0, 0.0]);
    let west = vector(&[-1.0, 0.0]);

    let orthogonal = east.similarity(&north, SimilarityMetric::Cosine).expect("cosine");
    assert!(orthogonal.abs() < 1e-12, "orthogonal vectors score 0, got {orthogonal}");
    let parallel = east.similarity(&east_again, SimilarityMetric::Cosine).expect("cosine");
    assert!((parallel - 1.0).abs() < 1e-12, "scaled parallels score 1, got {parallel}");
    let opposite = east.similarity(&west, SimilarityMetric::Cosine).expect("cosine");
    assert!((opposite + 1.0).abs() < 1e-12, "opposites score -1, got {opposite}");
}

#[test]
fn dot_product_and_euclidean_are_exact_on_known_pairs() {
    let left = vector(&[1.0, 2.0]);
    let right = vector(&[3.0, 4.0]);
    let dot = left.similarity(&right, SimilarityMetric::DotProduct).expect("dot");
    assert_eq!(dot, 11.0);
    let origin = vector(&[0.0, 0.0]);
    let corner = vector(&[3.0, 4.0]);
    let distance = origin.similarity(&corner, SimilarityMetric::Euclidean).expect("euclidean");
    assert_eq!(distance, 5.0, "3-4-5 triangle distance");
}

/// Thresholds are inclusive boundaries: exactly-at-threshold passes, one step
/// past it fails, and the direction flips between scores and distances.
#[test]
fn threshold_boundaries_are_inclusive_and_metric_directed() {
    assert!(similarity_meets_threshold(SimilarityMetric::Cosine, 0.8, 0.8));
    assert!(!similarity_meets_threshold(SimilarityMetric::Cosine, 0.79999, 0.8));
    assert!(similarity_meets_threshold(SimilarityMetric::DotProduct, 11.0, 11.0));
    assert!(!similarity_meets_threshold(SimilarityMetric::DotProduct, 10.9999, 11.0));
    assert!(similarity_meets_threshold(SimilarityMetric::Euclidean, 5.0, 5.0));
    assert!(!similarity_meets_threshold(SimilarityMetric::Euclidean, 5.00001, 5.0));
}

#[test]
fn dimension_mismatch_is_rejected_for_every_metric() {
    let pair = vector(&[1.0, 2.0]);
    let triple = vector(&[1.0, 2.0, 3.0]);
    for metric in [SimilarityMetric::Cosine, SimilarityMetric::DotProduct, SimilarityMetric::Euclidean] {
        assert_eq!(pair.similarity(&triple, metric), Err(VectorSimilarityError::DimensionMismatch), "{metric:?}");
    }
}

#[test]
fn cosine_rejects_zero_norm_operands() {
    let zero = vector(&[0.0, 0.0]);
    let some = vector(&[1.0, 0.0]);
    assert_eq!(zero.similarity(&some, SimilarityMetric::Cosine), Err(VectorSimilarityError::ZeroNorm));
    assert_eq!(some.similarity(&zero, SimilarityMetric::Cosine), Err(VectorSimilarityError::ZeroNorm));
    // Dot product and distance stay defined for a zero operand.
    assert_eq!(zero.similarity(&some, SimilarityMetric::DotProduct), Ok(0.0));
    assert_eq!(zero.similarity(&some, SimilarityMetric::Euclidean), Ok(1.0));
}
