use actix_web::{HttpResponse, web};

use crate::{api::ApiError, app::ApplicationContext};

use super::{
    super::{application::CreateQueueError, domain::QueueSettingsError},
    dto::{CreateQueueRequest, CreateQueueResponse},
};

pub(super) async fn create_queue(
    context: web::Data<ApplicationContext>,
    request: web::Json<CreateQueueRequest>,
) -> Result<HttpResponse, ApiError> {
    let created_queue = context
        .queue_module()
        .create_queue(request.into_inner().into_command())
        .await
        .map_err(map_create_queue_error)?;

    Ok(HttpResponse::Created().json(CreateQueueResponse::from(created_queue)))
}

fn map_create_queue_error(error: CreateQueueError) -> ApiError {
    match error {
        CreateQueueError::InvalidName(error) => {
            ApiError::bad_request("invalid_queue_name", error.to_string())
        }
        CreateQueueError::InvalidSettings(error @ QueueSettingsError::InvalidVisibilityTimeout) => {
            ApiError::bad_request("invalid_visibility_timeout", error.to_string())
        }
        CreateQueueError::InvalidSettings(
            error @ QueueSettingsError::InvalidMaxDeliveryAttempts,
        ) => ApiError::bad_request("invalid_max_delivery_attempts", error.to_string()),
        CreateQueueError::AlreadyExists => ApiError::conflict(
            "queue_already_exists",
            "a queue with this name already exists",
        ),
        CreateQueueError::Persistence(error) => ApiError::internal(error),
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{ResponseError as _, body::to_bytes, http::StatusCode};
    use serde_json::Value;

    use super::{
        super::super::domain::QueueNameError, CreateQueueError, QueueSettingsError,
        map_create_queue_error,
    };

    #[actix_web::test]
    async fn maps_queue_failures_to_stable_http_error_codes() {
        let cases = [
            (
                CreateQueueError::InvalidName(QueueNameError::InvalidFormat),
                StatusCode::BAD_REQUEST,
                "invalid_queue_name",
            ),
            (
                CreateQueueError::InvalidSettings(QueueSettingsError::InvalidVisibilityTimeout),
                StatusCode::BAD_REQUEST,
                "invalid_visibility_timeout",
            ),
            (
                CreateQueueError::InvalidSettings(QueueSettingsError::InvalidMaxDeliveryAttempts),
                StatusCode::BAD_REQUEST,
                "invalid_max_delivery_attempts",
            ),
            (
                CreateQueueError::AlreadyExists,
                StatusCode::CONFLICT,
                "queue_already_exists",
            ),
        ];

        for (error, expected_status, expected_code) in cases {
            let response = map_create_queue_error(error).error_response();
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
}
