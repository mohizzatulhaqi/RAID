use serde::Serialize;

/// 100% in basis points (1 bp = 0.01%).
pub const TOTAL_BP: u32 = 10_000;
/// Share of HP carried by milestones (70%).
pub const MILESTONE_TOTAL_BP: u32 = 7_000;
pub const BUILD_BP: u32 = 1_000;
pub const TEST_BP: u32 = 1_500;
pub const LINT_BP: u32 = 500;

/// Latest evidence for one verification check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckInput {
    /// No command configured for this check.
    NotSet,
    /// Configured, but no run recorded yet.
    NotRun,
    /// Latest recorded run. Only `passed && exit_code == Some(0)` counts.
    Ran {
        passed: bool,
        exit_code: Option<i32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatestVerification {
    pub build: CheckInput,
    pub test: CheckInput,
    pub lint: CheckInput,
}

impl LatestVerification {
    pub const fn not_run() -> Self {
        Self {
            build: CheckInput::NotRun,
            test: CheckInput::NotRun,
            lint: CheckInput::NotRun,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Passed,
    Failed,
    NotRun,
    NotSet,
}

impl From<CheckInput> for CheckState {
    fn from(input: CheckInput) -> Self {
        match input {
            CheckInput::NotSet => CheckState::NotSet,
            CheckInput::NotRun => CheckState::NotRun,
            CheckInput::Ran {
                passed: true,
                exit_code: Some(0),
            } => CheckState::Passed,
            CheckInput::Ran { .. } => CheckState::Failed,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MilestoneSegment {
    pub weight_bp: u32,
    pub proven: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckSegment {
    pub weight_bp: u32,
    pub state: CheckState,
}

#[derive(Debug, Clone, Serialize)]
pub struct BossHp {
    pub remaining_bp: u32,
    pub milestone_bp: u32,
    pub verification_bp: u32,
    pub defeated: bool,
    pub milestones_done: u32,
    pub milestones_total: u32,
    pub milestones: Vec<MilestoneSegment>,
    pub build: CheckSegment,
    pub test: CheckSegment,
    pub lint: CheckSegment,
    /// Plain-text explanation of the bar, built here so the UI never derives it.
    pub label: String,
    /// What to do next, in plain words. Built here so the UI never derives it.
    pub next_step: String,
}

/// Splits `MILESTONE_TOTAL_BP` over `total` milestones; the remainder is
/// spread one bp at a time so the weights always sum to exactly 70%.
fn milestone_weights(total: u32) -> Vec<u32> {
    let base = MILESTONE_TOTAL_BP / total;
    let extra = MILESTONE_TOTAL_BP % total;
    (0..total).map(|i| base + u32::from(i < extra)).collect()
}

fn check_phrase(name: &str, state: CheckState) -> String {
    match state {
        CheckState::Passed => format!("{name} passed"),
        CheckState::Failed => format!("{name} failed"),
        CheckState::NotRun => format!("{name} pending"),
        CheckState::NotSet => format!("{name} not set"),
    }
}

fn next_step(done: u32, total: u32, defeated: bool, checks: &[(&str, CheckState)]) -> String {
    let names = |wanted: CheckState| -> Vec<&str> {
        checks
            .iter()
            .filter(|(_, st)| *st == wanted)
            .map(|(n, _)| *n)
            .collect()
    };
    let not_set = names(CheckState::NotSet);
    let failed = names(CheckState::Failed);
    let not_run = names(CheckState::NotRun);

    if defeated {
        "All evidence is in. The boss is defeated.".to_string()
    } else if done < total && not_run.len() == checks.len() {
        "Start the task. Evidence appears here as checks pass.".to_string()
    } else if !failed.is_empty() {
        format!(
            "Failed: {}. Open Checks to see why, then run them again.",
            failed.join(", ")
        )
    } else if !not_run.is_empty() {
        format!("Run checks to get evidence for: {}.", not_run.join(", "))
    } else if !not_set.is_empty() {
        format!(
            "No command set for: {}. Add one in Checks, or the boss cannot be defeated.",
            not_set.join(", ")
        )
    } else if done < total {
        "Checks pass. The task still needs to be marked verified.".to_string()
    } else {
        "Run checks to finish the fight.".to_string()
    }
}

/// `milestones_total` below 1 is treated as one implicit milestone.
/// `milestones_done` is clamped to the total.
pub fn compute_boss_hp(
    milestones_done: u32,
    milestones_total: u32,
    latest: &LatestVerification,
) -> BossHp {
    let total = milestones_total.max(1);
    let done = milestones_done.min(total);
    let weights = milestone_weights(total);

    let milestones: Vec<MilestoneSegment> = weights
        .iter()
        .zip(0u32..)
        .map(|(&weight_bp, index)| MilestoneSegment {
            weight_bp,
            proven: index < done,
        })
        .collect();
    let milestone_bp: u32 = milestones
        .iter()
        .filter(|m| m.proven)
        .map(|m| m.weight_bp)
        .sum();

    let build = CheckSegment {
        weight_bp: BUILD_BP,
        state: latest.build.into(),
    };
    let test = CheckSegment {
        weight_bp: TEST_BP,
        state: latest.test.into(),
    };
    let lint = CheckSegment {
        weight_bp: LINT_BP,
        state: latest.lint.into(),
    };
    let verification_bp: u32 = [&build, &test, &lint]
        .iter()
        .filter(|c| c.state == CheckState::Passed)
        .map(|c| c.weight_bp)
        .sum();

    let remaining_bp = TOTAL_BP - milestone_bp - verification_bp;
    let all_passed = [&build, &test, &lint]
        .iter()
        .all(|c| c.state == CheckState::Passed);
    let defeated = remaining_bp == 0 && all_passed;

    let label = if defeated {
        "Boss defeated: build, test and lint passed".to_string()
    } else {
        format!(
            "Milestone {done} of {total} done, {}, {}, {}",
            check_phrase("build", build.state),
            check_phrase("test", test.state),
            check_phrase("lint", lint.state),
        )
    };

    let next_step = next_step(
        done,
        total,
        defeated,
        &[
            ("build", build.state),
            ("test", test.state),
            ("lint", lint.state),
        ],
    );

    BossHp {
        remaining_bp,
        milestone_bp,
        verification_bp,
        defeated,
        milestones_done: done,
        milestones_total: total,
        milestones,
        build,
        test,
        lint,
        label,
        next_step,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASS: CheckInput = CheckInput::Ran {
        passed: true,
        exit_code: Some(0),
    };
    const FAIL: CheckInput = CheckInput::Ran {
        passed: false,
        exit_code: Some(1),
    };

    fn all(input: CheckInput) -> LatestVerification {
        LatestVerification {
            build: input,
            test: input,
            lint: input,
        }
    }

    #[test]
    fn no_evidence_is_full_hp() {
        let hp = compute_boss_hp(0, 4, &LatestVerification::not_run());
        assert_eq!(hp.remaining_bp, 10_000);
        assert!(!hp.defeated);
    }

    #[test]
    fn all_milestones_without_verification_leaves_30_percent() {
        let hp = compute_boss_hp(4, 4, &LatestVerification::not_run());
        assert_eq!(hp.remaining_bp, 3_000);
        assert!(!hp.defeated);
    }

    #[test]
    fn all_verification_without_milestones_leaves_70_percent() {
        let hp = compute_boss_hp(0, 4, &all(PASS));
        assert_eq!(hp.remaining_bp, 7_000);
        assert!(!hp.defeated);
    }

    #[test]
    fn everything_proven_is_defeat() {
        let hp = compute_boss_hp(4, 4, &all(PASS));
        assert_eq!(hp.remaining_bp, 0);
        assert!(hp.defeated);
        assert_eq!(hp.label, "Boss defeated: build, test and lint passed");
    }

    #[test]
    fn failing_exit_code_blocks_defeat() {
        let mut latest = all(PASS);
        latest.lint = FAIL;
        let hp = compute_boss_hp(4, 4, &latest);
        assert_eq!(hp.remaining_bp, 500);
        assert!(!hp.defeated);
    }

    #[test]
    fn passed_status_with_nonzero_exit_code_does_not_count() {
        let mut latest = all(PASS);
        latest.test = CheckInput::Ran {
            passed: true,
            exit_code: Some(2),
        };
        let hp = compute_boss_hp(4, 4, &latest);
        assert_eq!(hp.test.state, CheckState::Failed);
        assert!(!hp.defeated);
    }

    #[test]
    fn missing_exit_code_does_not_count() {
        let mut latest = all(PASS);
        latest.build = CheckInput::Ran {
            passed: true,
            exit_code: None,
        };
        let hp = compute_boss_hp(4, 4, &latest);
        assert_eq!(hp.build.state, CheckState::Failed);
    }

    #[test]
    fn regression_restores_hp() {
        let before = compute_boss_hp(4, 4, &all(PASS));
        let mut latest = all(PASS);
        latest.test = FAIL;
        let after = compute_boss_hp(4, 4, &latest);
        assert_eq!(before.remaining_bp, 0);
        assert_eq!(after.remaining_bp, 1_500);
    }

    #[test]
    fn milestone_split_is_exact() {
        for n in [1u32, 3, 7] {
            let hp = compute_boss_hp(n, n, &LatestVerification::not_run());
            assert_eq!(hp.milestone_bp, 7_000, "n = {n}");
            let sum: u32 = hp.milestones.iter().map(|m| m.weight_bp).sum();
            assert_eq!(sum, 7_000, "n = {n}");
        }
    }

    #[test]
    fn zero_total_uses_one_implicit_milestone() {
        let hp = compute_boss_hp(0, 0, &LatestVerification::not_run());
        assert_eq!(hp.milestones_total, 1);
        assert_eq!(hp.milestones.len(), 1);
        assert_eq!(hp.remaining_bp, 10_000);
    }

    #[test]
    fn done_is_clamped_to_total() {
        let hp = compute_boss_hp(9, 3, &LatestVerification::not_run());
        assert_eq!(hp.milestones_done, 3);
        assert_eq!(hp.remaining_bp, 3_000);
    }

    #[test]
    fn unset_check_never_passes_and_blocks_defeat() {
        let mut latest = all(PASS);
        latest.lint = CheckInput::NotSet;
        let hp = compute_boss_hp(2, 2, &latest);
        assert_eq!(hp.remaining_bp, 500);
        assert!(!hp.defeated);
        assert!(hp.label.contains("lint not set"));
    }

    #[test]
    fn next_step_tells_what_to_do() {
        let fresh = compute_boss_hp(0, 1, &LatestVerification::not_run());
        assert!(fresh.next_step.starts_with("Start the task"));

        let mut latest = LatestVerification::not_run();
        latest.build = PASS;
        latest.test = FAIL;
        let hp = compute_boss_hp(0, 1, &latest);
        assert!(hp.next_step.starts_with("Failed: test"), "{}", hp.next_step);

        let latest = LatestVerification {
            build: PASS,
            test: PASS,
            lint: CheckInput::NotSet,
        };
        let hp = compute_boss_hp(1, 1, &latest);
        assert!(hp.next_step.contains("No command set for: lint"));

        assert!(
            compute_boss_hp(1, 1, &all(PASS))
                .next_step
                .contains("defeated")
        );
    }

    #[test]
    fn label_describes_partial_progress() {
        let latest = LatestVerification {
            build: PASS,
            test: CheckInput::NotRun,
            lint: CheckInput::NotSet,
        };
        let hp = compute_boss_hp(2, 4, &latest);
        assert_eq!(
            hp.label,
            "Milestone 2 of 4 done, build passed, test pending, lint not set"
        );
    }
}
