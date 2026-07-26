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
