use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{info, warn};
use uuid::Uuid;

use super::stream::{StreamConsumer, StreamEvent, StreamProducer};

/// 拦截结果
#[derive(Debug)]
pub enum InterceptResult {
    /// 放行，继续转发给消费者
    Pass,
    /// 拦截，终止流并通知双方
    Block { reason: String },
}

/// StreamInterceptor trait — 流拦截器
///
/// 在 Producer 和 Consumer 之间检查每个 chunk。
/// 检测到问题时可以终止流（Cancel 给 Producer，Error 给 Consumer）。
#[async_trait]
pub trait StreamInterceptor<T: Send + 'static>: Send + 'static {
    /// 检查一个 chunk，返回 Pass 或 Block
    async fn inspect(&mut self, chunk: &T) -> InterceptResult;
}

/// 创建带拦截器的流
///
/// 返回 (producer, consumer)，中间由 interceptor 在独立 tokio task 中转发。
/// Producer 写入数据 → interceptor 检查 → 通过则转发给 Consumer。
pub fn create_intercepted_stream<T: Send + 'static>(
    buffer_size: usize,
    interceptor: Box<dyn StreamInterceptor<T>>,
) -> (StreamProducer<T>, StreamConsumer<T>) {
    let id = Uuid::new_v4();

    // Producer → interceptor 的 channel
    let (prod_tx, mut prod_rx) = mpsc::channel::<StreamEvent<T>>(buffer_size);
    let (prod_cancel_tx, prod_cancel_rx) = mpsc::channel::<()>(1);

    // interceptor → Consumer 的 channel
    let (cons_tx, cons_rx) = mpsc::channel::<StreamEvent<T>>(buffer_size);
    let (cons_cancel_tx, mut cons_cancel_rx) = mpsc::channel::<()>(1);

    let producer = StreamProducer::from_parts(id, prod_tx, prod_cancel_rx);
    let consumer = StreamConsumer::from_parts(id, cons_rx, cons_cancel_tx);

    // 启动拦截转发 task
    tokio::spawn(async move {
        let mut interceptor = interceptor;

        loop {
            tokio::select! {
                event = prod_rx.recv() => {
                    match event {
                        Some(StreamEvent::Data(data)) => {
                            match interceptor.inspect(&data).await {
                                InterceptResult::Pass => {
                                    if cons_tx.send(StreamEvent::Data(data)).await.is_err() {
                                        // Consumer 已 drop
                                        break;
                                    }
                                }
                                InterceptResult::Block { reason } => {
                                    warn!(stream_id = %id, reason = %reason, "stream intercepted");
                                    // 通知 Consumer 流被拦截
                                    let _ = cons_tx.send(StreamEvent::Error(
                                        format!("intercepted: {}", reason)
                                    )).await;
                                    // 通知 Producer 取消
                                    let _ = prod_cancel_tx.send(()).await;
                                    break;
                                }
                            }
                        }
                        Some(StreamEvent::End) => {
                            let _ = cons_tx.send(StreamEvent::End).await;
                            break;
                        }
                        Some(StreamEvent::Error(e)) => {
                            let _ = cons_tx.send(StreamEvent::Error(e)).await;
                            break;
                        }
                        Some(StreamEvent::Cancel) => {
                            let _ = cons_tx.send(StreamEvent::Cancel).await;
                            break;
                        }
                        None => {
                            // Producer channel 关闭
                            break;
                        }
                    }
                }
                _ = cons_cancel_rx.recv() => {
                    // Consumer 请求取消，转发给 Producer
                    let _ = prod_cancel_tx.send(()).await;
                    break;
                }
            }
        }

        info!(stream_id = %id, "interceptor task finished");
    });

    (producer, consumer)
}

/// KeywordInterceptor — 内置的关键词拦截器示例
///
/// 检查 String 类型的 chunk 是否包含指定关键词。
pub struct KeywordInterceptor {
    keywords: Vec<String>,
}

impl KeywordInterceptor {
    pub fn new(keywords: Vec<String>) -> Self {
        Self { keywords }
    }
}

#[async_trait]
impl StreamInterceptor<String> for KeywordInterceptor {
    async fn inspect(&mut self, chunk: &String) -> InterceptResult {
        let lower = chunk.to_lowercase();
        for kw in &self.keywords {
            if lower.contains(&kw.to_lowercase()) {
                return InterceptResult::Block {
                    reason: format!("blocked keyword: {}", kw),
                };
            }
        }
        InterceptResult::Pass
    }
}
