use anyhow::{Result, anyhow};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ModelProviderKind {
    OpenAiCompatible,
    AnthropicCompatible,
    GeminiCompatible,
    Ollama,
    Mock,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderProfile {
    pub id: String,
    pub display_name: String,
    pub kind: ModelProviderKind,
    pub base_url: Option<String>,
    pub model: String,
    pub api_key: Option<String>,
    pub enabled: bool,
}

impl ModelProviderProfile {
    pub fn mock() -> Self {
        Self {
            id: "mock-local".to_string(),
            display_name: "本地演示模型".to_string(),
            kind: ModelProviderKind::Mock,
            base_url: None,
            model: "mock-story-v1".to_string(),
            api_key: None,
            enabled: true,
        }
    }

    pub fn with_new_id(mut self) -> Self {
        self.id = Uuid::new_v4().to_string();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTestResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelRequest {
    pub system: String,
    pub prompt: String,
    pub max_output_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelResponse {
    pub text: String,
}

#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn complete(&self, request: ModelRequest) -> Result<ModelResponse>;
}

#[derive(Debug, Default)]
pub struct MockStoryModel;

#[async_trait]
impl ModelClient for MockStoryModel {
    async fn complete(&self, request: ModelRequest) -> Result<ModelResponse> {
        let mut lines = request
            .prompt
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>();
        lines.reverse();
        let seed = lines
            .iter()
            .find(|line| line.contains("玩家动作") || line.contains("继续剧情"))
            .copied()
            .unwrap_or("玩家选择继续向前。");
        let text = format!(
            "你稳住呼吸，顺着原文里留下的线索往前试探。\n\n{seed}\n\n故事没有立刻偏离主线，但你刚才的选择已经在世界线里留下痕迹。远处有人提到一个熟悉的名字，下一步最好先确认对方的目的，再决定是说服、试探，还是暂时退开。"
        );
        Ok(ModelResponse {
            text: text.chars().take(request.max_output_chars).collect(),
        })
    }
}

pub fn validate_profile(profile: &ModelProviderProfile) -> ProviderTestResult {
    if !profile.enabled {
        return ProviderTestResult {
            ok: false,
            message: "这个模型配置还没有启用".to_string(),
        };
    }
    if profile.model.trim().is_empty() {
        return ProviderTestResult {
            ok: false,
            message: "请填写模型名称".to_string(),
        };
    }
    match profile.kind {
        ModelProviderKind::Mock => ProviderTestResult {
            ok: true,
            message: "本地演示模型可用，不会上传任何原文".to_string(),
        },
        ModelProviderKind::Ollama => {
            if profile.base_url.as_deref().unwrap_or("").trim().is_empty() {
                ProviderTestResult {
                    ok: false,
                    message: "Ollama 需要填写本地地址，例如 http://localhost:11434".to_string(),
                }
            } else {
                ProviderTestResult {
                    ok: true,
                    message: "配置格式有效。V1 会只发送当前回合检索片段".to_string(),
                }
            }
        }
        ModelProviderKind::OpenAiCompatible
        | ModelProviderKind::AnthropicCompatible
        | ModelProviderKind::GeminiCompatible => {
            let has_key = profile
                .api_key
                .as_deref()
                .map(|key| !key.trim().is_empty())
                .unwrap_or(false);
            let has_base = profile
                .base_url
                .as_deref()
                .map(|url| !url.trim().is_empty())
                .unwrap_or(false);
            if has_key && has_base {
                ProviderTestResult {
                    ok: true,
                    message: "配置格式有效。联网连通性测试会在真实模型适配器接入后启用".to_string(),
                }
            } else {
                ProviderTestResult {
                    ok: false,
                    message: "请填写 API Key 和 Base URL".to_string(),
                }
            }
        }
    }
}

pub fn profile_from_json(json: &str) -> Result<ModelProviderProfile> {
    serde_json::from_str(json).map_err(|error| anyhow!("模型配置解析失败：{error}"))
}

pub fn profile_to_json(profile: &ModelProviderProfile) -> Result<String> {
    serde_json::to_string(profile).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_model_returns_scene_text() -> Result<()> {
        let model = MockStoryModel;
        let response = model
            .complete(ModelRequest {
                system: "你是穿书导演".to_string(),
                prompt: "玩家动作：提醒主角小心".to_string(),
                max_output_chars: 120,
            })
            .await?;
        assert!(response.text.contains("提醒主角小心"));
        Ok(())
    }

    #[test]
    fn validates_mock_profile() {
        let result = validate_profile(&ModelProviderProfile::mock());
        assert!(result.ok);
    }
}
