use tokio::sync::mpsc;
use uuid::Uuid;

/// StreamHandle — 双向流通信句柄
///
/// 用于 Actor 之间的流式消息传递，支持：
/// - LLM 流式输出（逐 token 发送）
/// - 监督者实时消费子 Agent 的输出流并中途中断
pub struct StreamHandle<T> {
    pub id: Uuid,
    pub sender: mpsc::Sender<StreamEvent<T>>,
    pub receiver: Option<mpsc::Receiver<StreamEvent<T>>>,
}

/// 流事件
#[derive(Debug)]
pub enum StreamEvent<T> {
    /// 数据片段
    Data(T),
    /// 流结束
    End,
    /// 流出错
    Error(String),
    /// 中断请求（由消费者发出）
    Cancel,
}

/// 创建一对流句柄（生产者 + 消费者）
pub fn create_stream<T: Send + 'static>(
    buffer_size: usize,
) -> (StreamProducer<T>, StreamConsumer<T>) {
    let id = Uuid::new_v4();
    let (tx, rx) = mpsc::channel(buffer_size);
    let (cancel_tx, cancel_rx) = mpsc::channel(1);

    (
        StreamProducer {
            id,
            sender: tx,
            cancel_receiver: cancel_rx,
        },
        StreamConsumer {
            id,
            receiver: rx,
            cancel_sender: cancel_tx,
        },
    )
}

/// StreamProducer — 流的生产端（例如 LLM Provider Actor）
pub struct StreamProducer<T> {
    pub id: Uuid,
    sender: mpsc::Sender<StreamEvent<T>>,
    cancel_receiver: mpsc::Receiver<()>,
}

impl<T: Send + 'static> StreamProducer<T> {
    /// 从组件构造（供 interceptor 内部使用）
    pub(crate) fn from_parts(
        id: Uuid,
        sender: mpsc::Sender<StreamEvent<T>>,
        cancel_receiver: mpsc::Receiver<()>,
    ) -> Self {
        Self {
            id,
            sender,
            cancel_receiver,
        }
    }

    /// 发送数据片段
    pub async fn send(&self, data: T) -> Result<(), StreamError> {
        self.sender
            .send(StreamEvent::Data(data))
            .await
            .map_err(|_| StreamError::ConsumerDropped)
    }

    /// 标记流结束
    pub async fn finish(self) -> Result<(), StreamError> {
        self.sender
            .send(StreamEvent::End)
            .await
            .map_err(|_| StreamError::ConsumerDropped)
    }

    /// 发送错误
    pub async fn error(self, msg: String) -> Result<(), StreamError> {
        self.sender
            .send(StreamEvent::Error(msg))
            .await
            .map_err(|_| StreamError::ConsumerDropped)
    }

    /// 检查是否收到取消请求
    pub fn is_cancelled(&mut self) -> bool {
        self.cancel_receiver.try_recv().is_ok()
    }
}

/// StreamConsumer — 流的消费端（例如 Agent Actor）
pub struct StreamConsumer<T> {
    pub id: Uuid,
    receiver: mpsc::Receiver<StreamEvent<T>>,
    cancel_sender: mpsc::Sender<()>,
}

impl<T: Send + 'static> StreamConsumer<T> {
    /// 从组件构造（供 interceptor 内部使用）
    pub(crate) fn from_parts(
        id: Uuid,
        receiver: mpsc::Receiver<StreamEvent<T>>,
        cancel_sender: mpsc::Sender<()>,
    ) -> Self {
        Self {
            id,
            receiver,
            cancel_sender,
        }
    }

    /// 接收下一个流事件
    pub async fn next(&mut self) -> Option<StreamEvent<T>> {
        self.receiver.recv().await
    }

    /// 发送取消请求
    pub async fn cancel(&self) -> Result<(), StreamError> {
        self.cancel_sender
            .send(())
            .await
            .map_err(|_| StreamError::ProducerDropped)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("stream consumer has been dropped")]
    ConsumerDropped,
    #[error("stream producer has been dropped")]
    ProducerDropped,
}
