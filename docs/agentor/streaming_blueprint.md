# Technical Blueprint: Streaming Native Implementation (Rust)

This document provides a low-level view of how AAS implements streaming primitives using Rust and Tokio.

## 1. The Core Types

```rust
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StreamId(Uuid);

#[derive(Debug)]
pub enum Signal {
    Interrupt,
    Pause,
    Resume,
}

#[derive(Debug)]
pub enum AgentChunk {
    Token(String),
    ToolCall(serde_json::Value),
    Thought(String), // Inner monologue
}

#[derive(Debug)]
pub enum AgentMessage {
    StreamStart(StreamId),
    Chunk(AgentChunk),
    Control(Signal),
    StreamEnd { tokens: u32, cost: f64 },
}
```

## 2. Actor Communication Loop

Every Actor holds a `mpsc::Sender` for its mailbox. For streaming, we use a dedicated sub-channel per stream.

```rust
pub struct AgentActor {
    mailbox: mpsc::Receiver<Envelope>,
    // Other state...
}

impl AgentActor {
    async fn run(mut self) {
        while let Some(env) = self.mailbox.recv().await {
            match env.msg {
                AgentMessage::StreamStart(id) => {
                    self.process_stream(id, env.stream_rx).await;
                }
                // ...
            }
        }
    }

    async fn process_stream(&mut self, id: StreamId, mut rx: mpsc::Receiver<AgentChunk>) {
        while let Some(chunk) = rx.recv().await {
            // Real-time processing
            if self.detect_hallucination(&chunk) {
                // Send interrupt signal back to producer
                self.send_interrupt(id).await;
                break;
            }
        }
    }
}
```

## 3. Supervision & Proxy Pattern

A Supervisor can "wrap" an Agent Actor's stream to enforce safety.

```rust
pub struct SafeStreamProxy {
    inner_rx: mpsc::Receiver<AgentChunk>,
    output_tx: mpsc::Sender<AgentMessage>,
}

impl SafeStreamProxy {
    pub async fn pipe(mut self) {
        while let Some(chunk) = self.inner_rx.recv().await {
            if is_malicious(&chunk) {
                // DROP chunk and send INTERRUPT to producer
                self.emit_security_alert().await;
                return;
            }
            // Otherwise, forward to the consumer
            self.output_tx.send(AgentMessage::Chunk(chunk)).await.ok();
        }
    }
}
```

## 4. Semantic Backpressure Implementation

If the consumer actor lags, the producer's channel buffer fills.
- **Classic**: The producer blocks.
- **AAS Semantic**: The producer detects the lag and automatically switches its logic to `SummarizeMode`, emitting fewer but higher-density chunks.

```rust
if tx.capacity() < LOW_THRESHOLD {
    producer.switch_to_summary_mode().await;
}
```

## 5. Persistence Strategy

To optimize database I/O:
1. **In-Flight**: Chunks are stored in a transient Redis/In-memory buffer for UI subscribers.
2. **Finalized**: Once `StreamEnd` is received, the entire flattened content is written to long-term storage (PostgreSQL/S3).
3. **Trace Alignment**: Every chunk carries a `TraceId`, ensuring `xtrace` can visualize the generation process.
