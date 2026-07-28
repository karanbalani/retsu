use actix_web::{ResponseError as _, body::to_bytes, http::StatusCode};
use serde_json::Value;

use super::{
    super::super::domain::{MessageValidationError, QueueNameError},
    AcknowledgeMessageError, CreateQueueError, DequeueMessageError, EnqueueMessageError,
    QueueSettingsError, UpdateQueueError, map_acknowledge_message_error, map_create_queue_error,
    map_dequeue_message_error, map_enqueue_message_error, map_update_queue_error,
};

#[actix_web::test]
async fn maps_queue_failures_to_stable_http_error_codes() {
    let cases = [
        (
            map_create_queue_error(CreateQueueError::InvalidName(QueueNameError::InvalidFormat)),
            StatusCode::BAD_REQUEST,
            "invalid_queue_name",
        ),
        (
            map_create_queue_error(CreateQueueError::InvalidSettings(
                QueueSettingsError::InvalidVisibilityTimeout,
            )),
            StatusCode::BAD_REQUEST,
            "invalid_visibility_timeout",
        ),
        (
            map_create_queue_error(CreateQueueError::InvalidSettings(
                QueueSettingsError::InvalidMaxDeliveryAttempts,
            )),
            StatusCode::BAD_REQUEST,
            "invalid_max_delivery_attempts",
        ),
        (
            map_create_queue_error(CreateQueueError::AlreadyExists),
            StatusCode::CONFLICT,
            "queue_already_exists",
        ),
        (
            map_update_queue_error(UpdateQueueError::NoConfigurationChanges),
            StatusCode::BAD_REQUEST,
            "empty_queue_update",
        ),
        (
            map_update_queue_error(UpdateQueueError::InvalidSettings(
                QueueSettingsError::InvalidDefaultMessageTtl,
            )),
            StatusCode::BAD_REQUEST,
            "invalid_default_message_ttl",
        ),
        (
            map_update_queue_error(UpdateQueueError::QueueNotFound),
            StatusCode::NOT_FOUND,
            "queue_not_found",
        ),
        (
            map_enqueue_message_error(EnqueueMessageError::InvalidMessage(
                MessageValidationError::InvalidPriority,
            )),
            StatusCode::BAD_REQUEST,
            "invalid_priority",
        ),
        (
            map_enqueue_message_error(EnqueueMessageError::InvalidMessage(
                MessageValidationError::InvalidTtl,
            )),
            StatusCode::BAD_REQUEST,
            "invalid_ttl",
        ),
        (
            map_enqueue_message_error(EnqueueMessageError::QueueNotFound),
            StatusCode::NOT_FOUND,
            "queue_not_found",
        ),
        (
            map_dequeue_message_error(DequeueMessageError::QueueNotFound),
            StatusCode::NOT_FOUND,
            "queue_not_found",
        ),
        (
            map_acknowledge_message_error(AcknowledgeMessageError::QueueNotFound),
            StatusCode::NOT_FOUND,
            "queue_not_found",
        ),
    ];

    for (error, expected_status, expected_code) in cases {
        let response = error.error_response();
        let status = response.status();
        let body = to_bytes(response.into_body())
            .await
            .expect("problem response body should be readable");
        let body: Value =
            serde_json::from_slice(&body).expect("problem response body should be JSON");

        assert_eq!(status, expected_status);
        assert_eq!(body["code"], expected_code);
    }
}
