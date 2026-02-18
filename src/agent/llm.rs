use futures_util::StreamExt;
use llm_connector::types::{ChatRequest, Message, Role};
use llm_connector::LlmClient;
use tracing::{debug, info};

/// LlmConnector — 封装 llm-connector，为 AgentActor 提供 LLM 调用能力
///
/// 支持 12+ Provider（OpenAI, Anthropic, Google, DeepSeek, Ollama 等），
/// 统一的同步/流式接口。
pub struct LlmConnector {
    client: LlmClient,
    model: String,
}

/// LLM 调用结果
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

/// 流式回调：每收到一个 chunk 调用一次
pub type StreamCallback = Box<dyn Fn(&str) + Send + Sync>;

impl LlmConnector {
    /// 从 LlmClient + model 名创建
    pub fn new(client: LlmClient, model: impl Into<String>) -> Self {
        Self {
            client,
            model: model.into(),
        }
    }

    /// 快捷构造：OpenAI
    pub fn openai(api_key: &str, model: &str) -> anyhow::Result<Self> {
        let client = LlmClient::openai(api_key)?;
        Ok(Self::new(client, model))
    }

    /// 快捷构造：Anthropic
    pub fn anthropic(api_key: &str, model: &str) -> anyhow::Result<Self> {
        let client = LlmClient::anthropic(api_key)?;
        Ok(Self::new(client, model))
    }

    /// 快捷构造：DeepSeek
    pub fn deepseek(api_key: &str, model: &str) -> anyhow::Result<Self> {
        let client = LlmClient::deepseek(api_key)?;
        Ok(Self::new(client, model))
    }

    /// 快捷构造：Ollama（本地，无需 API key）
    pub fn ollama(model: &str) -> anyhow::Result<Self> {
        let client = LlmClient::ollama()?;
        Ok(Self::new(client, model))
    }

    /// 快捷构造：通过 builder 自定义
    pub fn builder() -> llm_connector::builder::LlmClientBuilder {
        LlmClient::builder()
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// 非流式调用
    pub async fn chat(&self, messages: &[LlmMessage]) -> anyhow::Result<LlmResponse> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.iter().map(|m| m.to_llm_message()).collect(),
            ..Default::default()
        };

        info!(model = %self.model, msg_count = messages.len(), "calling LLM");
        let response = self.client.chat(&request).await?;

        Ok(LlmResponse {
            content: response.content.clone(),
            prompt_tokens: response.usage.as_ref().map(|u| u.prompt_tokens as u64),
            completion_tokens: response.usage.as_ref().map(|u| u.completion_tokens as u64),
            total_tokens: response.usage.as_ref().map(|u| u.total_tokens as u64),
        })
    }

    /// 流式调用，返回完整内容 + token 统计
    pub async fn chat_stream(
        &self,
        messages: &[LlmMessage],
        on_chunk: Option<StreamCallback>,
    ) -> anyhow::Result<LlmResponse> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.iter().map(|m| m.to_llm_message()).collect(),
            stream: Some(true),
            ..Default::default()
        };

        info!(model = %self.model, msg_count = messages.len(), "calling LLM (stream)");
        let mut stream = self.client.chat_stream(&request).await?;
        let mut full_content = String::new();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(resp) => {
                    if let Some(content) = resp.get_content() {
                        full_content.push_str(content);
                        if let Some(cb) = &on_chunk {
                            cb(content);
                        }
                        debug!(chunk_len = content.len(), "stream chunk received");
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("stream error: {}", e));
                }
            }
        }

        Ok(LlmResponse {
            content: full_content,
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
        })
    }
}

/// 简化的消息类型，用于 AgentActor 与 LlmConnector 之间的接口
#[derive(Debug, Clone)]
pub struct LlmMessage {
    pub role: LlmRole,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum LlmRole {
    System,
    User,
    Assistant,
    Tool,
}

impl LlmMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::Assistant,
            content: content.into(),
        }
    }

    fn to_llm_message(&self) -> Message {
        let role = match self.role {
            LlmRole::System => Role::System,
            LlmRole::User => Role::User,
            LlmRole::Assistant => Role::Assistant,
            LlmRole::Tool => Role::User,
        };
        Message::text(role, &self.content)
    }
}
