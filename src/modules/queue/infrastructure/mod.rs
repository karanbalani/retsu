mod cached;
mod postgres;

pub(in crate::modules::queue) use cached::QueueNameCachingRepository;
pub(in crate::modules::queue) use postgres::PostgresQueueRepository;
