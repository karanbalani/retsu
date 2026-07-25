use thiserror::Error;

use uuid::Uuid;

const MIN_QUEUE_NAME_LENGTH: usize = 1;
const MAX_QUEUE_NAME_LENGTH: usize = 64;

const DEFAULT_VISIBILITY_TIMEOUT_SECONDS: u32 = 30;
const MIN_VISIBILITY_TIMEOUT_SECONDS: u32 = 1;
const MAX_VISIBILITY_TIMEOUT_SECONDS: u32 = 21600;

const DEFAULT_MAX_DELIVERY_ATTEMPTS: u16 = 5;
const MIN_MAX_DELIVERY_ATTEMPTS: u16 = 1;
const MAX_MAX_DELIVERY_ATTEMPTS: u16 = 100;

#[derive(Clone, Debug)]
pub(in crate::modules::queue) struct Queue {
    id: Uuid,
    name: QueueName,
    settings: QueueSettings,
}

impl Queue {
    pub(in crate::modules::queue) fn new(
        name: String,
        visibility_timeout_seconds: Option<u32>,
        max_delivery_attempts: Option<u16>,
    ) -> Result<Self, QueueValidationError> {
        Ok(Self {
            id: Uuid::now_v7(),
            name: QueueName::parse(name)?,
            settings: QueueSettings::new(visibility_timeout_seconds, max_delivery_attempts)?,
        })
    }

    pub(in crate::modules::queue) fn id(&self) -> Uuid {
        self.id
    }

    pub(in crate::modules::queue) fn name(&self) -> &str {
        self.name.as_str()
    }

    pub(in crate::modules::queue) fn visibility_timeout_seconds(&self) -> u32 {
        self.settings.visibility_timeout_seconds()
    }

    pub(in crate::modules::queue) fn max_delivery_attempts(&self) -> u16 {
        self.settings.max_delivery_attempts()
    }
}

#[derive(Debug, Clone)]
struct QueueName(String);

impl QueueName {
    fn parse(value: String) -> Result<Self, QueueNameError> {
        let bytes = value.as_bytes();

        if !(MIN_QUEUE_NAME_LENGTH..=MAX_QUEUE_NAME_LENGTH).contains(&bytes.len()) {
            return Err(QueueNameError::InvalidLength);
        }

        let has_valid_start = bytes.first().copied().is_some_and(is_queue_name_endpoint);

        let has_valid_end = bytes.last().copied().is_some_and(is_queue_name_endpoint);

        let contains_only_supported_characters = bytes.iter().copied().all(is_queue_name_character);

        if !has_valid_start || !has_valid_end || !contains_only_supported_characters {
            return Err(QueueNameError::InvalidFormat);
        }

        Ok(Self(value))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_queue_name_endpoint(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit()
}

fn is_queue_name_character(byte: u8) -> bool {
    is_queue_name_endpoint(byte) || matches!(byte, b'.' | b'_' | b'-')
}

#[derive(Debug, Clone, Copy)]
struct QueueSettings {
    visibility_timeout_seconds: u32,
    max_delivery_attempts: u16,
}

impl QueueSettings {
    fn new(
        visibility_timeout_seconds: Option<u32>,
        max_delivery_attempts: Option<u16>,
    ) -> Result<Self, QueueSettingsError> {
        let visibility_timeout_seconds =
            visibility_timeout_seconds.unwrap_or(DEFAULT_VISIBILITY_TIMEOUT_SECONDS);

        if !(MIN_VISIBILITY_TIMEOUT_SECONDS..=MAX_VISIBILITY_TIMEOUT_SECONDS)
            .contains(&visibility_timeout_seconds)
        {
            return Err(QueueSettingsError::InvalidVisibilityTimeout);
        }

        let max_delivery_attempts = max_delivery_attempts.unwrap_or(DEFAULT_MAX_DELIVERY_ATTEMPTS);

        if !(MIN_MAX_DELIVERY_ATTEMPTS..=MAX_MAX_DELIVERY_ATTEMPTS).contains(&max_delivery_attempts)
        {
            return Err(QueueSettingsError::InvalidMaxDeliveryAttempts);
        }

        Ok(Self {
            visibility_timeout_seconds,
            max_delivery_attempts,
        })
    }

    fn visibility_timeout_seconds(&self) -> u32 {
        self.visibility_timeout_seconds
    }

    fn max_delivery_attempts(&self) -> u16 {
        self.max_delivery_attempts
    }
}

#[derive(Error, Debug)]
pub(in crate::modules::queue) enum QueueValidationError {
    #[error(transparent)]
    InvalidName(#[from] QueueNameError),

    #[error(transparent)]
    InvalidSettings(#[from] QueueSettingsError),
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum QueueNameError {
    #[error("queue name must contain between 1 and 64 ASCII characters")]
    InvalidLength,

    #[error(
        "queue name must contain only ASCII lowercase letters, digits, dots, underscores, or hyphens, and must start and end with an ASCII lowercase letter or digit"
    )]
    InvalidFormat,
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum QueueSettingsError {
    #[error("visibility timeout must be between 1 and 21600 seconds")]
    InvalidVisibilityTimeout,

    #[error("max delivery attempts must be between 1 and 100")]
    InvalidMaxDeliveryAttempts,
}
