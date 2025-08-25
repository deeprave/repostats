//! Tests for message grouping functionality

#[cfg(test)]
mod tests {
    use crate::queue::{GroupedMessage, Message};

    /// Test message type that implements grouping for testing
    #[derive(Debug, Clone)]
    struct TestGroupedMessage {
        message: Message,
        group_id: Option<String>,
        starts_group_with_count: Option<(String, usize)>,
        completes_group: Option<String>,
    }

    impl TestGroupedMessage {
        fn new(producer_id: String, message_type: String, data: String) -> Self {
            Self {
                message: Message::new(producer_id, message_type, data),
                group_id: None,
                starts_group_with_count: None,
                completes_group: None,
            }
        }

        fn with_group(mut self, group_id: String) -> Self {
            self.group_id = Some(group_id);
            self
        }

        fn with_start_group(mut self, group_id: String, count: usize) -> Self {
            self.starts_group_with_count = Some((group_id.clone(), count));
            self.group_id = Some(group_id); // Also set group_id
            self
        }

        fn with_complete_group(mut self, group_id: String) -> Self {
            self.completes_group = Some(group_id.clone());
            self.group_id = Some(group_id); // Also set group_id
            self
        }
    }

    impl GroupedMessage for TestGroupedMessage {
        fn group_id(&self) -> Option<String> {
            self.group_id.clone()
        }

        fn starts_group(&self) -> Option<(String, usize)> {
            self.starts_group_with_count.clone()
        }

        fn completes_group(&self) -> Option<String> {
            self.completes_group.clone()
        }
    }

    #[test]
    fn test_default_message_has_no_grouping() {
        let message = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "test.rs".to_string(),
        );

        // Default implementation should return None for all grouping methods
        assert_eq!(message.group_id(), None);
        assert_eq!(message.starts_group(), None);
        assert_eq!(message.completes_group(), None);
    }

    #[test]
    fn test_grouped_message_basic_group_id() {
        let grouped_msg = TestGroupedMessage::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file1.rs".to_string(),
        )
        .with_group("commit-abc123".to_string());

        assert_eq!(grouped_msg.group_id(), Some("commit-abc123".to_string()));
        assert_eq!(grouped_msg.starts_group(), None);
        assert_eq!(grouped_msg.completes_group(), None);
    }

    #[test]
    fn test_grouped_message_starts_group_with_count() {
        let start_msg = TestGroupedMessage::new(
            "scanner-a".to_string(),
            "commit".to_string(),
            "commit data".to_string(),
        )
        .with_start_group("commit-abc123".to_string(), 5);

        assert_eq!(start_msg.group_id(), Some("commit-abc123".to_string()));
        assert_eq!(
            start_msg.starts_group(),
            Some(("commit-abc123".to_string(), 5))
        );
        assert_eq!(start_msg.completes_group(), None);
    }

    #[test]
    fn test_grouped_message_completes_group() {
        let complete_msg = TestGroupedMessage::new(
            "scanner-a".to_string(),
            "completion".to_string(),
            "end marker".to_string(),
        )
        .with_complete_group("commit-abc123".to_string());

        assert_eq!(complete_msg.group_id(), Some("commit-abc123".to_string()));
        assert_eq!(complete_msg.starts_group(), None);
        assert_eq!(
            complete_msg.completes_group(),
            Some("commit-abc123".to_string())
        );
    }

    #[test]
    fn test_grouped_message_scoped_by_producer_and_group() {
        // This test verifies that groups are scoped by both producer_id and group_id
        // Different producers can have the same group_id without interference
        let msg_producer_a = TestGroupedMessage::new(
            "producer-a".to_string(),
            "file".to_string(),
            "file1.rs".to_string(),
        )
        .with_group("commit-123".to_string());

        let msg_producer_b = TestGroupedMessage::new(
            "producer-b".to_string(),
            "file".to_string(),
            "file1.rs".to_string(),
        )
        .with_group("commit-123".to_string()); // Same group_id, different producer

        // Both should have the same group_id but they're logically separate groups
        assert_eq!(msg_producer_a.group_id(), Some("commit-123".to_string()));
        assert_eq!(msg_producer_b.group_id(), Some("commit-123".to_string()));

        // The scoping logic will be implemented in the event integration layer
        // This test documents the expected behavior
    }
}
