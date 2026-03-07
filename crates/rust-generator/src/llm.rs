use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Trait for LLM providers. Enables swapping between Claude, OpenAI, etc.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Generate a response given a system prompt and user prompt.
    async fn generate(&self, system: &str, user: &str) -> Result<String>;

    /// Return the provider name (for logging).
    fn name(&self) -> &str;
}

/// Claude API provider via Anthropic Messages API.
pub struct ClaudeProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl ClaudeProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY environment variable not set"))?;
        Ok(Self::new(api_key, "claude-sonnet-4-20250514".to_string()))
    }
}

#[derive(Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[async_trait]
impl LlmProvider for ClaudeProvider {
    async fn generate(&self, system: &str, user: &str) -> Result<String> {
        let request = MessagesRequest {
            model: self.model.clone(),
            max_tokens: 8192,
            system: system.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: user.to_string(),
            }],
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            anyhow::bail!("Claude API error ({}): {}", status, body);
        }

        let resp: MessagesResponse = response.json().await?;
        let text = resp
            .content
            .into_iter()
            .filter_map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");

        Ok(text)
    }

    fn name(&self) -> &str {
        "Claude"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_provider_requires_api_key() {
        if std::env::var("ANTHROPIC_API_KEY").is_err() {
            assert!(ClaudeProvider::from_env().is_err());
        }
    }
}
