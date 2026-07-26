use thiserror::Error;
use uuid::Uuid;

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

        if ttl_seconds == Some(0) {
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

    #[error("ttl_seconds must be greater than zero when provided")]
    InvalidTtl,
}

#[cfg(test)]
mod tests {
    use uuid::Version;

    use super::{Message, MessagePriority};

    #[test]
    fn creates_stable_ids_and_preserves_the_priority_persistence_contract() {
        let cases = [("HIGH", 3), ("MEDIUM", 2), ("LOW", 1)];

        for (label, rank) in cases {
            let message = Message::new("payload".to_owned(), label.to_owned(), None)
                .expect("supported priority should be valid");

            assert_eq!(message.id().get_version(), Some(Version::SortRand));
            assert_eq!(message.priority().as_str(), label);
            assert_eq!(message.priority().rank(), rank);
            assert_eq!(
                MessagePriority::from_rank(rank)
                    .expect("persisted priority rank should be readable")
                    .as_str(),
                label
            );
        }
    }
}
