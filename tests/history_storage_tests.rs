use chrono::Utc;
use jarvis_rust::history::{HistoryStorage, Message};
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_in_memory_storage() {
    let storage = HistoryStorage::new(":memory:").await.unwrap();
    // Note: db field is private, we'll test functionality instead of internal structure

    // Test saving and retrieving messages
    let session_id = "test-session";
    let user_msg = Message::user(session_id.to_string(), "Hello".to_string());
    let assistant_msg = Message::assistant(session_id.to_string(), "Hi there!".to_string());

    storage.save(user_msg.clone()).await.unwrap();
    storage.save(assistant_msg.clone()).await.unwrap();

    let messages = storage.list(session_id).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "Hello");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "Hi there!");
}

#[tokio::test]
async fn test_file_database_storage() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_path_str = db_path.to_string_lossy().to_string();

    let storage = HistoryStorage::new(&db_path_str).await.unwrap();
    // Note: db field is private, we'll test functionality instead of internal structure

    // Test saving and retrieving messages
    let session_id = "file-test-session";
    let messages_to_save = vec![
        Message::system(session_id.to_string(), "System prompt".to_string()),
        Message::user(session_id.to_string(), "What's the weather?".to_string()),
        Message::assistant(session_id.to_string(), "It's sunny today".to_string()),
    ];

    for msg in &messages_to_save {
        storage.save(msg.clone()).await.unwrap();
    }

    let retrieved_messages = storage.list(session_id).await.unwrap();
    assert_eq!(retrieved_messages.len(), 3);

    for (i, msg) in retrieved_messages.iter().enumerate() {
        assert_eq!(msg.role, messages_to_save[i].role);
        assert_eq!(msg.content, messages_to_save[i].content);
        assert_eq!(msg.session_id, session_id);
        assert!(msg.id.is_some()); // Should have ID from database
    }
}

#[tokio::test]
async fn test_fallback_storage_when_db_fails() {
    // Use an invalid path to force database initialization failure
    let invalid_path = "/invalid/path/to/database.db";
    let storage = HistoryStorage::new(invalid_path).await.unwrap();
    // Note: db field is private, we'll test functionality instead of internal structure

    // Test that fallback storage works
    let session_id = "fallback-test";
    let msg = Message::user(session_id.to_string(), "Test message".to_string());
    storage.save(msg.clone()).await.unwrap();

    let messages = storage.list(session_id).await.unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Test message");
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].session_id, session_id);
    assert!(messages[0].id.is_none()); // Fallback doesn't set ID
}

#[tokio::test]
async fn test_multiple_sessions() {
    let storage = HistoryStorage::new(":memory:").await.unwrap();
    let session1 = "session-1";
    let session2 = "session-2";

    // Add messages to different sessions
    storage
        .save(Message::user(
            session1.to_string(),
            "Session 1 message 1".to_string(),
        ))
        .await
        .unwrap();
    storage
        .save(Message::user(
            session2.to_string(),
            "Session 2 message 1".to_string(),
        ))
        .await
        .unwrap();
    storage
        .save(Message::user(
            session1.to_string(),
            "Session 1 message 2".to_string(),
        ))
        .await
        .unwrap();

    // Retrieve messages for each session
    let session1_messages = storage.list(session1).await.unwrap();
    let session2_messages = storage.list(session2).await.unwrap();

    assert_eq!(session1_messages.len(), 2);
    assert_eq!(session2_messages.len(), 1);
    assert_eq!(session1_messages[0].content, "Session 1 message 1");
    assert_eq!(session1_messages[1].content, "Session 1 message 2");
    assert_eq!(session2_messages[0].content, "Session 2 message 1");
}

#[tokio::test]
async fn test_empty_session() {
    let storage = HistoryStorage::new(":memory:").await.unwrap();
    let messages = storage.list("nonexistent-session").await.unwrap();
    assert!(messages.is_empty());
}

#[tokio::test]
async fn test_message_ordering() {
    let storage = HistoryStorage::new(":memory:").await.unwrap();
    let session_id = "ordering-test";

    // Add messages with small delays to ensure different timestamps
    let messages = vec![
        Message::user(session_id.to_string(), "First message".to_string()),
        Message::assistant(session_id.to_string(), "Second message".to_string()),
        Message::user(session_id.to_string(), "Third message".to_string()),
    ];

    for msg in messages {
        storage.save(msg).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
    }

    let retrieved = storage.list(session_id).await.unwrap();
    assert_eq!(retrieved.len(), 3);
    assert_eq!(retrieved[0].content, "First message");
    assert_eq!(retrieved[1].content, "Second message");
    assert_eq!(retrieved[2].content, "Third message");
}

#[tokio::test]
async fn test_message_creation_helpers() {
    let session_id = "test-session".to_string();

    let user_msg = Message::user(session_id.clone(), "User input".to_string());
    assert_eq!(user_msg.role, "user");
    assert_eq!(user_msg.content, "User input");
    assert_eq!(user_msg.session_id, session_id);
    assert!(user_msg.id.is_none());

    let assistant_msg = Message::assistant(session_id.clone(), "Assistant response".to_string());
    assert_eq!(assistant_msg.role, "assistant");
    assert_eq!(assistant_msg.content, "Assistant response");

    let system_msg = Message::system(session_id.clone(), "System prompt".to_string());
    assert_eq!(system_msg.role, "system");
    assert_eq!(system_msg.content, "System prompt");

    let tool_msg = Message::tool(session_id.clone(), "Tool result".to_string());
    assert_eq!(tool_msg.role, "tool");
    assert_eq!(tool_msg.content, "Tool result");
}

#[tokio::test]
async fn test_message_timestamps() {
    let before = Utc::now();
    let msg = Message::new(
        "test".to_string(),
        "user".to_string(),
        "content".to_string(),
    );
    let after = Utc::now();

    assert!(msg.created_at >= before && msg.created_at <= after);
}

#[tokio::test]
async fn test_concurrent_access() {
    let storage = Arc::new(HistoryStorage::new(":memory:").await.unwrap());
    let session_id = "concurrent-test";
    let mut handles = vec![];

    // Spawn multiple tasks that save messages concurrently
    for i in 0..10 {
        let storage_clone = Arc::clone(&storage);
        let session_id = session_id.to_string();
        let handle = tokio::spawn(async move {
            let msg = Message::user(session_id, format!("Message {}", i));
            storage_clone.save(msg).await
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Verify all messages were saved
    let messages = storage.list(session_id).await.unwrap();
    assert_eq!(messages.len(), 10);
}

#[tokio::test]
async fn test_fallback_concurrent_access() {
    // Force fallback storage by using invalid path
    let storage = Arc::new(HistoryStorage::new("/invalid/path").await.unwrap());
    let session_id = "fallback-concurrent-test";
    let mut handles = vec![];

    // Spawn multiple tasks that save messages concurrently to fallback storage
    for i in 0..5 {
        let storage_clone = Arc::clone(&storage);
        let session_id = session_id.to_string();
        let handle = tokio::spawn(async move {
            let msg = Message::assistant(session_id, format!("Fallback message {}", i));
            storage_clone.save(msg).await
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Verify all messages were saved to fallback
    let messages = storage.list(session_id).await.unwrap();
    assert_eq!(messages.len(), 5);

    // All messages should have no ID (fallback storage)
    for msg in &messages {
        assert!(msg.id.is_none());
    }
}

#[tokio::test]
async fn test_large_content() {
    let storage = HistoryStorage::new(":memory:").await.unwrap();
    let session_id = "large-content-test";

    // Create a message with large content
    let large_content = "x".repeat(10000);
    let msg = Message::user(session_id.to_string(), large_content.clone());
    storage.save(msg).await.unwrap();

    let messages = storage.list(session_id).await.unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, large_content);
    assert_eq!(messages[0].content.len(), 10000);
}
