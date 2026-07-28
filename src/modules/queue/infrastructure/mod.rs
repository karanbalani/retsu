mod cached;
mod postgres;

pub(in crate::modules::queue) use cached::CachedQueueRepository;
pub(in crate::modules::queue) use postgres::PostgresQueueRepository;
