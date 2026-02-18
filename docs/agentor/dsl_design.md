# Agentor DSL: Declarative Agent Orchestration

The **Agentor DSL** is a YAML-based configuration language for defining complex Agent Actor systems. It allows developers to specify how agents are spawned, how they communicate via streams, and how they are monitored without writing low-level orchestration code.

## 1. Top-Level Structure

```yaml
version: "1.0"
name: "system-code-reviewer"

# Define reusable agent templates
actors:
  - id: coder
    model: gpt-4o
    role: "Expert Rust Developer"
    tools: [bash, fetch_url]

  - id: critic
    model: claude-3-5-sonnet
    role: "Security & Performance Auditor"

# Define how data flows between actors
topology:
  - from: coder
    to: critic
    type: stream
    trigger: on_complete # or on_chunk for real-time preview

# Define supervision and resource limits
policies:
  budget:
    max_tokens: 100000
    max_cost_usd: 5.0
  interruption:
    human_in_the_loop: [critic] # Critic output requires human approval
```

## 2. Actor Definitions

Actors can be defined with deep customization:

```yaml
actors:
  - id: researcher
    template: base_agent
    params:
      temperature: 0.2
    memory:
      type: vector
      collection: "github_issues"
    lifecycle:
      persist: true
      hibernation: true
```

## 3. Topology & Streaming Paths

The topology defines the "circulatory system" of the agent swarm.

### Linear Pipeline
```yaml
topology:
  - path: [researcher, writer, editor]
```

### Fan-out / Parallel Processing
```yaml
topology:
  - from: orchestrator
    to: [agent_v4, agent_v5]
    mode: parallel
```

### Stream Interception (Supervision)
```yaml
topology:
  - from: coder
    to: reviewer
    interceptors: [security_scanner] # security_scanner scans the chunk stream
```

## 4. Operational Policies

### Budget & Limits
```yaml
policies:
  limits:
    max_recursion_depth: 5
    max_parallel_actors: 10
  cost:
    provider: "openai"
    limit_usd: 10.0
    on_limit: "suspend" # or "notify"
```

### Human-in-the-Loop (HITL)
```yaml
policies:
  approval:
    - actor: shell_executor
      conditions:
        - match: "rm -rf *"
          action: "require_approval"
        - match: "git commit"
          action: "auto_approve"
```

## 5. Benefits of DSL Approach

1.  **Observability**: The YAML file serves as a live map of the system.
2.  **Portability**: The same topology can be exported or version-controlled.
3.  **Stability**: Changes to the architecture don't require rewriting Rust connection logic; just update the manifest.
4.  **Static Validation**: The framework can validate the topology (e.g., checking for cycles or unreachable agents) at startup.
