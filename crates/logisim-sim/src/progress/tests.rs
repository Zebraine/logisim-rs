use super::*;
#[test]
fn settled_batches_and_retries_renew_the_limit_without_losing_history() {
    let mut progress = Propagation::default();
    for _ in 0..2 {
        for _ in 1..PROPAGATION_LIMIT {
            assert!(!progress.record_batch(true));
        }
        assert!(!progress.record_batch(false));
    }
    for _ in 1..PROPAGATION_LIMIT {
        assert!(!progress.record_batch(true));
    }
    assert!(progress.record_batch(true));
    assert_eq!(progress.steps(), 3 * PROPAGATION_LIMIT as u64);
    progress.restart();
    assert!(!progress.record_batch(true));
    assert_eq!(progress.steps(), 3 * PROPAGATION_LIMIT as u64 + 1);
    progress.reset();
    assert_eq!(progress.steps(), 0);
    assert!(!progress.record_batch(true));
}
