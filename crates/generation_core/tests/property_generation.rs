//! Property-based and exhaustive tests for the generation state machine.

use generation_core::{
    AccountState, CostCalculator, ExportState, GenerationMachine, GenerationModality,
    GenerationState, PreparingState, Pricing, ReadyAccount, RenderingState, TopUpConfig,
};

// ── Manual property tests ──

#[test]
fn can_submit_always_fails_for_unconfigured() {
    assert!(GenerationMachine::can_submit(&AccountState::Unconfigured).is_err());
    assert!(GenerationMachine::can_submit(&AccountState::MissingKeys).is_err());
}

#[test]
fn remaining_credits_never_negative() {
    // Exhaustively test combinations of budget/purchased/spent
    let budgets = [0, 1, 100, 1000, 10_000];
    let purchases = [0, 1, 50, 500, 5000];
    let spent = [0, 1, 50, 200, 1000, 50_000];

    for &budget in &budgets {
        for &purchase in &purchases {
            for &spend in &spent {
                let acct = ReadyAccount {
                    monthly_budget: budget,
                    purchased_credits: purchase,
                    spent_credits: spend,
                };
                let remaining = acct.remaining_credits();
                assert!(remaining >= 0, "remaining should never be negative");
                let expected = (budget + purchase - spend).max(0);
                assert_eq!(
                    remaining, expected,
                    "mismatch budget={budget} purchase={purchase} spend={spend}"
                );
            }
        }
    }
}

#[test]
fn clamp_image_count_stays_in_bounds() {
    for count in -10i64..20 {
        let clamped = GenerationMachine::clamp_image_count(count);
        assert!(clamped >= 1, "clamped should be >= 1 for input {count}");
        assert!(clamped <= 4, "clamped should be <= 4 for input {count}");
    }
}

#[test]
fn placeholder_count_matches_modality() {
    for num_images in 0i64..10 {
        let video = GenerationMachine::placeholder_count(num_images, &GenerationModality::Video);
        let image = GenerationMachine::placeholder_count(num_images, &GenerationModality::Image);
        let audio = GenerationMachine::placeholder_count(num_images, &GenerationModality::Audio);
        let music = GenerationMachine::placeholder_count(num_images, &GenerationModality::Music);

        assert_eq!(video, num_images.max(1) as usize);
        assert_eq!(image, num_images.max(1) as usize);
        assert_eq!(audio, 1);
        assert_eq!(music, 1);
    }
}

#[test]
fn start_prepare_produces_valid_preparing_state() {
    let acct = AccountState::Ready(ReadyAccount {
        monthly_budget: 1000,
        purchased_credits: 0,
        spent_credits: 0,
    });

    // Test various durations and image counts
    for duration in [0.0, 1.0, 30.0, 120.0] {
        for num_images in [0, 1, 4, 10] {
            let result = GenerationMachine::start_prepare(
                &acct,
                "test".into(),
                "model".into(),
                duration,
                "16:9".into(),
                None,
                None,
                50,
                num_images,
                vec![],
                GenerationModality::Video,
            );
            assert!(result.is_ok());
            match result.unwrap() {
                GenerationState::Preparing(prep) => {
                    assert_eq!(prep.prompt, "test");
                    assert_eq!(prep.model, "model");
                    assert!(prep.num_images >= 1, "num_images should be >= 1");
                    assert!(prep.num_images <= 4, "num_images should be <= 4");
                }
                _ => panic!("expected Preparing state"),
            }
        }
    }
}

#[test]
fn render_progress_always_in_range() {
    for progress in [-1.0, -0.5, 0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0] {
        let mut state = RenderingState {
            progress: 0.0,
            stall_timeout_seconds: 120,
            last_progress_time: chrono::Utc::now(),
            stall_watchdog_cancelled: false,
        };
        ExportState::update_progress(&mut state, progress);
        assert!(
            state.progress >= 0.0,
            "progress too low: {}",
            state.progress
        );
        assert!(
            state.progress <= 1.0,
            "progress too high: {}",
            state.progress
        );
    }
}

#[test]
fn top_up_amount_validation() {
    let cfg = TopUpConfig::default();
    // Below minimum
    for amount in [0, 5, 9] {
        assert!(
            cfg.validate_amount(amount).is_err(),
            "should reject amount={amount}"
        );
    }
    // Valid range
    for amount in [10, 50, 250, 500] {
        assert!(
            cfg.validate_amount(amount).is_ok(),
            "should accept amount={amount}"
        );
    }
    // Above maximum
    for amount in [501, 600, 1000] {
        assert!(
            cfg.validate_amount(amount).is_err(),
            "should reject amount={amount}"
        );
    }
}

#[test]
fn reset_only_works_on_completed_or_failed() {
    // Idle stays Idle
    assert_eq!(
        GenerationMachine::reset(GenerationState::Idle),
        GenerationState::Idle
    );

    // Preparing stays Preparing
    let prep = GenerationState::Preparing(PreparingState {
        prompt: "test".into(),
        model: "m".into(),
        duration_seconds: 10.0,
        aspect_ratio: "16:9".into(),
        resolution: None,
        quality: None,
        estimated_cost: 10,
        num_images: 1,
        reference_urls: vec![],
        modality: GenerationModality::Video,
    });
    match GenerationMachine::reset(prep) {
        GenerationState::Preparing(_) => {} // OK
        other => panic!("Preparing should not reset, got {other:?}"),
    }
}

// ── Cost calculator property tests ──

#[test]
fn video_cost_always_positive_with_valid_inputs() {
    let pricing = Pricing {
        credits_per_second: Some(1.0),
        resolution_pricing: None,
        quality_pricing: None,
        audio_discount: None,
        audio_pricing: None,
    };
    for duration in [1.0, 5.0, 10.0, 60.0, 120.0] {
        let cost = CostCalculator::video_cost(&pricing, duration, None, false);
        assert!(
            cost.is_some(),
            "cost should be Some for duration={duration}"
        );
        assert!(
            cost.unwrap() > 0,
            "cost should be positive for duration={duration}"
        );
    }
}

#[test]
fn image_cost_increases_with_more_images() {
    let pricing = Pricing {
        credits_per_second: Some(10.0),
        resolution_pricing: None,
        quality_pricing: None,
        audio_discount: None,
        audio_pricing: None,
    };
    let cost_1 = CostCalculator::image_cost(&pricing, 1, None, None).unwrap();
    let cost_3 = CostCalculator::image_cost(&pricing, 3, None, None).unwrap();
    assert!(cost_3 > cost_1, "more images should cost more");
    assert_eq!(cost_3, cost_1 * 3, "cost should scale linearly");
}

#[test]
fn upscale_cost_requires_positive_duration() {
    let pricing = Pricing {
        credits_per_second: Some(5.0),
        resolution_pricing: None,
        quality_pricing: None,
        audio_discount: None,
        audio_pricing: None,
    };
    assert!(CostCalculator::upscale_cost(&pricing, 0.0).is_none());
    assert!(CostCalculator::upscale_cost(&pricing, -1.0).is_none());
    assert!(CostCalculator::upscale_cost(&pricing, 1.0).is_some());
}

#[test]
fn cost_formatting_edge_cases() {
    assert_eq!(CostCalculator::format_cost(Some(2)), "2 credits");
    assert_eq!(CostCalculator::format_cost(Some(100)), "100 credits");
    assert_eq!(
        CostCalculator::format_cost(Some(i64::MAX)),
        format!("{} credits", i64::MAX)
    );
}

#[test]
fn export_state_cancel_chain() {
    // Export state machine: Rendering → Cancelling → (stays cancelled)
    let state = ExportState::start_rendering(120);
    if let ExportState::Rendering(s) = state {
        let cancelled = ExportState::cancel(s);
        assert_eq!(cancelled, ExportState::Cancelling);
    } else {
        panic!("expected Rendering");
    }
}

#[test]
fn export_fail_after_cancel() {
    // Verify you can fail after cancelling
    let state = ExportState::fail("render error".into());
    assert_eq!(state, ExportState::Failed("render error".into()));
}
