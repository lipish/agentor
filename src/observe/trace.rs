use std::collections::VecDeque;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;
use xtrace_client::{
    BatchIngestRequest, Client as XtraceClient, MetricPoint, ObservationIngest, TraceIngest,
};

use crate::actor::message::ActorId;

/// TraceEvent — 单条追踪事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub actor_id: ActorId,
    pub event_type: TraceEventType,
    pub detail: String,
    pub metadata: Option<serde_json::Value>,
}

/// 追踪事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraceEventType {
    MessageReceived,
    MessageSent,
    StateChanged,
    ToolCalled,
    LlmRequest,
    LlmResponse,
    Error,
    CheckpointSaved,
    ActorStarted,
    ActorStopped,
    Custom(String),
}

impl TraceEventType {
    fn as_observation_type(&self) -> &str {
        match self {
            Self::LlmRequest | Self::LlmResponse => "GENERATION",
            Self::ToolCalled => "SPAN",
            _ => "EVENT",
        }
    }
}

/// TraceCollector — 全链路追踪收集器
///
/// 收集所有 Actor 的事件，支持本地回放和查询。
/// 可选接入 xtrace (https://xtrace.sh) 进行远程链路追踪和指标上报。
#[derive(Clone)]
pub struct TraceCollector {
    inner: Arc<RwLock<TraceBuffer>>,
    xtrace: Option<Arc<XtraceClient>>,
}

struct TraceBuffer {
    events: VecDeque<TraceEvent>,
    capacity: usize,
}

impl TraceCollector {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(TraceBuffer {
                events: VecDeque::with_capacity(capacity),
                capacity,
            })),
            xtrace: None,
        }
    }

    /// 创建带 xtrace 远程上报的 TraceCollector
    pub fn with_xtrace(capacity: usize, endpoint: &str, token: &str) -> anyhow::Result<Self> {
        let client = XtraceClient::new(endpoint, token)?;
        Ok(Self {
            inner: Arc::new(RwLock::new(TraceBuffer {
                events: VecDeque::with_capacity(capacity),
                capacity,
            })),
            xtrace: Some(Arc::new(client)),
        })
    }

    /// 设置 xtrace client（运行时动态配置）
    pub fn set_xtrace_client(&mut self, client: XtraceClient) {
        self.xtrace = Some(Arc::new(client));
    }

    /// 获取 xtrace client 引用
    pub fn xtrace_client(&self) -> Option<&XtraceClient> {
        self.xtrace.as_deref()
    }

    /// 记录事件（本地 + 远程上报）
    pub fn record(&self, event: TraceEvent) {
        // 本地记录
        {
            let mut buf = self.inner.write();
            if buf.events.len() >= buf.capacity {
                buf.events.pop_front();
            }
            buf.events.push_back(event.clone());
        }

        // 异步上报到 xtrace
        if let Some(client) = &self.xtrace {
            let client = Arc::clone(client);
            tokio::spawn(async move {
                let req = BatchIngestRequest {
                    trace: Some(TraceIngest {
                        id: event.trace_id,
                        timestamp: Some(event.timestamp),
                        name: Some(format!("actor:{}", event.actor_id.name)),
                        input: None,
                        output: None,
                        session_id: Some(event.actor_id.id.to_string()),
                        release: None,
                        version: None,
                        user_id: None,
                        metadata: None,
                        tags: vec![],
                        public: None,
                        environment: None,
                        external_id: None,
                        bookmarked: None,
                        latency: None,
                        total_cost: None,
                        project_id: None,
                    }),
                    observations: vec![ObservationIngest {
                        id: event.id,
                        trace_id: event.trace_id,
                        r#type: Some(event.event_type.as_observation_type().into()),
                        name: Some(event.detail.clone()),
                        start_time: Some(event.timestamp),
                        end_time: None,
                        completion_start_time: None,
                        model: None,
                        model_parameters: None,
                        input: None,
                        output: None,
                        usage: None,
                        level: None,
                        status_message: None,
                        parent_observation_id: None,
                        prompt_id: None,
                        prompt_name: None,
                        prompt_version: None,
                        model_id: None,
                        input_price: None,
                        output_price: None,
                        total_price: None,
                        calculated_input_cost: None,
                        calculated_output_cost: None,
                        calculated_total_cost: None,
                        latency: None,
                        time_to_first_token: None,
                        completion_tokens: None,
                        prompt_tokens: None,
                        total_tokens: None,
                        unit: None,
                        metadata: event.metadata.clone(),
                        environment: None,
                        project_id: None,
                    }],
                };
                if let Err(e) = client.ingest_batch(&req).await {
                    warn!(error = %e, "failed to ingest trace to xtrace");
                }
            });
        }
    }

    /// 快捷方法：记录一条简单事件
    pub fn log(
        &self,
        trace_id: Uuid,
        actor_id: &ActorId,
        event_type: TraceEventType,
        detail: impl Into<String>,
    ) {
        self.record(TraceEvent {
            id: Uuid::new_v4(),
            trace_id,
            timestamp: Utc::now(),
            actor_id: actor_id.clone(),
            event_type,
            detail: detail.into(),
            metadata: None,
        });
    }

    /// 上报 token 使用量指标到 xtrace
    pub async fn report_token_usage(
        &self,
        actor_id: &ActorId,
        model: &str,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) {
        if let Some(client) = &self.xtrace {
            let labels = std::collections::HashMap::from([
                ("agent_role".into(), actor_id.name.clone()),
                ("model_name".into(), model.to_string()),
            ]);
            let points = vec![
                MetricPoint {
                    name: "prompt_tokens".into(),
                    labels: labels.clone(),
                    value: prompt_tokens as f64,
                    timestamp: Utc::now(),
                },
                MetricPoint {
                    name: "completion_tokens".into(),
                    labels: labels.clone(),
                    value: completion_tokens as f64,
                    timestamp: Utc::now(),
                },
                MetricPoint {
                    name: "total_tokens".into(),
                    labels,
                    value: (prompt_tokens + completion_tokens) as f64,
                    timestamp: Utc::now(),
                },
            ];
            if let Err(e) = client.push_metrics(&points).await {
                warn!(error = %e, "failed to push token metrics to xtrace");
            }
        }
    }

    /// 按 trace_id 查询所有事件（用于回放）
    pub fn query_by_trace(&self, trace_id: &Uuid) -> Vec<TraceEvent> {
        let buf = self.inner.read();
        buf.events
            .iter()
            .filter(|e| &e.trace_id == trace_id)
            .cloned()
            .collect()
    }

    /// 按 actor_id 查询最近 N 条事件
    pub fn query_by_actor(&self, actor_id: &ActorId, limit: usize) -> Vec<TraceEvent> {
        let buf = self.inner.read();
        buf.events
            .iter()
            .rev()
            .filter(|e| &e.actor_id == actor_id)
            .take(limit)
            .cloned()
            .collect()
    }

    /// 获取所有事件（用于导出）
    pub fn all_events(&self) -> Vec<TraceEvent> {
        let buf = self.inner.read();
        buf.events.iter().cloned().collect()
    }

    /// 事件总数
    pub fn len(&self) -> usize {
        self.inner.read().events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.read().events.is_empty()
    }

    /// 清空
    pub fn clear(&self) {
        self.inner.write().events.clear();
    }
}

impl Default for TraceCollector {
    fn default() -> Self {
        Self::new(10000)
    }
}
