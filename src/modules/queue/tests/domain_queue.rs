use uuid::Version;

use super::{
    DEFAULT_MAX_DELIVERY_ATTEMPTS, DEFAULT_VISIBILITY_TIMEOUT_SECONDS, MAX_MAX_DELIVERY_ATTEMPTS,
    MAX_MESSAGE_TTL_SECONDS, MAX_VISIBILITY_TIMEOUT_SECONDS, Queue, QueueValidationError,
};

#[test]
fn creates_queues_with_uuid_v7_defaults_and_supported_boundaries() {
    let queue = Queue::new("email-delivery".to_owned(), None, None, None)
        .expect("valid queue should be created");

    assert_eq!(queue.id().get_version(), Some(Version::SortRand));
    assert_eq!(
        queue.visibility_timeout_seconds(),
        DEFAULT_VISIBILITY_TIMEOUT_SECONDS
    );
    assert_eq!(queue.max_delivery_attempts(), DEFAULT_MAX_DELIVERY_ATTEMPTS);

    let maximum_length_name = format!("a{}z", "b".repeat(62));
    let boundary_queue = Queue::new(
        maximum_length_name.clone(),
        Some(MAX_VISIBILITY_TIMEOUT_SECONDS),
        Some(MAX_MAX_DELIVERY_ATTEMPTS),
        Some(MAX_MESSAGE_TTL_SECONDS),
    )
    .expect("boundary values should be accepted");

    assert_eq!(boundary_queue.name(), maximum_length_name);
    assert_eq!(
        boundary_queue.visibility_timeout_seconds(),
        MAX_VISIBILITY_TIMEOUT_SECONDS
    );
    assert_eq!(
        boundary_queue.max_delivery_attempts(),
        MAX_MAX_DELIVERY_ATTEMPTS
    );
}

#[test]
fn rejects_noncanonical_queue_names() {
    let invalid_names = [
        String::new(),
        "Uppercase".to_owned(),
        "-leading".to_owned(),
        "trailing-".to_owned(),
        "contains space".to_owned(),
        "non-ascii-é".to_owned(),
        "a".repeat(65),
    ];

    for name in invalid_names {
        assert!(
            matches!(
                Queue::new(name, None, None, None),
                Err(QueueValidationError::InvalidName(_))
            ),
            "invalid queue name should be rejected"
        );
    }
}

#[test]
fn rejects_queue_settings_outside_supported_ranges() {
    let invalid_settings = [
        (Some(0), None),
        (Some(MAX_VISIBILITY_TIMEOUT_SECONDS + 1), None),
        (None, Some(0)),
        (None, Some(MAX_MAX_DELIVERY_ATTEMPTS + 1)),
    ];

    for (visibility_timeout_seconds, max_delivery_attempts) in invalid_settings {
        assert!(
            matches!(
                Queue::new(
                    "email-delivery".to_owned(),
                    visibility_timeout_seconds,
                    max_delivery_attempts,
                    None,
                ),
                Err(QueueValidationError::InvalidSettings(_))
            ),
            "invalid queue settings should be rejected"
        );
    }
}
