//! AsyncNotificationManager implementation

use crate::notifications::event::{Event, EventFilter};
use crate::notifications::traits::SubscriberStatistics;
use crate::notifications::error::NotificationError;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

// Auto-management thresholds
const HIGH_WATER_MARK: usize = 10000;  // Queue size threshold for concern
const STALE_SUBSCRIBER_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes without consuming
const MIN_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(60); // Minimum time between error logs
const ERROR_RATE_THRESHOLD: f64 = 0.1; // 10% error rate triggers log rate limiting

struct SubscriberInfo {
    filter: EventFilter,
    source: String,
    sender: UnboundedSender<Event>,
    statistics: SubscriberStatistics,
}

pub struct HealthAssessment {
    pub high_water_mark_subscribers: Vec<String>,
    pub stale_subscribers: Vec<String>,
    pub error_prone_subscribers: Vec<String>,
}

pub struct AsyncNotificationManager {
    subscribers: HashMap<String, SubscriberInfo>,
}

impl AsyncNotificationManager {
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
        }
    }

    pub fn subscribe(
        &mut self,
        subscriber_id: String,
        filter: EventFilter,
        source: String,
    ) -> Result<UnboundedReceiver<Event>, Box<dyn std::error::Error>> {
        let (sender, receiver) = unbounded_channel();

        let subscriber_info = SubscriberInfo {
            filter,
            source: source.clone(),
            sender,
            statistics: SubscriberStatistics::new(),
        };

        // Warn if overwriting existing subscriber
        if let Some(existing) = self.subscribers.insert(subscriber_id.clone(), subscriber_info) {
            log::warn!(
                "Subscriber '{}' replaced existing subscription (source: {} -> {})",
                subscriber_id,
                existing.source,
                source
            );
        }

        Ok(receiver)
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }

    pub fn has_subscriber(&self, subscriber_id: &str) -> bool {
        self.subscribers.contains_key(subscriber_id)
    }

    pub fn get_subscriber_statistics(&self, subscriber_id: &str) -> Option<&SubscriberStatistics> {
        self.subscribers.get(subscriber_id).map(|info| &info.statistics)
    }

    pub fn check_high_water_marks(&self) -> Vec<String> {
        self.subscribers
            .iter()
            .filter_map(|(id, info)| {
                if info.statistics.queue_size() >= HIGH_WATER_MARK {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn check_stale_subscribers(&self) -> Vec<String> {
        let now = Instant::now();
        self.subscribers
            .iter()
            .filter_map(|(id, info)| {
                let has_high_queue = info.statistics.queue_size() >= HIGH_WATER_MARK;
                let is_stale = if let Some(last_msg_time) = info.statistics.last_message_time() {
                    now.duration_since(last_msg_time) > STALE_SUBSCRIBER_TIMEOUT
                } else {
                    // No messages processed yet, but has high queue, consider stale
                    has_high_queue
                };

                if has_high_queue && is_stale {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn auto_unsubscribe_stale(&mut self) -> Vec<String> {
        let stale_subscribers = self.check_stale_subscribers();

        for subscriber_id in &stale_subscribers {
            self.subscribers.remove(subscriber_id);
        }

        stale_subscribers
    }

    pub fn log_subscriber_health(&self, subscriber_id: &str) {
        if let Some(info) = self.subscribers.get(subscriber_id) {
            let stats = &info.statistics;
            let queue_size = stats.queue_size();
            let error_count = stats.error_count();
            let messages_processed = stats.messages_processed();

            // Check if we should log based on error rate and time since last log
            let should_log = if let Some(last_error_log_time) = stats.last_error_log_time() {
                let time_since_last_log = Instant::now().duration_since(last_error_log_time);
                time_since_last_log >= MIN_ERROR_LOG_INTERVAL
            } else {
                true // First time logging
            };

            if should_log && messages_processed > 0 {
                let error_rate = error_count as f64 / messages_processed as f64;

                if error_rate >= ERROR_RATE_THRESHOLD || queue_size >= HIGH_WATER_MARK {
                    log::warn!(
                        "Subscriber health concern - ID: '{}', Source: '{}', Queue: {}, Errors: {}/{} ({:.1}% rate), Messages: {}",
                        subscriber_id,
                        info.source,
                        queue_size,
                        error_count,
                        messages_processed,
                        error_rate * 100.0,
                        messages_processed
                    );

                    // Record that we logged
                    stats.record_error_logged();
                }
            }
        }
    }


    pub fn assess_subscriber_health(&self) -> HealthAssessment {
        let high_water_mark_subscribers = self.check_high_water_marks();
        let stale_subscribers = self.check_stale_subscribers();

        let error_prone_subscribers = self.subscribers
            .iter()
            .filter_map(|(id, info)| {
                let stats = &info.statistics;
                let messages_processed = stats.messages_processed();
                if messages_processed > 0 {
                    let error_rate = stats.error_count() as f64 / messages_processed as f64;
                    if error_rate >= ERROR_RATE_THRESHOLD {
                        Some(id.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        HealthAssessment {
            high_water_mark_subscribers,
            stale_subscribers,
            error_prone_subscribers,
        }
    }

    pub async fn perform_health_maintenance(&mut self) -> HealthAssessment {
        // Assess current health
        let assessment = self.assess_subscriber_health();

        // Log health concerns for error-prone subscribers
        for subscriber_id in &assessment.error_prone_subscribers {
            self.log_subscriber_health(subscriber_id);
        }

        // Log health concerns for high water mark subscribers
        for subscriber_id in &assessment.high_water_mark_subscribers {
            self.log_subscriber_health(subscriber_id);
        }

        // Auto-unsubscribe stale subscribers
        let _removed_stale = self.auto_unsubscribe_stale();

        assessment
    }

    pub fn check_memory_exhaustion(&self) -> Result<(), NotificationError> {
        let queue_sizes: Vec<(String, usize)> = self.subscribers
            .iter()
            .map(|(id, info)| (id.clone(), info.statistics.queue_size()))
            .collect();

        let total_events: usize = queue_sizes.iter().map(|(_, size)| *size).sum();

        // Rough heuristic: if we have more than 1 million total events queued, consider OOM
        const MEMORY_EXHAUSTION_THRESHOLD: usize = 1_000_000;

        if total_events > MEMORY_EXHAUSTION_THRESHOLD {
            return Err(NotificationError::OutOfMemory {
                queue_sizes,
                total_events,
            });
        }

        Ok(())
    }

    pub fn check_system_overload(&self) -> Result<(), NotificationError> {
        let assessment = self.assess_subscriber_health();
        let active_subscribers = self.subscriber_count();

        // System is overloaded if too many subscribers are problematic
        const MAX_ACTIVE_SUBSCRIBERS: usize = 1000;
        const MAX_PROBLEMATIC_RATIO: f64 = 0.5; // 50% of subscribers problematic

        let problematic_count = assessment.high_water_mark_subscribers.len() + assessment.stale_subscribers.len();
        let problematic_ratio = if active_subscribers > 0 {
            problematic_count as f64 / active_subscribers as f64
        } else {
            0.0
        };

        if active_subscribers > MAX_ACTIVE_SUBSCRIBERS || problematic_ratio > MAX_PROBLEMATIC_RATIO {
            return Err(NotificationError::SystemOverload {
                active_subscribers,
                high_water_mark_count: assessment.high_water_mark_subscribers.len(),
                stale_count: assessment.stale_subscribers.len(),
            });
        }

        Ok(())
    }

    pub async fn publish(&mut self, event: Event) -> Result<(), NotificationError> {
        let mut failed_subscribers = Vec::new();
        let event_type = match &event {
            Event::Scan(_) => "Scan",
            Event::Queue(_) => "Queue",
            Event::Plugin(_) => "Plugin",
            Event::System(_) => "System",
        }.to_string();

        for (subscriber_id, subscriber_info) in &self.subscribers {
            // Check if the event matches the subscriber's filter
            if subscriber_info.filter.accepts(&event) {
                // Increment queue size before sending
                subscriber_info.statistics.increment_queue_size();

                // Try to send the event
                if let Err(_) = subscriber_info.sender.send(event.clone()) {
                    // Channel is closed, mark for removal
                    failed_subscribers.push(subscriber_id.clone());
                }
            }
        }

        // Remove subscribers with closed channels
        for subscriber_id in &failed_subscribers {
            self.subscribers.remove(subscriber_id);
        }

        // Return error if any subscribers failed
        if !failed_subscribers.is_empty() {
            return Err(NotificationError::PublishFailed {
                event_type,
                failed_subscribers,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_notification_manager_creation() {
        let _manager = AsyncNotificationManager::new();
        // Test passes when AsyncNotificationManager::new() is implemented
    }

    #[tokio::test]
    async fn test_subscribe_method() {
        let mut manager = AsyncNotificationManager::new();

        // Test subscribe method returns UnboundedReceiver
        let receiver = manager.subscribe(
            "test_subscriber".to_string(),
            EventFilter::All,
            "test:unit".to_string()
        );

        // Verify receiver is returned
        assert!(receiver.is_ok());
    }

    #[tokio::test]
    async fn test_subscriber_registration_with_source() {
        let mut manager = AsyncNotificationManager::new();

        // Subscribe with different filters and sources
        let _receiver1 = manager.subscribe(
            "scanner".to_string(),
            EventFilter::ScanOnly,
            "scanner:file_processor".to_string()
        ).expect("Should subscribe successfully");

        let _receiver2 = manager.subscribe(
            "exporter".to_string(),
            EventFilter::All,
            "plugin:export".to_string()
        ).expect("Should subscribe successfully");

        // Verify subscribers are stored (we need a way to check this)
        assert_eq!(manager.subscriber_count(), 2);
        assert!(manager.has_subscriber("scanner"));
        assert!(manager.has_subscriber("exporter"));
        assert!(!manager.has_subscriber("nonexistent"));
    }

    #[tokio::test]
    async fn test_publish_method() {
        use crate::notifications::event::{Event, ScanEvent, ScanEventType, SystemEvent, SystemEventType};

        let mut manager = AsyncNotificationManager::new();

        // Subscribe with different filters
        let mut scan_receiver = manager.subscribe(
            "scanner".to_string(),
            EventFilter::ScanOnly,
            "scanner:test".to_string()
        ).expect("Should subscribe successfully");

        let mut all_receiver = manager.subscribe(
            "logger".to_string(),
            EventFilter::All,
            "logger:test".to_string()
        ).expect("Should subscribe successfully");

        // Publish a scan event
        let scan_event = Event::Scan(ScanEvent::new(ScanEventType::Started, "test_scan".to_string()));
        let _ = manager.publish(scan_event.clone()).await; // May fail due to closed channels, that's ok

        // Publish a system event
        let system_event = Event::System(SystemEvent::new(SystemEventType::Startup));
        let _ = manager.publish(system_event.clone()).await; // May fail due to closed channels, that's ok

        // Scan subscriber should receive only scan event
        let received_scan = scan_receiver.recv().await.expect("Should receive scan event");
        assert!(matches!(received_scan, Event::Scan(_)));

        // All subscriber should receive both events
        let received_1 = all_receiver.recv().await.expect("Should receive first event");
        let received_2 = all_receiver.recv().await.expect("Should receive second event");

        // Should have received both events (order may vary)
        let mut received_scan_count = 0;
        let mut received_system_count = 0;

        for event in [received_1, received_2] {
            match event {
                Event::Scan(_) => received_scan_count += 1,
                Event::System(_) => received_system_count += 1,
                _ => {}
            }
        }

        assert_eq!(received_scan_count, 1);
        assert_eq!(received_system_count, 1);
    }

    #[tokio::test]
    async fn test_automatic_subscriber_cleanup() {
        use crate::notifications::event::{Event, SystemEvent, SystemEventType};

        let mut manager = AsyncNotificationManager::new();

        // Subscribe two subscribers
        let _receiver1 = manager.subscribe(
            "subscriber1".to_string(),
            EventFilter::All,
            "test:cleanup1".to_string()
        ).expect("Should subscribe successfully");

        let receiver2 = manager.subscribe(
            "subscriber2".to_string(),
            EventFilter::All,
            "test:cleanup2".to_string()
        ).expect("Should subscribe successfully");

        // Verify both subscribers are registered
        assert_eq!(manager.subscriber_count(), 2);
        assert!(manager.has_subscriber("subscriber1"));
        assert!(manager.has_subscriber("subscriber2"));

        // Drop one receiver (simulating subscriber going away)
        drop(_receiver1);

        // Publish an event
        let event = Event::System(SystemEvent::new(SystemEventType::Startup));
        let _ = manager.publish(event).await;

        // Verify the dropped subscriber was cleaned up
        assert_eq!(manager.subscriber_count(), 1);
        assert!(!manager.has_subscriber("subscriber1"));
        assert!(manager.has_subscriber("subscriber2"));

        // Verify remaining subscriber still works
        drop(receiver2);
        let event2 = Event::System(SystemEvent::new(SystemEventType::Shutdown));
        let _ = manager.publish(event2).await;

        // All subscribers should be cleaned up now
        assert_eq!(manager.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_queue_size_increment_on_publish() {
        use crate::notifications::event::{Event, ScanEvent, ScanEventType, SystemEvent, SystemEventType};

        let mut manager = AsyncNotificationManager::new();

        // Subscribe two subscribers with different filters
        let _scan_receiver = manager.subscribe(
            "scanner".to_string(),
            EventFilter::ScanOnly,
            "scanner:test".to_string()
        ).expect("Should subscribe successfully");

        let _all_receiver = manager.subscribe(
            "logger".to_string(),
            EventFilter::All,
            "logger:test".to_string()
        ).expect("Should subscribe successfully");

        // Check initial queue sizes
        assert_eq!(manager.get_subscriber_statistics("scanner").unwrap().queue_size(), 0);
        assert_eq!(manager.get_subscriber_statistics("logger").unwrap().queue_size(), 0);

        // Publish a scan event
        let scan_event = Event::Scan(ScanEvent::new(ScanEventType::Started, "test_scan".to_string()));
        let _ = manager.publish(scan_event).await;

        // Scanner should have queue size 1, logger should also have queue size 1
        assert_eq!(manager.get_subscriber_statistics("scanner").unwrap().queue_size(), 1);
        assert_eq!(manager.get_subscriber_statistics("logger").unwrap().queue_size(), 1);

        // Publish a system event
        let system_event = Event::System(SystemEvent::new(SystemEventType::Startup));
        let _ = manager.publish(system_event).await;

        // Scanner queue size should still be 1 (filtered out), logger should be 2
        assert_eq!(manager.get_subscriber_statistics("scanner").unwrap().queue_size(), 1);
        assert_eq!(manager.get_subscriber_statistics("logger").unwrap().queue_size(), 2);
    }

    #[tokio::test]
    async fn test_high_water_mark_detection() {
        let mut manager = AsyncNotificationManager::new();

        // Subscribe a subscriber that we'll overload
        let _receiver = manager.subscribe(
            "overloaded".to_string(),
            EventFilter::All,
            "test:overload".to_string()
        ).expect("Should subscribe successfully");

        // Subscribe a normal subscriber
        let mut normal_receiver = manager.subscribe(
            "normal".to_string(),
            EventFilter::All,
            "test:normal".to_string()
        ).expect("Should subscribe successfully");

        // Initially no high water marks
        assert!(manager.check_high_water_marks().is_empty());

        // Simulate HIGH_WATER_MARK events (10000) for overloaded subscriber
        use crate::notifications::event::{Event, SystemEvent, SystemEventType};

        // Since HIGH_WATER_MARK = 10000, we need to publish many events
        // Let's manually set queue size for testing purposes
        let overloaded_stats = manager.get_subscriber_statistics("overloaded").unwrap();

        // Simulate HIGH_WATER_MARK by incrementing manually
        for _ in 0..HIGH_WATER_MARK {
            overloaded_stats.increment_queue_size();
        }

        // Normal subscriber processes messages normally
        let event = Event::System(SystemEvent::new(SystemEventType::Startup));
        let _ = manager.publish(event).await;

        // Process the message for normal subscriber to keep its queue low
        let _ = normal_receiver.recv().await.expect("Should receive event");
        manager.get_subscriber_statistics("normal").unwrap().decrement_queue_size();

        // Check high water marks
        let high_water_subscribers = manager.check_high_water_marks();
        assert_eq!(high_water_subscribers.len(), 1);
        assert!(high_water_subscribers.contains(&"overloaded".to_string()));
        assert!(!high_water_subscribers.contains(&"normal".to_string()));
    }

    #[tokio::test]
    async fn test_stale_subscriber_detection() {
        let mut manager = AsyncNotificationManager::new();

        // Subscribe a stale subscriber (won't process messages)
        let _stale_receiver = manager.subscribe(
            "stale".to_string(),
            EventFilter::All,
            "test:stale".to_string()
        ).expect("Should subscribe successfully");

        // Subscribe an active subscriber
        let _active_receiver = manager.subscribe(
            "active".to_string(),
            EventFilter::All,
            "test:active".to_string()
        ).expect("Should subscribe successfully");

        // Initially no stale subscribers
        assert!(manager.check_stale_subscribers().is_empty());

        // Simulate high queue size for both subscribers

        let stale_stats = manager.get_subscriber_statistics("stale").unwrap();
        let active_stats = manager.get_subscriber_statistics("active").unwrap();

        // Set high queue sizes for both
        for _ in 0..HIGH_WATER_MARK {
            stale_stats.increment_queue_size();
            active_stats.increment_queue_size();
        }

        // Active subscriber processes messages (updates last_message_time)
        active_stats.record_message_processed();

        // Wait for STALE_SUBSCRIBER_TIMEOUT + a bit more to ensure staleness
        // Since STALE_SUBSCRIBER_TIMEOUT is 300 seconds, we'll simulate this
        // by not calling record_message_processed() on stale subscriber

        // Check stale subscribers - stale should be detected, active should not
        let stale_subscribers = manager.check_stale_subscribers();

        // The stale subscriber should be detected (high queue + no recent message processing)
        assert_eq!(stale_subscribers.len(), 1);
        assert!(stale_subscribers.contains(&"stale".to_string()));
        assert!(!stale_subscribers.contains(&"active".to_string()));

        // Test completed - no need to actually receive messages
    }

    #[tokio::test]
    async fn test_auto_unsubscribe_stale_subscribers() {
        let mut manager = AsyncNotificationManager::new();

        // Subscribe multiple subscribers
        let _stale1 = manager.subscribe(
            "stale1".to_string(),
            EventFilter::All,
            "test:stale1".to_string()
        ).expect("Should subscribe successfully");

        let _stale2 = manager.subscribe(
            "stale2".to_string(),
            EventFilter::All,
            "test:stale2".to_string()
        ).expect("Should subscribe successfully");

        let _active = manager.subscribe(
            "active".to_string(),
            EventFilter::All,
            "test:active".to_string()
        ).expect("Should subscribe successfully");

        // Initially 3 subscribers
        assert_eq!(manager.subscriber_count(), 3);

        // Make stale1 and stale2 stale (high queue, no recent processing)
        let stale1_stats = manager.get_subscriber_statistics("stale1").unwrap();
        let stale2_stats = manager.get_subscriber_statistics("stale2").unwrap();
        let active_stats = manager.get_subscriber_statistics("active").unwrap();

        // Set high queue sizes for stale subscribers
        for _ in 0..HIGH_WATER_MARK {
            stale1_stats.increment_queue_size();
            stale2_stats.increment_queue_size();
        }

        // Active subscriber processes messages recently
        active_stats.record_message_processed();

        // Auto-unsubscribe stale subscribers
        let removed = manager.auto_unsubscribe_stale();

        // Should have removed 2 stale subscribers
        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&"stale1".to_string()));
        assert!(removed.contains(&"stale2".to_string()));
        assert!(!removed.contains(&"active".to_string()));

        // Only active subscriber should remain
        assert_eq!(manager.subscriber_count(), 1);
        assert!(!manager.has_subscriber("stale1"));
        assert!(!manager.has_subscriber("stale2"));
        assert!(manager.has_subscriber("active"));
    }

    #[tokio::test]
    async fn test_intelligent_error_logging() {
        let mut manager = AsyncNotificationManager::new();

        // Subscribe a subscriber we'll simulate problems with
        let _receiver = manager.subscribe(
            "problematic".to_string(),
            EventFilter::All,
            "test:problematic".to_string()
        ).expect("Should subscribe successfully");

        let stats = manager.get_subscriber_statistics("problematic").unwrap();

        // Simulate processing some messages with errors
        for _ in 0..100 {
            stats.record_message_processed();
        }

        // Simulate errors (15% error rate - above threshold)
        for _ in 0..15 {
            stats.record_error();
        }

        // Set high queue size to trigger logging
        for _ in 0..HIGH_WATER_MARK {
            stats.increment_queue_size();
        }

        // This should trigger health logging due to high error rate and high queue
        manager.log_subscriber_health("problematic");

        // Verify error log time was recorded
        assert!(stats.last_error_log_time().is_some());

        // Test that logging is rate-limited
        // Immediately try to log again - should not log due to rate limiting
        let first_log_time = stats.last_error_log_time().unwrap();

        // Add more errors
        for _ in 0..5 {
            stats.record_error();
        }

        // Try to log again immediately
        manager.log_subscriber_health("problematic");

        // Log time should be unchanged (rate limited)
        assert_eq!(stats.last_error_log_time().unwrap(), first_log_time);

        // Test normal subscriber doesn't trigger logging
        let _normal_receiver = manager.subscribe(
            "normal".to_string(),
            EventFilter::All,
            "test:normal".to_string()
        ).expect("Should subscribe successfully");

        let normal_stats = manager.get_subscriber_statistics("normal").unwrap();

        // Low error rate
        for _ in 0..100 {
            normal_stats.record_message_processed();
        }
        for _ in 0..2 {  // 2% error rate - below threshold
            normal_stats.record_error();
        }

        // Should not trigger logging (low error rate, low queue size)
        manager.log_subscriber_health("normal");

        // No error log time should be recorded
        assert!(normal_stats.last_error_log_time().is_none());
    }

    #[tokio::test]
    async fn test_subscriber_health_assessment() {
        let mut manager = AsyncNotificationManager::new();

        // Create different types of problematic subscribers

        // 1. High water mark subscriber
        let _high_queue = manager.subscribe(
            "high_queue".to_string(),
            EventFilter::All,
            "test:high_queue".to_string()
        ).expect("Should subscribe successfully");

        // 2. Stale subscriber (high queue + no recent processing)
        let _stale = manager.subscribe(
            "stale".to_string(),
            EventFilter::All,
            "test:stale".to_string()
        ).expect("Should subscribe successfully");

        // 3. Error-prone subscriber
        let _error_prone = manager.subscribe(
            "error_prone".to_string(),
            EventFilter::All,
            "test:error_prone".to_string()
        ).expect("Should subscribe successfully");

        // 4. Healthy subscriber
        let _healthy = manager.subscribe(
            "healthy".to_string(),
            EventFilter::All,
            "test:healthy".to_string()
        ).expect("Should subscribe successfully");

        // Configure high_queue subscriber
        let high_queue_stats = manager.get_subscriber_statistics("high_queue").unwrap();
        for _ in 0..HIGH_WATER_MARK {
            high_queue_stats.increment_queue_size();
        }
        high_queue_stats.record_message_processed(); // Recent activity

        // Configure stale subscriber
        let stale_stats = manager.get_subscriber_statistics("stale").unwrap();
        for _ in 0..HIGH_WATER_MARK {
            stale_stats.increment_queue_size();
        }
        // No recent message processing - will be considered stale

        // Configure error_prone subscriber
        let error_prone_stats = manager.get_subscriber_statistics("error_prone").unwrap();
        for _ in 0..100 {
            error_prone_stats.record_message_processed();
        }
        for _ in 0..15 { // 15% error rate - above threshold
            error_prone_stats.record_error();
        }

        // Configure healthy subscriber
        let healthy_stats = manager.get_subscriber_statistics("healthy").unwrap();
        for _ in 0..100 {
            healthy_stats.record_message_processed();
        }
        for _ in 0..2 { // 2% error rate - below threshold
            healthy_stats.record_error();
        }

        // Assess health
        let assessment = manager.assess_subscriber_health();

        // Check high water mark detection
        assert_eq!(assessment.high_water_mark_subscribers.len(), 2); // high_queue and stale
        assert!(assessment.high_water_mark_subscribers.contains(&"high_queue".to_string()));
        assert!(assessment.high_water_mark_subscribers.contains(&"stale".to_string()));

        // Check stale detection
        assert_eq!(assessment.stale_subscribers.len(), 1);
        assert!(assessment.stale_subscribers.contains(&"stale".to_string()));

        // Check error-prone detection
        assert_eq!(assessment.error_prone_subscribers.len(), 1);
        assert!(assessment.error_prone_subscribers.contains(&"error_prone".to_string()));

        // Healthy subscriber should not appear in any problematic category
        assert!(!assessment.high_water_mark_subscribers.contains(&"healthy".to_string()));
        assert!(!assessment.stale_subscribers.contains(&"healthy".to_string()));
        assert!(!assessment.error_prone_subscribers.contains(&"healthy".to_string()));
    }

    #[tokio::test]
    async fn test_error_handling_closed_channels() {
        use crate::notifications::event::{Event, SystemEvent, SystemEventType};
        use crate::notifications::error::NotificationError;

        let mut manager = AsyncNotificationManager::new();

        // Subscribe a subscriber and immediately drop the receiver
        let receiver = manager.subscribe(
            "will_drop".to_string(),
            EventFilter::All,
            "test:will_drop".to_string()
        ).expect("Should subscribe successfully");

        // Subscribe a normal subscriber that keeps its receiver
        let mut normal_receiver = manager.subscribe(
            "normal".to_string(),
            EventFilter::All,
            "test:normal".to_string()
        ).expect("Should subscribe successfully");

        // Drop the first receiver to close the channel
        drop(receiver);

        // Try to publish an event
        let event = Event::System(SystemEvent::new(SystemEventType::Startup));
        let result = manager.publish(event.clone()).await;

        // Should return error indicating publish failure
        assert!(result.is_err());

        if let Err(NotificationError::PublishFailed { event_type: _, failed_subscribers }) = result {
            assert_eq!(failed_subscribers.len(), 1);
            assert!(failed_subscribers.contains(&"will_drop".to_string()));
        } else {
            panic!("Expected PublishFailed error");
        }

        // Verify the failed subscriber was automatically removed
        assert_eq!(manager.subscriber_count(), 1);
        assert!(!manager.has_subscriber("will_drop"));
        assert!(manager.has_subscriber("normal"));

        // Normal subscriber should still be able to receive events
        let event2 = Event::System(SystemEvent::new(SystemEventType::Shutdown));
        let result2 = manager.publish(event2).await;
        assert!(result2.is_ok());

        // Normal receiver should get the event
        let received = normal_receiver.recv().await.expect("Should receive event");
        assert!(matches!(received, Event::System(_)));
    }

    #[tokio::test]
    async fn test_fatal_error_out_of_memory() {
        use crate::notifications::error::NotificationError;

        let mut manager = AsyncNotificationManager::new();

        // Create multiple subscribers with very large queues to simulate OOM
        for i in 0..5 {
            let subscriber_id = format!("overloaded_{}", i);
            let _receiver = manager.subscribe(
                subscriber_id.clone(),
                EventFilter::All,
                format!("test:overloaded_{}", i)
            ).expect("Should subscribe successfully");

            // Simulate massive queue sizes (200k+1 each = 1M+5 total)
            let stats = manager.get_subscriber_statistics(&subscriber_id).unwrap();
            for _ in 0..200_001 {
                stats.increment_queue_size();
            }
        }

        // Check for memory exhaustion
        let result = manager.check_memory_exhaustion();
        assert!(result.is_err());

        if let Err(NotificationError::OutOfMemory { queue_sizes, total_events }) = result {
            assert_eq!(queue_sizes.len(), 5);
            assert_eq!(total_events, 1_000_005);
        } else {
            panic!("Expected OutOfMemory error");
        }
    }

    #[tokio::test]
    async fn test_fatal_error_system_overload() {
        use crate::notifications::error::NotificationError;

        let mut manager = AsyncNotificationManager::new();

        // Create many subscribers with problematic states
        for i in 0..10 {
            let subscriber_id = format!("problematic_{}", i);
            let _receiver = manager.subscribe(
                subscriber_id.clone(),
                EventFilter::All,
                format!("test:problematic_{}", i)
            ).expect("Should subscribe successfully");

            // Make them all have high water marks (problematic)
            let stats = manager.get_subscriber_statistics(&subscriber_id).unwrap();
            for _ in 0..HIGH_WATER_MARK {
                stats.increment_queue_size();
            }
        }

        // Check for system overload
        let result = manager.check_system_overload();
        assert!(result.is_err());

        if let Err(NotificationError::SystemOverload { active_subscribers, high_water_mark_count, stale_count: _ }) = result {
            assert_eq!(active_subscribers, 10);
            assert_eq!(high_water_mark_count, 10); // All subscribers at high water mark
        } else {
            panic!("Expected SystemOverload error");
        }
    }

    #[tokio::test]
    async fn test_graceful_degradation() {
        use crate::notifications::event::{Event, SystemEvent, SystemEventType};
        use crate::notifications::error::NotificationError;

        let mut manager = AsyncNotificationManager::new();

        // Create a mix of healthy and problematic subscribers

        // Healthy subscriber
        let mut healthy_receiver = manager.subscribe(
            "healthy".to_string(),
            EventFilter::All,
            "test:healthy".to_string()
        ).expect("Should subscribe successfully");

        // Problematic subscriber that will be dropped
        let dropped_receiver = manager.subscribe(
            "will_be_dropped".to_string(),
            EventFilter::All,
            "test:dropped".to_string()
        ).expect("Should subscribe successfully");

        // Verify both are registered
        assert_eq!(manager.subscriber_count(), 2);

        // Drop the problematic subscriber
        drop(dropped_receiver);

        // Publish event - should handle the failure gracefully
        let event = Event::System(SystemEvent::new(SystemEventType::Startup));
        let result = manager.publish(event.clone()).await;

        // Should return error but system continues to operate
        assert!(result.is_err());
        if let Err(NotificationError::PublishFailed { failed_subscribers, .. }) = result {
            assert_eq!(failed_subscribers.len(), 1);
            assert!(failed_subscribers.contains(&"will_be_dropped".to_string()));
        }

        // System should have automatically cleaned up the failed subscriber
        assert_eq!(manager.subscriber_count(), 1);
        assert!(manager.has_subscriber("healthy"));
        assert!(!manager.has_subscriber("will_be_dropped"));

        // Healthy subscriber should still work
        let event2 = Event::System(SystemEvent::new(SystemEventType::Shutdown));
        let result2 = manager.publish(event2).await;
        assert!(result2.is_ok());

        // Healthy subscriber should receive the event
        let received = healthy_receiver.recv().await.expect("Should receive event");
        assert!(matches!(received, Event::System(_)));
    }

    // Integration Tests

    #[tokio::test]
    async fn test_integration_multi_subscriber_scenario() {
        use crate::notifications::event::{Event, ScanEvent, ScanEventType, QueueEvent, QueueEventType, SystemEvent, SystemEventType};

        let mut manager = AsyncNotificationManager::new();

        // Create subscribers with different filters and sources
        let mut scan_logger = manager.subscribe(
            "scan_logger".to_string(),
            EventFilter::ScanOnly,
            "logger:scan_events".to_string()
        ).expect("Should subscribe successfully");

        let mut queue_monitor = manager.subscribe(
            "queue_monitor".to_string(),
            EventFilter::QueueOnly,
            "monitor:queue_status".to_string()
        ).expect("Should subscribe successfully");

        let mut system_admin = manager.subscribe(
            "system_admin".to_string(),
            EventFilter::SystemOnly,
            "admin:system_events".to_string()
        ).expect("Should subscribe successfully");

        let mut audit_logger = manager.subscribe(
            "audit_logger".to_string(),
            EventFilter::All,
            "audit:all_events".to_string()
        ).expect("Should subscribe successfully");

        // Verify all subscribers are registered
        assert_eq!(manager.subscriber_count(), 4);

        // Publish different types of events
        let scan_event = Event::Scan(ScanEvent::new(ScanEventType::Started, "repo_1".to_string()));
        let queue_event = Event::Queue(QueueEvent::new(QueueEventType::MessageAdded, "task_1".to_string()));
        let system_event = Event::System(SystemEvent::new(SystemEventType::Startup));

        // Publish all events
        let _ = manager.publish(scan_event.clone()).await;
        let _ = manager.publish(queue_event.clone()).await;
        let _ = manager.publish(system_event.clone()).await;

        // Verify filtering works correctly

        // Scan logger should only receive scan event
        let scan_received = scan_logger.recv().await.expect("Should receive scan event");
        assert!(matches!(scan_received, Event::Scan(_)));

        // Queue monitor should only receive queue event
        let queue_received = queue_monitor.recv().await.expect("Should receive queue event");
        assert!(matches!(queue_received, Event::Queue(_)));

        // System admin should only receive system event
        let system_received = system_admin.recv().await.expect("Should receive system event");
        assert!(matches!(system_received, Event::System(_)));

        // Audit logger should receive all three events
        let mut audit_events = Vec::new();
        for _ in 0..3 {
            let event = audit_logger.recv().await.expect("Should receive event");
            audit_events.push(event);
        }

        // Count event types received by audit logger
        let mut scan_count = 0;
        let mut queue_count = 0;
        let mut system_count = 0;

        for event in audit_events {
            match event {
                Event::Scan(_) => scan_count += 1,
                Event::Queue(_) => queue_count += 1,
                Event::System(_) => system_count += 1,
                _ => {}
            }
        }

        assert_eq!(scan_count, 1);
        assert_eq!(queue_count, 1);
        assert_eq!(system_count, 1);

        // Verify statistics tracking
        for subscriber_id in ["scan_logger", "queue_monitor", "system_admin", "audit_logger"] {
            let stats = manager.get_subscriber_statistics(subscriber_id).unwrap();
            if subscriber_id == "audit_logger" {
                assert_eq!(stats.queue_size(), 3); // Received all events
            } else {
                assert_eq!(stats.queue_size(), 1); // Received filtered event
            }
        }
    }

    #[tokio::test]
    async fn test_integration_subscriber_lifecycle() {
        use crate::notifications::event::{Event, SystemEvent, SystemEventType};

        let mut manager = AsyncNotificationManager::new();

        // Phase 1: Subscribe
        let mut receiver = manager.subscribe(
            "lifecycle_test".to_string(),
            EventFilter::SystemOnly,
            "test:lifecycle".to_string()
        ).expect("Should subscribe successfully");

        assert_eq!(manager.subscriber_count(), 1);
        assert!(manager.has_subscriber("lifecycle_test"));

        {
            let stats = manager.get_subscriber_statistics("lifecycle_test").unwrap();
            assert_eq!(stats.queue_size(), 0);
            assert_eq!(stats.messages_processed(), 0);
        }

        // Phase 2: Receive events
        let event1 = Event::System(SystemEvent::new(SystemEventType::Startup));
        let event2 = Event::System(SystemEvent::new(SystemEventType::Shutdown));

        // Publish events
        let result1 = manager.publish(event1).await;
        let result2 = manager.publish(event2).await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // Verify queue size increased
        let stats = manager.get_subscriber_statistics("lifecycle_test").unwrap();
        assert_eq!(stats.queue_size(), 2);

        // Receive events
        let received1 = receiver.recv().await.expect("Should receive first event");
        assert!(matches!(received1, Event::System(_)));

        // Simulate subscriber processing (decrement queue size, record processing)
        stats.decrement_queue_size();
        stats.record_message_processed();

        let received2 = receiver.recv().await.expect("Should receive second event");
        assert!(matches!(received2, Event::System(_)));

        stats.decrement_queue_size();
        stats.record_message_processed();

        // Verify processing statistics
        assert_eq!(stats.queue_size(), 0);
        assert_eq!(stats.messages_processed(), 2);
        assert!(stats.last_message_time().is_some());

        // Phase 3: Unsubscribe (drop receiver)
        drop(receiver);

        // Publish another event - should detect closed channel
        let event3 = Event::System(SystemEvent::new(SystemEventType::Startup));
        let result3 = manager.publish(event3).await;

        // Should return error but clean up automatically
        assert!(result3.is_err());

        // Subscriber should be automatically removed
        assert_eq!(manager.subscriber_count(), 0);
        assert!(!manager.has_subscriber("lifecycle_test"));
    }

    #[tokio::test]
    async fn test_integration_auto_management_under_load() {
        use crate::notifications::event::{Event, SystemEvent, SystemEventType};

        let mut manager = AsyncNotificationManager::new();

        // Create a mix of subscriber types
        let mut healthy_subscriber = manager.subscribe(
            "healthy".to_string(),
            EventFilter::All,
            "test:healthy".to_string()
        ).expect("Should subscribe successfully");

        let overloaded_receiver = manager.subscribe(
            "overloaded".to_string(),
            EventFilter::All,
            "test:overloaded".to_string()
        ).expect("Should subscribe successfully");

        let _stale_receiver = manager.subscribe(
            "stale".to_string(),
            EventFilter::All,
            "test:stale".to_string()
        ).expect("Should subscribe successfully");

        let error_prone_receiver = manager.subscribe(
            "error_prone".to_string(),
            EventFilter::All,
            "test:error_prone".to_string()
        ).expect("Should subscribe successfully");

        // Initial state - all healthy
        assert_eq!(manager.subscriber_count(), 4);

        // Simulate load conditions

        // 1. Overloaded subscriber - simulate high queue
        let overloaded_stats = manager.get_subscriber_statistics("overloaded").unwrap();
        for _ in 0..HIGH_WATER_MARK + 100 {
            overloaded_stats.increment_queue_size();
        }
        overloaded_stats.record_message_processed(); // Recent activity

        // 2. Stale subscriber - high queue + no activity
        let stale_stats = manager.get_subscriber_statistics("stale").unwrap();
        for _ in 0..HIGH_WATER_MARK + 50 {
            stale_stats.increment_queue_size();
        }
        // No recent activity - will be considered stale

        // 3. Error-prone subscriber - high error rate
        let error_prone_stats = manager.get_subscriber_statistics("error_prone").unwrap();
        for _ in 0..100 {
            error_prone_stats.record_message_processed();
        }
        for _ in 0..20 { // 20% error rate - above threshold
            error_prone_stats.record_error();
        }

        // 4. Healthy subscriber - processes messages normally
        let healthy_stats = manager.get_subscriber_statistics("healthy").unwrap();
        for _ in 0..50 {
            healthy_stats.record_message_processed();
        }
        for _ in 0..2 { // 4% error rate - below threshold
            healthy_stats.record_error();
        }

        // Perform health maintenance
        let assessment = manager.perform_health_maintenance().await;

        // Verify health assessment detects problems
        assert!(!assessment.high_water_mark_subscribers.is_empty());
        assert!(!assessment.stale_subscribers.is_empty());
        assert!(!assessment.error_prone_subscribers.is_empty());

        // Check specific categorization
        assert!(assessment.high_water_mark_subscribers.contains(&"overloaded".to_string()));
        assert!(assessment.high_water_mark_subscribers.contains(&"stale".to_string()));
        assert!(assessment.stale_subscribers.contains(&"stale".to_string()));
        assert!(assessment.error_prone_subscribers.contains(&"error_prone".to_string()));

        // Healthy subscriber should not appear in any problematic category
        assert!(!assessment.high_water_mark_subscribers.contains(&"healthy".to_string()));
        assert!(!assessment.stale_subscribers.contains(&"healthy".to_string()));
        assert!(!assessment.error_prone_subscribers.contains(&"healthy".to_string()));

        // Stale subscriber should have been auto-unsubscribed
        assert_eq!(manager.subscriber_count(), 3); // One less due to stale cleanup
        assert!(!manager.has_subscriber("stale"));

        // Verify system continues to operate normally
        let event = Event::System(SystemEvent::new(SystemEventType::Startup));
        let result = manager.publish(event).await;

        // Should succeed for remaining subscribers
        assert!(result.is_ok());

        // Healthy subscriber should receive the event
        let received = healthy_subscriber.recv().await.expect("Should receive event");
        assert!(matches!(received, Event::System(_)));

        // Clean up receivers to avoid warnings
        drop(overloaded_receiver);
        drop(error_prone_receiver);
    }

    #[tokio::test]
    async fn test_duplicate_subscriber_warning() {
        let mut manager = AsyncNotificationManager::new();

        // Subscribe first time
        let _receiver1 = manager.subscribe(
            "duplicate_test".to_string(),
            EventFilter::ScanOnly,
            "test:original".to_string()
        ).expect("Should subscribe successfully");

        assert_eq!(manager.subscriber_count(), 1);

        // Subscribe with same ID but different source - should warn and replace
        let _receiver2 = manager.subscribe(
            "duplicate_test".to_string(),
            EventFilter::All,
            "test:replacement".to_string()
        ).expect("Should subscribe successfully");

        // Should still have only 1 subscriber (replaced, not added)
        assert_eq!(manager.subscriber_count(), 1);

        // Verify the replacement took effect by checking the source would be updated
        // (We can't directly check the source without exposing internal state)
        assert!(manager.has_subscriber("duplicate_test"));
    }

    #[tokio::test]
    async fn test_integration_complete_system_end_to_end() {
        use crate::notifications::event::{Event, ScanEvent, ScanEventType, QueueEvent, QueueEventType, SystemEvent, SystemEventType, PluginEvent, PluginEventType};
        use crate::notifications::error::NotificationError;

        let mut manager = AsyncNotificationManager::new();

        // Create realistic subscriber ecosystem
        let mut scan_processor = manager.subscribe(
            "scan_processor".to_string(),
            EventFilter::ScanOnly,
            "processor:scan_results".to_string()
        ).expect("Should subscribe successfully");

        let mut queue_manager = manager.subscribe(
            "queue_manager".to_string(),
            EventFilter::QueueOnly,
            "manager:task_queue".to_string()
        ).expect("Should subscribe successfully");

        let mut plugin_monitor = manager.subscribe(
            "plugin_monitor".to_string(),
            EventFilter::PluginOnly,
            "monitor:plugin_status".to_string()
        ).expect("Should subscribe successfully");

        let mut system_logger = manager.subscribe(
            "system_logger".to_string(),
            EventFilter::SystemOnly,
            "logger:system_events".to_string()
        ).expect("Should subscribe successfully");

        let mut audit_trail = manager.subscribe(
            "audit_trail".to_string(),
            EventFilter::All,
            "audit:complete_trail".to_string()
        ).expect("Should subscribe successfully");

        // Simulate real-world event sequence
        let events = vec![
            Event::System(SystemEvent::new(SystemEventType::Startup)),
            Event::Scan(ScanEvent::new(ScanEventType::Started, "project_a".to_string())),
            Event::Queue(QueueEvent::new(QueueEventType::MessageAdded, "scan_task_1".to_string())),
            Event::Plugin(PluginEvent::new(PluginEventType::Processing, "exporter_v1".to_string())),
            Event::Scan(ScanEvent::new(ScanEventType::Completed, "project_a".to_string())),
            Event::Queue(QueueEvent::new(QueueEventType::MessageProcessed, "scan_task_1".to_string())),
            Event::Plugin(PluginEvent::new(PluginEventType::DataReady, "exporter_v1".to_string())),
            Event::System(SystemEvent::new(SystemEventType::Shutdown)),
        ];

        // Publish all events
        let mut publish_results = Vec::new();
        for event in &events {
            let result = manager.publish(event.clone()).await;
            publish_results.push(result);
        }

        // Verify all publishes succeeded
        for (i, result) in publish_results.iter().enumerate() {
            assert!(result.is_ok(), "Event {} failed to publish: {:?}", i, result);
        }

        // Verify each subscriber received correct events

        // Scan processor should get 2 scan events
        let scan_event1 = scan_processor.recv().await.expect("Should receive scan event");
        let scan_event2 = scan_processor.recv().await.expect("Should receive scan event");
        assert!(matches!(scan_event1, Event::Scan(_)));
        assert!(matches!(scan_event2, Event::Scan(_)));

        // Queue manager should get 2 queue events
        let queue_event1 = queue_manager.recv().await.expect("Should receive queue event");
        let queue_event2 = queue_manager.recv().await.expect("Should receive queue event");
        assert!(matches!(queue_event1, Event::Queue(_)));
        assert!(matches!(queue_event2, Event::Queue(_)));

        // Plugin monitor should get 2 plugin events
        let plugin_event1 = plugin_monitor.recv().await.expect("Should receive plugin event");
        let plugin_event2 = plugin_monitor.recv().await.expect("Should receive plugin event");
        assert!(matches!(plugin_event1, Event::Plugin(_)));
        assert!(matches!(plugin_event2, Event::Plugin(_)));

        // System logger should get 2 system events
        let system_event1 = system_logger.recv().await.expect("Should receive system event");
        let system_event2 = system_logger.recv().await.expect("Should receive system event");
        assert!(matches!(system_event1, Event::System(_)));
        assert!(matches!(system_event2, Event::System(_)));

        // Audit trail should get all 8 events
        let mut audit_events = Vec::new();
        for _ in 0..8 {
            let event = audit_trail.recv().await.expect("Should receive event");
            audit_events.push(event);
        }
        assert_eq!(audit_events.len(), 8);

        // Verify event type distribution in audit trail
        let mut event_counts = std::collections::HashMap::new();
        for event in &audit_events {
            let event_type = match event {
                Event::System(_) => "System",
                Event::Scan(_) => "Scan",
                Event::Queue(_) => "Queue",
                Event::Plugin(_) => "Plugin",
            };
            *event_counts.entry(event_type).or_insert(0) += 1;
        }

        assert_eq!(event_counts["System"], 2);
        assert_eq!(event_counts["Scan"], 2);
        assert_eq!(event_counts["Queue"], 2);
        assert_eq!(event_counts["Plugin"], 2);

        // Verify statistics tracking
        for subscriber_id in ["scan_processor", "queue_manager", "plugin_monitor", "system_logger", "audit_trail"] {
            let stats = manager.get_subscriber_statistics(subscriber_id).unwrap();

            match subscriber_id {
                "audit_trail" => assert_eq!(stats.queue_size(), 8),
                _ => assert_eq!(stats.queue_size(), 2),
            }
        }

        // Test health assessment on working system
        let assessment = manager.assess_subscriber_health();
        assert!(assessment.high_water_mark_subscribers.is_empty());
        assert!(assessment.stale_subscribers.is_empty());
        assert!(assessment.error_prone_subscribers.is_empty());

        // Test error detection capabilities
        let memory_check = manager.check_memory_exhaustion();
        let overload_check = manager.check_system_overload();
        assert!(memory_check.is_ok());
        assert!(overload_check.is_ok());

        // Verify final system state
        assert_eq!(manager.subscriber_count(), 5);

        // Test graceful cleanup when subscribers disconnect
        drop(scan_processor);
        drop(queue_manager);

        // Publish events that the dropped subscribers would have received
        let scan_cleanup = Event::Scan(ScanEvent::new(ScanEventType::Started, "cleanup_test".to_string()));
        let queue_cleanup = Event::Queue(QueueEvent::new(QueueEventType::MessageAdded, "cleanup_test".to_string()));

        let result1 = manager.publish(scan_cleanup).await;
        let result2 = manager.publish(queue_cleanup).await;

        // Should report failures for the appropriate dropped subscribers
        assert!(result1.is_err());
        assert!(result2.is_err());

        if let Err(NotificationError::PublishFailed { failed_subscribers, .. }) = result1 {
            assert_eq!(failed_subscribers.len(), 1);
            assert!(failed_subscribers.contains(&"scan_processor".to_string()));
        }

        if let Err(NotificationError::PublishFailed { failed_subscribers, .. }) = result2 {
            assert_eq!(failed_subscribers.len(), 1);
            assert!(failed_subscribers.contains(&"queue_manager".to_string()));
        }

        // System should have automatically cleaned up the failed subscribers
        assert_eq!(manager.subscriber_count(), 3);
        assert!(!manager.has_subscriber("scan_processor"));
        assert!(!manager.has_subscriber("queue_manager"));
        assert!(manager.has_subscriber("plugin_monitor"));
        assert!(manager.has_subscriber("system_logger"));
        assert!(manager.has_subscriber("audit_trail"));
    }
}