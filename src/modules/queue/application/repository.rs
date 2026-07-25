use super::super::domain::Queue;

pub(in crate::modules::queue) enum CreateQueueOutcome {
    Created,
    AlreadyExists,
}

pub(in crate::modules::queue) trait QueueRepository {
    async fn create_queue(&self, queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error>;
}
