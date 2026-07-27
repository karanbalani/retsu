use thiserror::Error;
use uuid::Uuid;

const MIN_MESSAGE_TTL_SECONDS: u32 = 1;
const MAX_MESSAGE_TTL_SECONDS: u32 = 2_592_000;

#[derive(Clone, Debug)]
pub(in crate::modules::queue) struct Message {
    id: Uuid,
    payload: String,
    priority: MessagePriority,
    ttl_seconds: Option<u32>,
}

impl Message {
    pub(in crate::modules::queue) fn new(
        payload: String,
        priority: String,
        ttl_seconds: Option<u32>,
    ) -> Result<Self, MessageValidationError> {
        let priority = MessagePriority::parse(&priority)?;

        if ttl_seconds
            .is_some_and(|ttl| !(MIN_MESSAGE_TTL_SECONDS..=MAX_MESSAGE_TTL_SECONDS).contains(&ttl))
        {
            return Err(MessageValidationError::InvalidTtl);
        }

        Ok(Self {
            id: Uuid::now_v7(),
            payload,
            priority,
            ttl_seconds,
        })
    }

    pub(in crate::modules::queue) fn id(&self) -> Uuid {
        self.id
    }

    pub(in crate::modules::queue) fn payload(&self) -> &str {
        &self.payload
    }

    pub(in crate::modules::queue) fn priority(&self) -> MessagePriority {
        self.priority
    }

    pub(in crate::modules::queue) fn ttl_seconds(&self) -> Option<u32> {
        self.ttl_seconds
    }
}

#[derive(Clone, Debug, Copy)]
pub(in crate::modules::queue) enum MessagePriority {
    High,
    Medium,
    Low,
}

impl MessagePriority {
    fn parse(value: &str) -> Result<Self, MessageValidationError> {
        match value {
            "HIGH" => Ok(Self::High),
            "MEDIUM" => Ok(Self::Medium),
            "LOW" => Ok(Self::Low),
            _ => Err(MessageValidationError::InvalidPriority),
        }
    }

    pub(in crate::modules::queue) fn from_rank(rank: i16) -> Option<Self> {
        match rank {
            3 => Some(Self::High),
            2 => Some(Self::Medium),
            1 => Some(Self::Low),
            _ => None,
        }
    }

    pub(in crate::modules::queue) fn as_str(self) -> &'static str {
        match self {
            Self::High => "HIGH",
            Self::Medium => "MEDIUM",
            Self::Low => "LOW",
        }
    }

    pub(in crate::modules::queue) fn rank(self) -> i16 {
        match self {
            MessagePriority::High => 3,
            MessagePriority::Medium => 2,
            MessagePriority::Low => 1,
        }
    }
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum MessageValidationError {
    #[error("priority must be one of HIGH, MEDIUM or LOW")]
    InvalidPriority,

    #[error("ttl_seconds must be between 1 and 2592000 when provided")]
    InvalidTtl,
}

#[cfg(test)]
#[path = "../tests/domain_message.rs"]
mod tests;
