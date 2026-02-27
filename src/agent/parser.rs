
use serde_json::Value;

/// 工具调用请求
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub name: String,
    pub args: Value,
}

/// LLM 响应解析器
pub trait OutputParser: Send + Sync {
    /// 从 LLM 响应中解析出工具调用
    /// 如果返回 None，表示没有工具调用，是普通文本回复
    fn parse(&self, content: &str) -> Option<ToolCall>;
}

/// 简单的 JSON 格式解析器
/// 假设 LLM 会输出类似 `{"tool": "name", "args": {...}}` 的 JSON
pub struct JsonParser;

impl OutputParser for JsonParser {
    fn parse(&self, content: &str) -> Option<ToolCall> {
        // 尝试寻找 JSON 块
        // 这里做一个简化的实现：尝试直接解析整个 content，或者寻找 ```json ... ```
        
        let json_str = if let Some(start) = content.find("```json") {
            // 从 ```json 之后开始找 ```
            if let Some(end) = content[start+7..].find("```") {
                 let s = &content[start+7..start+7+end];
                 s.trim()
            } else {
                content
            }
        } else {
            content
        };

        if let Ok(val) = serde_json::from_str::<Value>(json_str) {
            if let Some(obj) = val.as_object() {
                if let (Some(name), Some(args)) = (obj.get("tool"), obj.get("args")) {
                    if let Some(name_str) = name.as_str() {
                        return Some(ToolCall {
                            name: name_str.to_string(),
                            args: args.clone(),
                        });
                    }
                }
            }
        }
        
        None
    }
}
