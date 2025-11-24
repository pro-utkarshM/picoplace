//! # picoplace-ai-engine
//!
//! This crate provides AI-powered placement and routing hints using LLM APIs.
//! It takes a `Schematic` object as input and generates intelligent suggestions
//! for component placement and net routing priorities.

use anyhow::{Context, Result};
use picoplace_engine::{placer_sa::PlacementHints, Point};
use picoplace_netlist::Schematic;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// AI hints for placement and routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIHints {
    /// Suggested component placements (component refdes -> position)
    pub placement_suggestions: PlacementHints,
    /// Routing priorities (net names in order of importance)
    pub routing_priorities: Vec<String>,
    /// Additional reasoning from the AI
    pub reasoning: String,
}

/// Configuration for the AI engine
#[derive(Debug, Clone)]
pub struct AIEngineConfig {
    /// API key for the LLM service
    pub api_key: String,
    /// Model to use (e.g., "gpt-4.1-mini", "gpt-4.1-nano", "gemini-2.5-flash")
    pub model: String,
    /// Base URL for the API (defaults to OpenAI-compatible endpoint)
    pub base_url: Option<String>,
    /// Maximum tokens for the response
    pub max_tokens: u32,
    /// Temperature for generation (0.0 to 1.0)
    pub temperature: f32,
}

impl Default for AIEngineConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: "gpt-4.1-mini".to_string(),
            base_url: None,
            max_tokens: 2000,
            temperature: 0.7,
        }
    }
}

/// Request structure for OpenAI API
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

/// Response structure from OpenAI API
#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: MessageResponse,
}

#[derive(Debug, Deserialize)]
struct MessageResponse {
    content: String,
}

/// AI Engine for generating placement and routing hints
pub struct AIEngine {
    config: AIEngineConfig,
    client: reqwest::blocking::Client,
}

impl AIEngine {
    /// Create a new AI engine with the given configuration
    pub fn new(config: AIEngineConfig) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { config, client })
    }

    /// Create a new AI engine with default configuration
    pub fn with_defaults() -> Result<Self> {
        Self::new(AIEngineConfig::default())
    }

    /// Generate AI hints for the given schematic
    pub fn generate_hints(&self, schematic: &Schematic) -> Result<AIHints> {
        let prompt = self.build_prompt(schematic);
        let response = self.call_llm(&prompt)?;
        self.parse_response(&response)
    }

    /// Build the prompt for the LLM
    fn build_prompt(&self, schematic: &Schematic) -> String {
        let mut prompt = String::new();

        prompt.push_str("You are an expert PCB layout designer. Given the following circuit schematic, ");
        prompt.push_str("provide intelligent suggestions for component placement and routing priorities.\n\n");

        // Add component information
        prompt.push_str("## Components:\n");
        for (inst_ref, instance) in &schematic.instances {
            if let Some(refdes) = &instance.reference_designator {
                prompt.push_str(&format!(
                    "- {} (type: {})\n",
                    refdes,
                    inst_ref.module.module_name
                ));
            }
        }

        // Add net information
        prompt.push_str("\n## Nets:\n");
        for (net_name, net) in &schematic.nets {
            prompt.push_str(&format!("- {} (connects {} pins)\n", net_name, net.ports.len()));
        }

        // Add instructions
        prompt.push_str("\n## Task:\n");
        prompt.push_str("Please provide:\n");
        prompt.push_str("1. Suggested X,Y coordinates (in mm) for each component on a 100mm x 100mm board\n");
        prompt.push_str("2. Routing priorities (list of net names in order of importance)\n");
        prompt.push_str("3. Brief reasoning for your suggestions\n\n");
        prompt.push_str("Respond in JSON format:\n");
        prompt.push_str("{\n");
        prompt.push_str("  \"placement_suggestions\": {\n");
        prompt.push_str("    \"R1\": {\"x\": 20.0, \"y\": 30.0},\n");
        prompt.push_str("    \"C1\": {\"x\": 50.0, \"y\": 30.0}\n");
        prompt.push_str("  },\n");
        prompt.push_str("  \"routing_priorities\": [\"VCC\", \"GND\", \"SIGNAL\"],\n");
        prompt.push_str("  \"reasoning\": \"Power components placed near edge, signal components grouped by function...\"\n");
        prompt.push_str("}\n");

        prompt
    }

    /// Call the LLM API
    fn call_llm(&self, prompt: &str) -> Result<String> {
        let api_url = self.config.base_url.clone()
            .unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".to_string());

        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
        };

        let response = self
            .client
            .post(&api_url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .context("Failed to send request to LLM API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            anyhow::bail!("LLM API request failed with status {}: {}", status, error_text);
        }

        let chat_response: ChatResponse = response
            .json()
            .context("Failed to parse LLM API response")?;

        chat_response
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .context("No response from LLM")
    }

    /// Parse the LLM response into AIHints
    fn parse_response(&self, response: &str) -> Result<AIHints> {
        // Try to extract JSON from the response (it might be wrapped in markdown code blocks)
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        // Parse the JSON response
        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .context("Failed to parse JSON response from LLM")?;

        // Extract placement suggestions
        let mut placement_suggestions = HashMap::new();
        if let Some(placements) = parsed.get("placement_suggestions").and_then(|v| v.as_object()) {
            for (refdes, pos) in placements {
                if let (Some(x), Some(y)) = (
                    pos.get("x").and_then(|v| v.as_f64()),
                    pos.get("y").and_then(|v| v.as_f64()),
                ) {
                    placement_suggestions.insert(refdes.clone(), Point { x, y });
                }
            }
        }

        // Extract routing priorities
        let routing_priorities = parsed
            .get("routing_priorities")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Extract reasoning
        let reasoning = parsed
            .get("reasoning")
            .and_then(|v| v.as_str())
            .unwrap_or("No reasoning provided")
            .to_string();

        Ok(AIHints {
            placement_suggestions,
            routing_priorities,
            reasoning,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response() {
        let engine = AIEngine::with_defaults().unwrap();
        let response = r#"
        {
            "placement_suggestions": {
                "R1": {"x": 20.0, "y": 30.0},
                "C1": {"x": 50.0, "y": 30.0}
            },
            "routing_priorities": ["VCC", "GND", "SIGNAL"],
            "reasoning": "Test reasoning"
        }
        "#;

        let hints = engine.parse_response(response).unwrap();
        assert_eq!(hints.placement_suggestions.len(), 2);
        assert_eq!(hints.routing_priorities.len(), 3);
        assert_eq!(hints.reasoning, "Test reasoning");
    }
}
