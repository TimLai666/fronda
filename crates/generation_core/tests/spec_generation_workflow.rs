//! Integration test for the end-to-end generation workflow (GEN-004 through GEN-024).
//!
//! Tests the full state machine lifecycle:
//!   Idle → Preparing → Uploading → AwaitingJob → Downloading → Completed

use generation_core::{
    AccountState, GenerationMachine, GenerationModality, GenerationSnapshot, ReadyAccount,
};

fn make_ready_account() -> ReadyAccount {
    ReadyAccount {
        monthly_budget: 1000,
        purchased_credits: 0,
        spent_credits: 0,
    }
}

fn make_snapshot_for_verify() -> GenerationSnapshot {
    // Minimal snapshot to verify fields are preserved through the workflow
    GenerationSnapshot {
        prompt: "workflow test".into(),
        model: "m1".into(),
        duration_seconds: 10.0,
        aspect_ratio: "16:9".into(),
        resolution: None,
        quality: None,
        num_images: 2,
        reference_urls: vec![],
        reference_asset_ids: vec![],
        modality: GenerationModality::Video,
        created_at: chrono::Utc::now(),
    }
}

#[test]
fn spec_generation_workflow_full_lifecycle() {
    // ── 1. Start from Idle → transition to Preparing ──
    let acct = make_ready_account();
    let prep = GenerationMachine::start_prepare(
        &AccountState::Ready(acct),
        "generate a cinematic scene".into(),
        "m1".into(),
        10.0,
        "16:9".into(),
        None,   // resolution
        None,   // quality
        50,     // estimated_cost
        2,      // num_images (2 placeholders)
        vec![], // reference_urls
        GenerationModality::Video,
    )
    .expect("start_prepare should succeed with a Ready account");

    let preparing = match prep {
        generation_core::GenerationState::Preparing(s) => s,
        other => panic!("expected Preparing state, got {other:?}"),
    };
    assert_eq!(preparing.prompt, "generate a cinematic scene");
    assert_eq!(preparing.model, "m1");
    assert_eq!(preparing.num_images, 2);
    assert_eq!(preparing.estimated_cost, 50);
    assert_eq!(preparing.modality, GenerationModality::Video);

    // ── 2. Start uploading → should transition to Uploading state ──
    let acct2 = make_ready_account();
    let upload = GenerationMachine::start_uploading(
        preparing,
        &acct2,
        vec![], // local_paths
        vec![], // pre_uploaded
    )
    .expect("start_uploading should pass credit check");

    let uploading = match upload {
        generation_core::GenerationState::Uploading(s) => s,
        other => panic!("expected Uploading state, got {other:?}"),
    };
    assert!(uploading.pending_uploads.is_empty());
    assert!(uploading.completed_uploads.is_empty());
    assert_eq!(uploading.snapshot.prompt, "generate a cinematic scene");
    assert_eq!(uploading.snapshot.num_images, 2);

    // ── 3. Submit job → should transition to AwaitingJob state ──
    let placeholder_ids: Vec<String> = (0..uploading.snapshot.num_images)
        .map(|i| format!("ph-{i}"))
        .collect();
    let submitted = GenerationMachine::submit_job(uploading, "job-42".into(), placeholder_ids);

    let awaiting = match submitted {
        generation_core::GenerationState::AwaitingJob(s) => s,
        other => panic!("expected AwaitingJob state, got {other:?}"),
    };
    assert_eq!(awaiting.job_id, "job-42");
    assert_eq!(
        awaiting.placeholder_ids,
        vec!["ph-0".to_string(), "ph-1".to_string()]
    );
    assert_eq!(awaiting.snapshot.prompt, "generate a cinematic scene");

    // ── 4. Job succeeds → should transition to Downloading state ──
    let result_urls = vec![
        "https://cdn.example.com/result0.mp4".into(),
        "https://cdn.example.com/result1.mp4".into(),
    ];
    let downloading = GenerationMachine::job_succeeded(awaiting, result_urls);

    let mut dl = match downloading {
        generation_core::GenerationState::Downloading(s) => s,
        other => panic!("expected Downloading state, got {other:?}"),
    };
    assert_eq!(dl.result_urls.len(), 2);
    assert_eq!(dl.completed_downloads.len(), 0);
    assert_eq!(dl.failed_downloads.len(), 0);
    assert_eq!(dl.job_id, "job-42");

    // ── 5. Mark downloads complete → transition to Completed state ──
    GenerationMachine::mark_download_complete(&mut dl, "asset-0".into());
    GenerationMachine::mark_download_complete(&mut dl, "asset-1".into());

    let completed = GenerationMachine::finalize_completed(dl);

    let final_state = match completed {
        generation_core::GenerationState::Completed(s) => s,
        other => panic!("expected Completed state, got {other:?}"),
    };

    // ── 6. Verify final state has the expected asset IDs ──
    assert_eq!(final_state.final_asset_ids, vec!["asset-0", "asset-1"]);
    assert_eq!(
        GenerationMachine::first_successful_asset(&final_state),
        Some("asset-0")
    );
    assert_eq!(final_state.snapshot.prompt, "generate a cinematic scene");
    assert_eq!(final_state.snapshot.num_images, 2);
}

#[test]
fn spec_generation_workflow_upload_order() {
    // Verify that reference upload order is preserved in the workflow.
    let acct = make_ready_account();
    let prep_state = GenerationMachine::start_prepare(
        &AccountState::Ready(acct),
        "multi-reference".into(),
        "m1".into(),
        10.0,
        "16:9".into(),
        None,
        None,
        50,
        1,
        vec![
            "https://ref1.com/a.mp4".into(),
            "https://ref2.com/b.mp4".into(),
        ],
        GenerationModality::Video,
    )
    .expect("start_prepare");
    let prep = match prep_state {
        generation_core::GenerationState::Preparing(s) => s,
        other => panic!("expected Preparing, got {other:?}"),
    };

    let acct2 = make_ready_account();
    // Provide paths in reverse index order
    let local_paths = vec![
        (2usize, "/tmp/last.mp4".into()),
        (0usize, "/tmp/first.mp4".into()),
        (1usize, "/tmp/middle.mp4".into()),
    ];
    let upload = GenerationMachine::start_uploading(prep, &acct2, local_paths, vec![])
        .expect("start_uploading");
    let uploading = match upload {
        generation_core::GenerationState::Uploading(s) => s,
        other => panic!("expected Uploading, got {other:?}"),
    };

    // Order must be sorted by target_index
    let paths: Vec<&str> = uploading
        .pending_uploads
        .iter()
        .map(|u| u.local_path.as_str())
        .collect();
    assert_eq!(
        paths,
        vec!["/tmp/first.mp4", "/tmp/middle.mp4", "/tmp/last.mp4"]
    );
}

#[test]
fn spec_generation_workflow_credit_block() {
    // Verify that insufficient credits block the workflow at upload time.
    let broke = ReadyAccount {
        monthly_budget: 10,
        purchased_credits: 0,
        spent_credits: 0,
    };
    let prep_state = GenerationMachine::start_prepare(
        &AccountState::Ready(broke.clone()),
        "expensive".into(),
        "m1".into(),
        10.0,
        "16:9".into(),
        None,
        None,
        100, // estimated_cost exceeds remaining 10
        1,
        vec![],
        GenerationModality::Video,
    )
    .expect("start_prepare should pass (credit check is at upload time)");
    let prep = match prep_state {
        generation_core::GenerationState::Preparing(s) => s,
        other => panic!("expected Preparing, got {other:?}"),
    };

    let err = GenerationMachine::start_uploading(prep, &broke, vec![], vec![])
        .expect_err("should fail with insufficient credits");
    assert!(err.contains("exceeds remaining credits"));
}

#[test]
fn spec_generation_workflow_download_failure() {
    // Verify that download failures surface correctly.
    let acct = make_ready_account();
    let prep_state = GenerationMachine::start_prepare(
        &AccountState::Ready(acct),
        "download test".into(),
        "m1".into(),
        10.0,
        "16:9".into(),
        None,
        None,
        10,
        2,
        vec![],
        GenerationModality::Video,
    )
    .expect("start_prepare");
    let prep = match prep_state {
        generation_core::GenerationState::Preparing(s) => s,
        other => panic!("expected Preparing, got {other:?}"),
    };

    let acct2 = make_ready_account();
    let upload =
        GenerationMachine::start_uploading(prep, &acct2, vec![], vec![]).expect("start_uploading");
    let uploading = match upload {
        generation_core::GenerationState::Uploading(s) => s,
        other => panic!("expected Uploading, got {other:?}"),
    };

    let submitted = GenerationMachine::submit_job(
        uploading,
        "job-dl".into(),
        vec!["ph-0".into(), "ph-1".into()],
    );
    let awaiting = match submitted {
        generation_core::GenerationState::AwaitingJob(s) => s,
        other => panic!("expected AwaitingJob, got {other:?}"),
    };

    let downloading = GenerationMachine::job_succeeded(
        awaiting,
        vec!["https://dl.example.com/result0.mp4".into()],
    );
    let mut dl = match downloading {
        generation_core::GenerationState::Downloading(s) => s,
        other => panic!("expected Downloading, got {other:?}"),
    };

    // Mark one download as failed (GEN-017)
    GenerationMachine::mark_download_failed(&mut dl, "https://dl.example.com/result0.mp4".into());
    assert_eq!(dl.failed_downloads.len(), 1);

    // Finalize with errors
    let completed_with_errors = GenerationMachine::finalize_completed_with_errors(dl);
    match completed_with_errors {
        generation_core::GenerationState::CompletedWithErrors(s) => {
            assert_eq!(s.final_asset_ids.len(), 0);
            assert_eq!(s.pending_download_urls.len(), 1);
            assert!(s.pending_download_urls[0].contains("result0"));
        }
        other => panic!("expected CompletedWithErrors, got {other:?}"),
    }
}

#[test]
fn spec_generation_workflow_fatal_failure() {
    // Verify a fatal failure transitions to Failed state.
    let failed = GenerationMachine::fail("Backend API returned 500".into(), None, vec![]);
    match failed {
        generation_core::GenerationState::Failed(s) => {
            assert!(s.reason.contains("Backend API"));
            assert!(s.pending_retry_urls.is_empty());
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn spec_generation_workflow_reset() {
    // Verify reset transitions back to Idle from Completed/Failed.
    let idle = generation_core::GenerationState::Idle;
    assert_eq!(
        GenerationMachine::reset(idle),
        generation_core::GenerationState::Idle
    );

    let completed = generation_core::GenerationState::Completed(generation_core::CompletedState {
        final_asset_ids: vec![],
        snapshot: make_snapshot_for_verify(),
    });
    assert_eq!(
        GenerationMachine::reset(completed),
        generation_core::GenerationState::Idle
    );

    let failed = generation_core::GenerationState::Failed(generation_core::FailedState {
        reason: "oops".into(),
        snapshot: None,
        pending_retry_urls: vec![],
    });
    assert_eq!(
        GenerationMachine::reset(failed),
        generation_core::GenerationState::Idle
    );
}
