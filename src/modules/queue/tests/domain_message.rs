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
