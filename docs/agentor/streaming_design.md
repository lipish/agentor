# Streaming Native Primitives for AAS

Traditional Actor systems treat messages as discrete, atomic events. In an AI context, this leads to a "Wait-and-React" bottleneck. **Streaming Native** redesigns the communication layer to treat partial content as a first-class citizen.

## 1. The Streamable Message Protocol

Messages in AAS are no longer just `T`, but `Streamable<T>`.

### Data Structure
```rust
enum MessageBlock {
    Header(Metadata),    // Model, StreamId, Priority
    Chunk(String),       // Partial token/content
    Control(Signal),     // Interrupt, Amend, Pause
    Footer(UsageStats),  // Tokens used, final summary
}
```

## 2. Communication Modes

### 2.1 Pass-through Streaming
Agent A starts generating. The framework emits `Chunk` messages immediately to Agent B's mailbox. Agent B can start "pre-processing" (e.g., scanning for sensitive info) before the footer arrives.

### 2.2 Supervised Interception (Middle-man)
A supervisor Actor can sit between Agent A and Agent B, inspecting the stream in real-time.
- **Example**: If Agent A starts generating code that violates safety rules, the Supervisor sends a `Control::Interrupt` to Agent A and a `MessageBlock::Error` to Agent B, truncating the stream instantly.

## 3. The "Interrupt" Primitive

One of the hardest problems in Agent UX is stopping an LLM that is "hallucinating."
- **In AAS**: `ActorRef.send(Control::Interrupt)` triggers a framework-level cancellation.
- The framework immediately drops the remaining output from the LLM provider and marks the current `StreamId` as `Cancelled`.

## 4. Semantic Backpressure

Standard backpressure (stop sending if the buffer is full) is too dumb for AI.
- **Smart Backpressure**: If Agent B (the consumer) is slow, the framework can request Agent A (the producer) to **summarize** instead of continuing the raw stream, or increase the chunk size to reduce overhead.

## 5. Use Case: Real-time UI
A "User Interface Actor" can subscribe to the stream of an "Assistant Actor."
- The user sees tokens appearing in real-time.
- If the user types "No, not like that!", the UI Actor sends an `Interrupt` to the Assistant.
- Because it's "Streaming Native," the Assistant stops *immediately* (saving tokens) and processes the new instruction.

## 6. Technical Implementation (Rust/Tokio)
- **Transport**: Utilizes `tokio::sync::mpsc::unbounded_channel` for low-latency chunk delivery.
- **Persistence**: Only the `Footer` (final result) is saved to the long-term DB by default, while chunks are kept in a short-term ring buffer for real-time subscribers.
