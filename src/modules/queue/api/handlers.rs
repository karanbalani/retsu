use actix_web::{HttpResponse, web};

use crate::{api::ApiError, app::ApplicationContext};

use super::{
    super::{
        application::{
            AcknowledgeMessageError, CreateQueueError, DequeueMessageError, EnqueueMessageError,
            UpdateQueueError,
        },
        domain::{MessageValidationError, QueueSettingsError},
    },
    dto::{
        AcknowledgeMessageRequest, CreateQueueRequest, DequeueMessageResponse,
        EnqueueMessageRequest, EnqueueMessageResponse, MessagePath, QueuePath, QueueResponse,
        UpdateQueueRequest,
    },
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

    Ok(HttpResponse::Created().json(QueueResponse::from(created_queue)))
}

fn map_create_queue_error(error: CreateQueueError) -> ApiError {
    match error {
        CreateQueueError::InvalidName(error) => {
            ApiError::bad_request("invalid_queue_name", error.to_string())
        }

        CreateQueueError::InvalidSettings(error) => map_queue_settings_error(error),

        CreateQueueError::AlreadyExists => ApiError::conflict(
            "queue_already_exists",
            "a queue with this name already exists",
        ),

        CreateQueueError::Persistence(error) => ApiError::internal(error),
    }
}

pub(super) async fn update_queue(
    context: web::Data<ApplicationContext>,
    path: web::Path<QueuePath>,
    request: web::Json<UpdateQueueRequest>,
) -> Result<HttpResponse, ApiError> {
    let command = request
        .into_inner()
        .into_command(path.into_inner().into_queue_id());
    let queue = context
        .queue_module()
        .update_queue(command)
        .await
        .map_err(map_update_queue_error)?;

    Ok(HttpResponse::Ok().json(QueueResponse::from(queue)))
}

fn map_update_queue_error(error: UpdateQueueError) -> ApiError {
    match error {
        UpdateQueueError::NoConfigurationChanges => ApiError::bad_request(
            "empty_queue_update",
            "at least one queue configuration field must be provided",
        ),

        UpdateQueueError::InvalidSettings(error) => map_queue_settings_error(error),

        UpdateQueueError::QueueNotFound => {
            ApiError::resource_not_found("queue_not_found", "the requested queue does not exist")
        }

        UpdateQueueError::Persistence(error) => ApiError::internal(error),
    }
}

fn map_queue_settings_error(error: QueueSettingsError) -> ApiError {
    match error {
        error @ QueueSettingsError::InvalidVisibilityTimeout => {
            ApiError::bad_request("invalid_visibility_timeout", error.to_string())
        }
        error @ QueueSettingsError::InvalidMaxDeliveryAttempts => {
            ApiError::bad_request("invalid_max_delivery_attempts", error.to_string())
        }
        error @ QueueSettingsError::InvalidDefaultMessageTtl => {
            ApiError::bad_request("invalid_default_message_ttl", error.to_string())
        }
    }
}

pub(super) async fn enqueue_message(
    context: web::Data<ApplicationContext>,
    path: web::Path<QueuePath>,
    request: web::Json<EnqueueMessageRequest>,
) -> Result<HttpResponse, ApiError> {
    let queue_id = path.into_inner().into_queue_id();
    let command = request.into_inner().into_command(queue_id);

    let enqueued_message = context
        .queue_module()
        .enqueue_message(command)
        .await
        .map_err(map_enqueue_message_error)?;

    Ok(HttpResponse::Created().json(EnqueueMessageResponse::from(enqueued_message)))
}

fn map_enqueue_message_error(error: EnqueueMessageError) -> ApiError {
    match error {
        EnqueueMessageError::InvalidMessage(error @ MessageValidationError::InvalidPriority) => {
            ApiError::bad_request("invalid_priority", error.to_string())
        }

        EnqueueMessageError::InvalidMessage(error @ MessageValidationError::InvalidTtl) => {
            ApiError::bad_request("invalid_ttl", error.to_string())
        }

        EnqueueMessageError::QueueNotFound => {
            ApiError::resource_not_found("queue_not_found", "the requested queue does not exist")
        }

        EnqueueMessageError::Persistence(error) => ApiError::internal(error),
    }
}

pub(super) async fn dequeue_message(
    context: web::Data<ApplicationContext>,
    path: web::Path<QueuePath>,
) -> Result<HttpResponse, ApiError> {
    let command = path.into_inner().into_dequeue_command();

    match context
        .queue_module()
        .dequeue_message(command)
        .await
        .map_err(map_dequeue_message_error)?
    {
        Some(message) => Ok(HttpResponse::Ok().json(DequeueMessageResponse::from(message))),
        None => Ok(HttpResponse::NoContent().finish()),
    }
}

fn map_dequeue_message_error(error: DequeueMessageError) -> ApiError {
    match error {
        DequeueMessageError::QueueNotFound => {
            ApiError::resource_not_found("queue_not_found", "the requested queue does not exist")
        }

        DequeueMessageError::Persistence(error) => ApiError::internal(error),
    }
}

pub(super) async fn acknowledge_message(
    context: web::Data<ApplicationContext>,
    path: web::Path<MessagePath>,
    request: web::Json<AcknowledgeMessageRequest>,
) -> Result<HttpResponse, ApiError> {
    let command = request.into_inner().into_command(path.into_inner());

    context
        .queue_module()
        .acknowledge_message(command)
        .await
        .map_err(map_acknowledge_message_error)?;

    Ok(HttpResponse::NoContent().finish())
}

fn map_acknowledge_message_error(error: AcknowledgeMessageError) -> ApiError {
    match error {
        AcknowledgeMessageError::QueueNotFound => {
            ApiError::resource_not_found("queue_not_found", "the requested queue does not exist")
        }

        AcknowledgeMessageError::Persistence(error) => ApiError::internal(error),
    }
}

#[cfg(test)]
#[path = "../tests/api_handlers.rs"]
mod tests;
