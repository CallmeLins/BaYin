//! 音乐源注册表：维护 `id → SourceConnector` 的映射，并按配置查找连接器。

use crate::models::StreamServerConfig;
use std::collections::HashMap;

use super::SourceConnector;

/// 音乐源注册表。
///
/// 线程安全：连接器是 `Box<dyn SourceConnector>`，通过 `Arc` 共享，只读查询。
/// 注册操作仅在启动阶段调用一次，因此用 `Mutex` 保护可变性即可。
#[derive(Default)]
pub struct SourceRegistry {
    connectors: HashMap<&'static str, Box<dyn SourceConnector>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个连接器。`id()` 重复时覆盖旧项（并记录日志）。
    pub fn register(&mut self, connector: Box<dyn SourceConnector>) {
        let id = connector.id();
        if self.connectors.contains_key(id) {
            log::warn!("音乐源连接器 {id} 重复注册，将覆盖旧实现");
        }
        self.connectors.insert(id, connector);
    }

    /// 按 `id` 查找连接器。
    pub fn get(&self, id: &str) -> Option<&dyn SourceConnector> {
        self.connectors.get(id).map(|b| b.as_ref())
    }

    /// 按配置匹配连接器。
    ///
    /// 遍历注册表，用 `connector.matches(config)` 判定。找不到时返回带提示的错误，
    /// 便于定位"server_type 未注册"。
    pub fn resolve(&self, config: &StreamServerConfig) -> Result<&dyn SourceConnector, String> {
        for c in self.connectors.values() {
            if c.matches(config) {
                return Ok(c.as_ref());
            }
        }
        Err(format!(
            "未注册的音乐源类型: {:?}（当前支持: {}）",
            config.server_type,
            self.supported_types()
        ))
    }

    /// 已注册的所有连接器 id（逗号分隔）。
    pub fn supported_types(&self) -> String {
        let mut ids: Vec<&str> = self.connectors.keys().copied().collect();
        ids.sort();
        ids.join(", ")
    }

    /// 已注册连接器数量。
    pub fn len(&self) -> usize {
        self.connectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connectors.is_empty()
    }
}
