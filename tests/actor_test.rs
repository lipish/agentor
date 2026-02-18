use agentor::prelude::*;
use async_trait::async_trait;

/// 简单的 Echo Actor，用于测试基础 Actor 功能
struct EchoActor {
    id: ActorId,
    received: Vec<String>,
}

impl EchoActor {
    fn new(name: &str) -> Self {
        Self {
            id: ActorId::new(name),
            received: Vec::new(),
        }
    }
}

#[async_trait]
impl Actor for EchoActor {
    async fn on_start(&mut self, _ctx: &mut ActorContext) -> anyhow::Result<()> {
        Ok(())
    }

    async fn handle_message(
        &mut self,
        envelope: Envelope,
        _ctx: &mut ActorContext,
    ) -> anyhow::Result<()> {
        if let Ok(msg) = envelope.downcast::<String>() {
            self.received.push(msg);
        }
        Ok(())
    }

    async fn on_stop(&mut self, _ctx: &mut ActorContext) -> anyhow::Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        &self.id.name
    }

    fn id(&self) -> &ActorId {
        &self.id
    }
}

#[tokio::test]
async fn test_actor_system_spawn_and_shutdown() {
    let mut system = ActorSystem::new("test-system");
    let actor = EchoActor::new("echo");
    let actor_ref = system.spawn_default(Box::new(actor));

    assert!(!actor_ref.is_stopped());

    // 发送消息
    actor_ref.tell("hello".to_string()).await.unwrap();
    actor_ref.tell("world".to_string()).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // 关闭系统
    system.shutdown().await;

    // 关闭后 actor 应该已停止
    assert!(actor_ref.is_stopped());
}

#[tokio::test]
async fn test_actor_system_find_by_name() {
    let mut system = ActorSystem::new("test-find");
    let actor = EchoActor::new("finder");
    let original_ref = system.spawn_default(Box::new(actor));

    let found = system.find_by_name("finder");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id().name, original_ref.id().name);

    let not_found = system.find_by_name("nonexistent");
    assert!(not_found.is_none());

    system.shutdown().await;
}

#[tokio::test]
async fn test_agent_actor_message_handling() {
    let mut system = ActorSystem::new("test-agent");
    let agent = AgentActor::new("test-planner");
    let agent_ref = system.spawn_default(Box::new(agent));

    // 发送 UserPrompt
    agent_ref
        .tell(AgentMessage::UserPrompt("test prompt".to_string()))
        .await
        .unwrap();

    // 发送 ToolResult
    agent_ref
        .tell(AgentMessage::ToolResult {
            tool_name: "test_tool".to_string(),
            output: "result".to_string(),
        })
        .await
        .unwrap();

    // 发送纯文本（会被转为 UserPrompt）
    agent_ref.tell("plain text".to_string()).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    system.shutdown().await;
}

#[tokio::test]
async fn test_mailbox_backpressure() {
    let (mut mailbox, sender) = agentor::actor::Mailbox::new(2);

    // 发送 2 条消息（容量内）
    let e1 = Envelope::new(Box::new("msg1".to_string()), None);
    let e2 = Envelope::new(Box::new("msg2".to_string()), None);
    sender.send(e1).await.unwrap();
    sender.send(e2).await.unwrap();

    // try_send 第 3 条应该失败（信箱满）
    let e3 = Envelope::new(Box::new("msg3".to_string()), None);
    let result = sender.try_send(e3);
    assert!(result.is_err());

    // 消费一条后应该能再发
    let _ = mailbox.recv().await;
    let e4 = Envelope::new(Box::new("msg4".to_string()), None);
    sender.send(e4).await.unwrap();
}

#[tokio::test]
async fn test_token_budget() {
    let budget = TokenBudget::new("test-budget", 100);

    assert!(budget.try_consume(50));
    assert_eq!(budget.used(), 50);
    assert_eq!(budget.remaining(), 50);
    assert!(!budget.is_tripped());

    // 超出预算
    assert!(!budget.try_consume(60));
    assert!(budget.is_tripped());

    // 重置
    budget.reset();
    assert!(!budget.is_tripped());
    assert_eq!(budget.used(), 0);
}

#[tokio::test]
async fn test_environment() {
    let env = Environment::new();
    env.set_config("model", "gpt-4");
    env.set_secret("API_KEY", "sk-test");

    assert_eq!(env.get_config("model"), Some("gpt-4".to_string()));
    assert_eq!(env.get_secret("API_KEY"), Some("sk-test".to_string()));
    assert!(env.has_secret("API_KEY"));
    assert!(!env.has_secret("MISSING"));
    assert_eq!(env.get_config("missing"), None);
}

#[tokio::test]
async fn test_trace_collector() {
    let collector = TraceCollector::new(100);
    let actor_id = ActorId::new("test-actor");
    let trace_id = uuid::Uuid::new_v4();

    collector.log(
        trace_id,
        &actor_id,
        TraceEventType::MessageReceived,
        "received hello",
    );
    collector.log(
        trace_id,
        &actor_id,
        TraceEventType::LlmRequest,
        "calling gpt-4",
    );

    assert_eq!(collector.len(), 2);

    let events = collector.query_by_trace(&trace_id);
    assert_eq!(events.len(), 2);

    let actor_events = collector.query_by_actor(&actor_id, 10);
    assert_eq!(actor_events.len(), 2);

    collector.clear();
    assert!(collector.is_empty());
}

#[tokio::test]
async fn test_stream_producer_consumer() {
    let (producer, mut consumer) = agentor::stream::create_stream::<String>(8);

    // 生产者发送数据
    producer.send("chunk1".to_string()).await.unwrap();
    producer.send("chunk2".to_string()).await.unwrap();
    producer.finish().await.unwrap();

    // 消费者接收
    let mut chunks = Vec::new();
    while let Some(event) = consumer.next().await {
        match event {
            StreamEvent::Data(data) => chunks.push(data),
            StreamEvent::End => break,
            _ => {}
        }
    }

    assert_eq!(chunks, vec!["chunk1", "chunk2"]);
}

#[tokio::test]
async fn test_agent_state() {
    let mut state = AgentState::new(3);

    // 添加短期记忆
    for i in 0..5 {
        state.push_short_term(MemoryEntry {
            timestamp: chrono::Utc::now(),
            role: "user".to_string(),
            content: format!("msg {}", i),
            metadata: None,
        });
    }

    // 容量为 3，应该只保留最近 3 条
    assert_eq!(state.short_term.len(), 3);
    assert_eq!(state.short_term.front().unwrap().content, "msg 2");
    assert_eq!(state.short_term.back().unwrap().content, "msg 4");

    // 最近 2 条
    let recent = state.recent_memories(2);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].content, "msg 4");
}

// ============================================================
// v0.2 新特性测试
// ============================================================

// --- Transactional Mailbox ---

#[tokio::test]
async fn test_mailbox_commit() {
    let (mut mailbox, sender) = agentor::actor::Mailbox::new(8);

    let e = Envelope::new(Box::new("hello".to_string()), None);
    sender.send(e).await.unwrap();

    let envelope = mailbox.recv().await.unwrap();
    assert_eq!(envelope.downcast::<String>().unwrap(), "hello");

    // commit 后 retry_queue 应为空
    mailbox.commit();
    assert_eq!(mailbox.retry_queue_len(), 0);
    assert_eq!(mailbox.dead_letter_count(), 0);
}

#[tokio::test]
async fn test_mailbox_nack_retry() {
    let (mut mailbox, sender) = agentor::actor::Mailbox::new(8);
    mailbox.set_max_retries(3);

    let e = Envelope::new(Box::new(42u32), None);
    sender.send(e).await.unwrap();

    // 第一次 recv + nack → 进入 retry_queue
    let envelope = mailbox.recv().await.unwrap();
    mailbox.nack(envelope, "transient error");
    assert_eq!(mailbox.retry_queue_len(), 1);
    assert_eq!(mailbox.dead_letter_count(), 0);

    // 第二次 recv（从 retry_queue 取出）+ nack
    let envelope = mailbox.recv().await.unwrap();
    mailbox.nack(envelope, "transient error again");
    assert_eq!(mailbox.retry_queue_len(), 1);

    // 第三次 recv + nack → 超过 max_retries(3)，进入 DLQ
    let envelope = mailbox.recv().await.unwrap();
    mailbox.nack(envelope, "final failure");
    assert_eq!(mailbox.retry_queue_len(), 0);
    assert_eq!(mailbox.dead_letter_count(), 1);

    // 检查 DLQ 内容
    let dead_letters = mailbox.drain_dead_letters();
    assert_eq!(dead_letters.len(), 1);
    assert_eq!(dead_letters[0].last_error, "final failure");
    assert!(dead_letters[0].envelope.is_some());
}

#[tokio::test]
async fn test_mailbox_record_failure() {
    let (mut mailbox, sender) = agentor::actor::Mailbox::new(8);

    let e = Envelope::new(Box::new("data".to_string()), None);
    let msg_id = e.id;
    let trace_id = e.trace_id;
    sender.send(e).await.unwrap();

    // recv 后 envelope 被消费，用 record_failure 记录
    let _envelope = mailbox.recv().await.unwrap();
    mailbox.record_failure(msg_id, trace_id, "handle_message panicked");

    assert_eq!(mailbox.dead_letter_count(), 1);
    let dls = mailbox.drain_dead_letters();
    assert_eq!(dls[0].msg_id, msg_id);
    assert!(dls[0].envelope.is_none()); // envelope 已被消费
}

// --- Failure Classification ---

#[tokio::test]
async fn test_failure_kind_properties() {
    let transient = FailureKind::transient();
    assert!(transient.is_retryable());

    let logic = FailureKind::logic("bad JSON");
    assert!(logic.is_retryable());

    let critical = FailureKind::critical();
    assert!(!critical.is_retryable());

    let critical_silent = FailureKind::critical_silent();
    assert!(!critical_silent.is_retryable());
}

#[tokio::test]
async fn test_agent_error_extract() {
    let err = AgentError::new(FailureKind::transient(), "LLM API timeout");
    let anyhow_err: anyhow::Error = err.into();

    let extracted = AgentError::extract(&anyhow_err);
    assert!(extracted.is_some());
    let extracted = extracted.unwrap();
    assert!(extracted.kind.is_retryable());
    assert_eq!(extracted.message, "LLM API timeout");
}

// --- Stream Interception ---

#[tokio::test]
async fn test_stream_interceptor_pass() {
    use agentor::stream::{create_intercepted_stream, KeywordInterceptor};

    let interceptor = KeywordInterceptor::new(vec!["blocked".to_string()]);
    let (producer, mut consumer) = create_intercepted_stream::<String>(8, Box::new(interceptor));

    producer.send("hello".to_string()).await.unwrap();
    producer.send("world".to_string()).await.unwrap();
    producer.finish().await.unwrap();

    let mut chunks = Vec::new();
    while let Some(event) = consumer.next().await {
        match event {
            StreamEvent::Data(d) => chunks.push(d),
            StreamEvent::End => break,
            _ => break,
        }
    }

    assert_eq!(chunks, vec!["hello", "world"]);
}

#[tokio::test]
async fn test_stream_interceptor_block() {
    use agentor::stream::{create_intercepted_stream, KeywordInterceptor};

    let interceptor = KeywordInterceptor::new(vec!["danger".to_string()]);
    let (producer, mut consumer) = create_intercepted_stream::<String>(8, Box::new(interceptor));

    producer.send("safe content".to_string()).await.unwrap();
    producer
        .send("this is danger zone".to_string())
        .await
        .unwrap();

    let mut chunks = Vec::new();
    let mut got_error = false;
    while let Some(event) = consumer.next().await {
        match event {
            StreamEvent::Data(d) => chunks.push(d),
            StreamEvent::Error(e) => {
                assert!(e.contains("danger"));
                got_error = true;
                break;
            }
            _ => break,
        }
    }

    assert_eq!(chunks, vec!["safe content"]);
    assert!(got_error);
}

// --- Hibernation ---

#[tokio::test]
async fn test_hibernate_and_thaw() {
    let mut system = ActorSystem::new("test-hibernate");
    let actor = EchoActor::new("sleepy");
    let actor_id = actor.id.clone();
    let _actor_ref = system.spawn_default(Box::new(actor));

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    // 休眠
    system.hibernate(&actor_id).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let status = system.status(&actor_id);
    assert!(
        status == Some(ActorStatus::Hibernated) || status == Some(ActorStatus::Stopped),
        "expected Hibernated or Stopped, got {:?}",
        status
    );

    // 唤醒（提供新的 Actor 实例）
    let new_actor = EchoActor::new("sleepy");
    let new_ref = system
        .thaw(&actor_id, Box::new(new_actor), 256)
        .await
        .unwrap();

    // 唤醒后应该能正常发消息
    new_ref.tell("after thaw".to_string()).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    system.shutdown().await;
}

#[tokio::test]
async fn test_hibernate_pending_messages() {
    let mut system = ActorSystem::new("test-pending");
    let actor = EchoActor::new("buffered");
    let actor_id = actor.id.clone();
    let _actor_ref = system.spawn_default(Box::new(actor));

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    // 休眠
    system.hibernate(&actor_id).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 向休眠的 Actor 暂存消息
    let e1 = Envelope::new(Box::new("pending1".to_string()), None);
    let e2 = Envelope::new(Box::new("pending2".to_string()), None);
    assert!(system.buffer_message(&actor_id, e1));
    assert!(system.buffer_message(&actor_id, e2));
    assert_eq!(system.pending_message_count(&actor_id), 2);

    // 唤醒，暂存消息应被重放
    let new_actor = EchoActor::new("buffered");
    let _new_ref = system
        .thaw(&actor_id, Box::new(new_actor), 256)
        .await
        .unwrap();

    // 重放后 pending 应清空
    assert_eq!(system.pending_message_count(&actor_id), 0);

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    system.shutdown().await;
}
