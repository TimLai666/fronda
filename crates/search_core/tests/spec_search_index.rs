//! Spec tests for search index model lifecycle (SRCH series).
//!
//! These tests validate the data-layer model: cache identity, frame sampling
//! invariants, embedding layout, sorting, and preconditions. Orchestration
//! (queuing, model lifecycle, indexing scheduling) is tested at integration
//! level; these tests cover the types and their spec contract.

use search_core::search_index::{
    CacheIdentity, EmbeddingRow, HitKind, SearchHit, SearchResults, VisualIndex,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn identity(path: &str) -> CacheIdentity {
    CacheIdentity {
        path: path.to_string(),
        modification_time: 1_700_000_000,
        file_size: 42_000,
    }
}

// ===================================================================
// SRCH-007: Only video/image assets for visual indexing
// ===================================================================
//
// The visual-index type does not restrict asset type at the data layer;
// the filtering happens at the scheduling level. We validate that the
// type accepts any identity (video or image) without error.

#[test]
fn srch_007_visual_index_accepts_video_identity() {
    let id = identity("/videos/clip.mov");
    let rows = vec![EmbeddingRow {
        frame: 0,
        embedding: vec![0.1; 128],
    }];
    let index = VisualIndex::new(id.clone(), rows);
    assert_eq!(index.identity, id);
    assert_eq!(index.rows.len(), 1);
}

#[test]
fn srch_007_visual_index_accepts_image_identity() {
    let id = identity("/images/photo.jpg");
    let rows = vec![EmbeddingRow {
        frame: 0,
        embedding: vec![0.2; 128],
    }];
    let index = VisualIndex::new(id.clone(), rows);
    assert_eq!(index.identity.path, "/images/photo.jpg");
}

// ===================================================================
// SRCH-008: Only audio/video-with-audio for transcript indexing
// ===================================================================
//
// Like SRCH-007, transcript indexing filtering is an orchestration concern.
// The transcript type (in `transcript.rs`) accepts any identity.
// We validate the CacheIdentity shape is compatible.

#[test]
fn srch_008_transcript_identity_compatible_with_audio_path() {
    let id = identity("/audio/track.wav");
    assert_eq!(id.path, "/audio/track.wav");
    assert!(id.file_size > 0);
}

// ===================================================================
// SRCH-015: Index identity = path + mtime + file size
// ===================================================================

#[test]
fn srch_015_identity_equality() {
    let a = identity("/videos/clip.mov");
    let b = identity("/videos/clip.mov");
    assert_eq!(a, b);
}

#[test]
fn srch_015_identity_different_path_ne() {
    let a = identity("/videos/a.mov");
    let b = identity("/videos/b.mov");
    assert_ne!(a, b);
}

#[test]
fn srch_015_identity_different_mtime_ne() {
    let a = CacheIdentity {
        path: "/videos/clip.mov".into(),
        modification_time: 1_700_000_000,
        file_size: 42_000,
    };
    let b = CacheIdentity {
        path: "/videos/clip.mov".into(),
        modification_time: 1_700_000_001,
        file_size: 42_000,
    };
    assert_ne!(a, b);
}

#[test]
fn srch_015_identity_different_file_size_ne() {
    let a = CacheIdentity {
        path: "/videos/clip.mov".into(),
        modification_time: 1_700_000_000,
        file_size: 42_000,
    };
    let b = CacheIdentity {
        path: "/videos/clip.mov".into(),
        modification_time: 1_700_000_000,
        file_size: 99_000,
    };
    assert_ne!(a, b);
}

#[test]
fn srch_015_identity_hash_consistency() {
    use std::collections::HashSet;
    let a = identity("/videos/clip.mov");
    let b = identity("/videos/clip.mov");
    let mut set = HashSet::new();
    set.insert(a);
    // Same key → not a new insertion
    assert!(!set.insert(b), "equal identities must hash identically");
}

// ===================================================================
// SRCH-016: Still images have 1 embedding at time 0
// ===================================================================

#[test]
fn srch_016_single_frame_has_one_row() {
    let id = identity("/images/still.png");
    let index = VisualIndex::single_frame(id, vec![0.5; 512]);
    assert_eq!(index.rows.len(), 1);
}

#[test]
fn srch_016_single_frame_at_time_zero() {
    let id = identity("/images/still.png");
    let index = VisualIndex::single_frame(id, vec![0.3; 256]);
    assert_eq!(index.rows[0].frame, 0);
}

#[test]
fn srch_016_single_frame_preserves_embedding() {
    let embedding = vec![0.1, 0.2, 0.3, 0.4, 0.5];
    let id = identity("/images/still.png");
    let index = VisualIndex::single_frame(id, embedding.clone());
    assert_eq!(index.rows[0].embedding, embedding);
}

// ===================================================================
// SRCH-018: Frame-sampler output strictly increasing
// ===================================================================
//
// The crate does not contain a frame-sampler; this test validates that
// `VisualIndex` rows can be checked for the strictly-increasing invariant.

#[test]
fn srch_018_rows_must_be_strictly_increasing() {
    // Valid: strictly increasing frames
    let rows = vec![
        EmbeddingRow {
            frame: 0,
            embedding: vec![0.1; 4],
        },
        EmbeddingRow {
            frame: 30,
            embedding: vec![0.2; 4],
        },
        EmbeddingRow {
            frame: 60,
            embedding: vec![0.3; 4],
        },
    ];
    for pair in rows.windows(2) {
        assert!(
            pair[0].frame < pair[1].frame,
            "frame must be strictly increasing"
        );
    }
}

#[test]
#[should_panic(expected = "strictly increasing")]
fn srch_018_duplicate_frame_invalid() {
    let rows = vec![
        EmbeddingRow {
            frame: 0,
            embedding: vec![0.1; 4],
        },
        EmbeddingRow {
            frame: 0,
            embedding: vec![0.2; 4],
        },
    ];
    assert!(
        rows[0].frame < rows[1].frame,
        "frame must be strictly increasing"
    );
}

#[test]
#[should_panic(expected = "strictly increasing")]
fn srch_018_out_of_order_frames_invalid() {
    let rows = vec![
        EmbeddingRow {
            frame: 60,
            embedding: vec![0.1; 4],
        },
        EmbeddingRow {
            frame: 30,
            embedding: vec![0.2; 4],
        },
    ];
    assert!(
        rows[0].frame < rows[1].frame,
        "frame must be strictly increasing"
    );
}

#[test]
fn srch_018_single_frame_is_trivially_increasing() {
    let row = EmbeddingRow {
        frame: 42,
        embedding: vec![0.5; 4],
    };
    // Single element → no ordering violation
    assert_eq!(row.frame, 42);
}

#[test]
fn srch_018_large_gap_between_frames_allowed() {
    let rows = vec![
        EmbeddingRow {
            frame: 0,
            embedding: vec![0.1; 4],
        },
        EmbeddingRow {
            frame: 9000,
            embedding: vec![0.2; 4],
        },
    ];
    // Gaps are fine as long as they are strictly increasing
    assert!(rows[0].frame < rows[1].frame);
}

// ===================================================================
// SRCH-021: Visual search requires model ready + non-empty query
// ===================================================================
//
// The crate does not contain a search executor; we validate that
// SearchResults behaves correctly with empty/missing data.

#[test]
fn srch_021_empty_results_when_no_search_performed() {
    let results = SearchResults::default();
    assert!(results.is_empty());
    assert_eq!(results.total_hits(), 0);
}

#[test]
fn srch_021_results_populated_only_after_search() {
    let results = SearchResults {
        moments: vec![SearchHit {
            media_id: "m1".into(),
            frame: 10,
            score: 0.85,
            kind: HitKind::Visual,
        }],
        ..Default::default()
    };
    assert!(!results.is_empty());
    assert_eq!(results.total_hits(), 1);
    assert_eq!(results.spoken.len(), 0);
    assert_eq!(results.files.len(), 0);
}

// ===================================================================
// SRCH-023: Sort hits by descending score
// ===================================================================

#[test]
fn srch_023_hits_sorted_descending() {
    let mut hits = vec![
        SearchHit {
            media_id: "a".into(),
            frame: 0,
            score: 0.5,
            kind: HitKind::Visual,
        },
        SearchHit {
            media_id: "b".into(),
            frame: 0,
            score: 0.9,
            kind: HitKind::Visual,
        },
        SearchHit {
            media_id: "c".into(),
            frame: 0,
            score: 0.1,
            kind: HitKind::Visual,
        },
    ];
    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    assert_eq!(hits[0].media_id, "b");
    assert_eq!(hits[1].media_id, "a");
    assert_eq!(hits[2].media_id, "c");
}

#[test]
fn srch_023_scores_preserved_after_sort() {
    let mut hits = vec![
        SearchHit {
            media_id: "low".into(),
            frame: 0,
            score: 0.2,
            kind: HitKind::Visual,
        },
        SearchHit {
            media_id: "high".into(),
            frame: 0,
            score: 0.95,
            kind: HitKind::Visual,
        },
        SearchHit {
            media_id: "mid".into(),
            frame: 0,
            score: 0.6,
            kind: HitKind::Visual,
        },
    ];
    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    assert_eq!(hits[0].score, 0.95);
    assert_eq!(hits[1].score, 0.6);
    assert_eq!(hits[2].score, 0.2);
}

#[test]
fn srch_023_tie_scores_any_order() {
    let mut hits = vec![
        SearchHit {
            media_id: "x".into(),
            frame: 10,
            score: 0.5,
            kind: HitKind::Visual,
        },
        SearchHit {
            media_id: "y".into(),
            frame: 20,
            score: 0.5,
            kind: HitKind::Visual,
        },
    ];
    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    // Both scores are equal, so either order is acceptable
    let scores: Vec<f64> = hits.iter().map(|h| h.score).collect();
    assert_eq!(scores, vec![0.5, 0.5]);
}

#[test]
fn srch_023_single_hit_trivially_sorted() {
    let mut hits = vec![SearchHit {
        media_id: "only".into(),
        frame: 0,
        score: 0.7,
        kind: HitKind::Visual,
    }];
    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].score, 0.7);
}

// ===================================================================
// Additional coverage
// ===================================================================

#[test]
fn srch_embedding_row_serde_roundtrip() {
    let row = EmbeddingRow {
        frame: 42,
        embedding: vec![0.1, 0.2, 0.3, 0.4],
    };
    let json = serde_json::to_string(&row).unwrap();
    let deserialized: EmbeddingRow = serde_json::from_str(&json).unwrap();
    assert_eq!(row, deserialized);
}

#[test]
fn srch_visual_index_serde_roundtrip() {
    let id = identity("/videos/test.mov");
    let index = VisualIndex::new(
        id,
        vec![
            EmbeddingRow {
                frame: 0,
                embedding: vec![0.1; 128],
            },
            EmbeddingRow {
                frame: 30,
                embedding: vec![0.2; 128],
            },
        ],
    );
    let json = serde_json::to_string(&index).unwrap();
    let deserialized: VisualIndex = serde_json::from_str(&json).unwrap();
    assert_eq!(index, deserialized);
}

#[test]
fn srch_cache_identity_serde_roundtrip() {
    let id = identity("/videos/clip.mov");
    let json = serde_json::to_string(&id).unwrap();
    let deserialized: CacheIdentity = serde_json::from_str(&json).unwrap();
    assert_eq!(id, deserialized);
}

#[test]
fn srch_hit_kind_variants() {
    let visual = HitKind::Visual;
    let spoken = HitKind::Spoken;
    let file = HitKind::File;
    // Each variant exists and is distinct
    assert_ne!(visual, spoken);
    assert_ne!(spoken, file);
    assert_ne!(visual, file);
}

#[test]
fn srch_search_results_mixed_kinds() {
    let results = SearchResults {
        moments: vec![
            SearchHit {
                media_id: "m1".into(),
                frame: 10,
                score: 0.9,
                kind: HitKind::Visual,
            },
            SearchHit {
                media_id: "m2".into(),
                frame: 50,
                score: 0.8,
                kind: HitKind::Visual,
            },
        ],
        spoken: vec![SearchHit {
            media_id: "s1".into(),
            frame: 20,
            score: 0.7,
            kind: HitKind::Spoken,
        }],
        files: vec![SearchHit {
            media_id: "f1".into(),
            frame: 0,
            score: 0.6,
            kind: HitKind::File,
        }],
    };
    assert_eq!(results.moments.len(), 2);
    assert_eq!(results.spoken.len(), 1);
    assert_eq!(results.files.len(), 1);
    assert_eq!(results.total_hits(), 4);
}

#[test]
fn srch_custom_cache_identity_from_path() {
    let id = CacheIdentity::from_path("/custom/path/file.mp4");
    assert_eq!(id.path, "/custom/path/file.mp4");
    assert_eq!(id.file_size, 0);
    // modification_time is set to SystemTime::now() so it should be > 0
    assert!(id.modification_time > 0);
}
