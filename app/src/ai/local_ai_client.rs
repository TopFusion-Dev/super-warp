//! Standalone Direct AI Client for super-warp-terminal
//!
//! This module provides a direct MiniMax AI client that bypasses warp-server.
//! Used when users want to use their own API key without requiring the server.

use std::sync::Arc;
use async_trait::async_trait;
use serde_json::json;

use crate::ai::llms::{
    AvailableLLMs, LLMContextWindow, LLMId, LLMInfo, LLMProvider, LLMSpec, LLMUsageMetadata,
    ModelsByFeature,
};
use crate::ai_assistant::{
    execution_context::WarpAiExecutionContext,
    requests::GenerateDialogueResult,
    utils::TranscriptPart,
    AIGeneratedCommand, GenerateCommandsFromNaturalLanguageError,
};
use crate::server::server_api::ai::{
    AgentTaskState, SpawnAgentRequest, SpawnAgentResponse, TaskStatusUpdate, AmbientAgentTaskId,
};

/// Configuration for direct MiniMax API access
#[derive(Clone, Debug)]
pub struct DirectMinimaxConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl DirectMinimaxConfig {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("MINIMAX_API_KEY").ok()?;
        let base_url = std::env::var("MINIMAX_BASE_URL")
            .unwrap_or_else(|_| "https://api.minimax.io/anthropic/v1".to_string());
        let model = std::env::var("MINIMAX_MODEL")
            .unwrap_or_else(|_| "MiniMax-M2.7".to_string());

        Some(Self {
            api_key,
            base_url,
            model,
        })
    }
}

/// Direct MiniMax AI Client - bypasses warp-server for AI calls
pub struct DirectMinimaxClient {
    config: DirectMinimaxConfig,
    http_client: reqwest::Client,
}

impl DirectMinimaxClient {
    pub fn new(config: DirectMinimaxConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }

    pub fn from_env() -> Option<Arc<Self>> {
        DirectMinimaxConfig::from_env().map(|config| Arc::new(Self::new(config)))
    }

    async fn call_minimax(&self, messages: Vec<serde_json::Value>, max_tokens: u32) -> anyhow::Result<String> {
        let response = self
            .http_client
            .post(format!("{}/messages", self.config.base_url))
            .header("x-api-key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.config.model,
                "max_tokens": max_tokens,
                "messages": messages
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("MiniMax API error {}: {}", status, error_text);
        }

        let response_json: serde_json::Value = response.json().await?;

        // Extract text content from response
        if let Some(content) = response_json.get("content").and_then(|c| c.as_array()) {
            for item in content {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    return Ok(text.to_string());
                }
            }
        }

        Ok(response_json.to_string())
    }

    fn minimax_llm_info(model_id: &str, display_name: &str) -> LLMInfo {
        LLMInfo {
            display_name: display_name.to_string(),
            base_model_name: model_id.to_string(),
            id: LLMId(model_id.to_string()),
            reasoning_level: Some("high".to_string()),
            usage_metadata: LLMUsageMetadata {
                request_multiplier: 1,
                credit_multiplier: None,
            },
            description: Some("MiniMax M2.7 - 196K context window, reasoning, tool calls".to_string()),
            disable_reason: None,
            vision_supported: true,
            spec: Some(LLMSpec {
                cost: 0.0,
                quality: 0.9,
                speed: 0.8,
            }),
            provider: LLMProvider::MiniMax,
            host_configs: Default::default(),
            discount_percentage: None,
            context_window: LLMContextWindow {
                is_configurable: false,
                min: 4096,
                max: 196608,
                default_max: 196608,
            },
        }
    }
}

#[async_trait]
impl AIClient for DirectMinimaxClient {
    async fn generate_commands_from_natural_language(
        &self,
        prompt: String,
        _ai_execution_context: Option<WarpAiExecutionContext>,
    ) -> Result<Vec<AIGeneratedCommand>, GenerateCommandsFromNaturalLanguageError> {
        let messages = vec![
            json!({
                "role": "user",
                "content": format!(
                    "Given the following natural language request, generate the appropriate shell command(s). \
                    Return ONLY the commands, one per line, with no additional explanation:\n\n{}",
                    prompt
                )
            })
        ];

        match self.call_minimax(messages, 200).await {
            Ok(response) => {
                let commands: Vec<AIGeneratedCommand> = response
                    .lines()
                    .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
                    .map(|line| AIGeneratedCommand {
                        command: line.trim().to_string(),
                        description: None,
                        rank: 0,
                    })
                    .collect();

                if commands.is_empty() {
                    // If no commands parsed, return the whole response as one command
                    Ok(vec![AIGeneratedCommand {
                        command: response.trim().to_string(),
                        description: None,
                        rank: 0,
                    }])
                } else {
                    Ok(commands)
                }
            }
            Err(e) => {
                log::error!("DirectMinimaxClient: generate_commands_from_natural_language failed: {}", e);
                Err(GenerateCommandsFromNaturalLanguageError::Other)
            }
        }
    }

    async fn generate_dialogue_answer(
        &self,
        transcript: Vec<TranscriptPart>,
        prompt: String,
        _ai_execution_context: Option<WarpAiExecutionContext>,
    ) -> anyhow::Result<GenerateDialogueResult> {
        let mut messages: Vec<serde_json::Value> = Vec::new();

        for part in transcript {
            messages.push(json!({
                "role": "user",
                "content": part.raw_user_prompt()
            }));
            messages.push(json!({
                "role": "assistant",
                "content": part.raw_assistant_answer()
            }));
        }

        messages.push(json!({
            "role": "user",
            "content": prompt
        }));

        match self.call_minimax(messages, 1024).await {
            Ok(answer) => {
                Ok(GenerateDialogueResult::Success {
                    answer,
                    truncated: false,
                    request_limit_info: crate::ai::RequestUsageInfo::default(),
                    transcript_summarized: false,
                })
            }
            Err(e) => {
                log::error!("DirectMinimaxClient: generate_dialogue_answer failed: {}", e);
                Err(anyhow::anyhow!("MiniMax API error: {}", e))
            }
        }
    }

    async fn generate_metadata_for_command(
        &self,
        command: String,
    ) -> Result<crate::drive::workflows::ai_assist::GeneratedCommandMetadata, GenerateCommandsFromNaturalLanguageError> {
        Ok(crate::drive::workflows::ai_assist::GeneratedCommandMetadata {
            command,
            description: None,
            tags: vec![],
        })
    }

    async fn get_request_limit_info(&self) -> anyhow::Result<crate::ai::RequestUsageInfo> {
        Ok(crate::ai::RequestUsageInfo::default())
    }

    async fn get_feature_model_choices(&self) -> anyhow::Result<ModelsByFeature> {
        let minimax_m2_7 = Self::minimax_llm_info("MiniMax-M2.7", "MiniMax M2.7");
        let minimax_coding = Self::minimax_llm_info("minimax-coding-plan/MiniMax-M2.7", "MiniMax Coding Plan");
        let minimax_commander = Self::minimax_llm_info("minimax/minimax-m2.7", "MiniMax Commander");

        Ok(ModelsByFeature {
            agent_mode: AvailableLLMs {
                default_id: LLMId("MiniMax-M2.7".to_string()),
                choices: vec![minimax_m2_7.clone()],
                preferred_codex_model_id: None,
            },
            coding: AvailableLLMs {
                default_id: LLMId("MiniMax-M2.7".to_string()),
                choices: vec![minimax_m2_7.clone(), minimax_coding],
                preferred_codex_model_id: None,
            },
            cli_agent: Some(AvailableLLMs {
                default_id: LLMId("MiniMax-M2.7".to_string()),
                choices: vec![minimax_m2_7.clone()],
                preferred_codex_model_id: None,
            }),
            computer_use: Some(AvailableLLMs {
                default_id: LLMId("MiniMax-M2.7".to_string()),
                choices: vec![minimax_m2_7],
                preferred_codex_model_id: None,
            }),
        })
    }

    async fn get_available_harnesses(&self) -> anyhow::Result<Vec<crate::ai::harness_availability::HarnessAvailability>> {
        Ok(vec![])
    }

    async fn get_free_available_models(
        &self,
        _referrer: Option<String>,
    ) -> anyhow::Result<ModelsByFeature> {
        self.get_feature_model_choices().await
    }

    async fn update_merkle_tree(
        &self,
        _embedding_config: crate::ai::index::full_source_code_embedding::EmbeddingConfig,
        _nodes: Vec<crate::ai::index::full_source_code_embedding::store_client::IntermediateNode>,
    ) -> anyhow::Result<std::collections::HashMap<crate::ai::index::full_source_code_embedding::NodeHash, bool>> {
        Ok(std::collections::HashMap::new())
    }

    async fn generate_code_embeddings(
        &self,
        _embedding_config: crate::ai::index::full_source_code_embedding::EmbeddingConfig,
        _fragments: Vec<crate::ai::index::full_source_code_embedding::Fragment>,
        _root_hash: crate::ai::index::full_source_code_embedding::NodeHash,
        _repo_metadata: crate::ai::index::full_source_code_embedding::RepoMetadata,
    ) -> anyhow::Result<std::collections::HashMap<crate::ai::index::full_source_code_embedding::ContentHash, bool>> {
        Ok(std::collections::HashMap::new())
    }

    async fn provide_negative_feedback_response_for_ai_conversation(
        &self,
        _conversation_id: String,
        _request_ids: Vec<String>,
    ) -> anyhow::Result<i32> {
        Ok(0)
    }

    async fn create_agent_task(
        &self,
        _prompt: String,
        _environment_uid: Option<String>,
        _parent_run_id: Option<String>,
        _config: Option<crate::ai::ambient_agents::AgentConfigSnapshot>,
    ) -> anyhow::Result<AmbientAgentTaskId> {
        Err(anyhow::anyhow!("Agent tasks require warp-server"))
    }

    async fn update_agent_task(
        &self,
        _task_id: AmbientAgentTaskId,
        _task_state: Option<AgentTaskState>,
        _session_id: Option<crate::session_sharing_protocol::common::SessionId>,
        _conversation_id: Option<String>,
        _status_message: Option<TaskStatusUpdate>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn spawn_agent(
        &self,
        _request: SpawnAgentRequest,
    ) -> anyhow::Result<SpawnAgentResponse> {
        Err(anyhow::anyhow!("Agent spawning requires warp-server"))
    }

    async fn upload_local_handoff_snapshot(
        &self,
        _request: crate::server::server_api::ai::UploadLocalHandoffSnapshotRequest,
    ) -> anyhow::Result<crate::server::server_api::ai::UploadLocalHandoffSnapshotResponse> {
        Err(anyhow::anyhow!("Handoff requires warp-server"))
    }

    async fn fork_conversation(
        &self,
        _conversation_id: String,
        _title: Option<String>,
    ) -> anyhow::Result<crate::server::server_api::ai::ForkConversationResponse> {
        Err(anyhow::anyhow!("Conversation fork requires warp-server"))
    }

    async fn list_ambient_agent_tasks(
        &self,
        _limit: i32,
        _filter: crate::server::server_api::ai::TaskListFilter,
    ) -> anyhow::Result<Vec<crate::ai::ambient_agents::AmbientAgentTask>> {
        Ok(vec![])
    }

    async fn list_agent_runs_raw(
        &self,
        _limit: i32,
        _filter: crate::server::server_api::ai::TaskListFilter,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::json!({ "runs": [] }))
    }

    async fn get_ambient_agent_task(
        &self,
        _task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<crate::ai::ambient_agents::AmbientAgentTask> {
        Err(anyhow::anyhow!("Agent tasks require warp-server"))
    }

    async fn get_agent_run_raw(
        &self,
        _task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::json!({}))
    }

    async fn submit_run_followup(
        &self,
        _run_id: &AmbientAgentTaskId,
        _request: crate::server::server_api::ai::RunFollowupRequest,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_scheduled_agent_history(
        &self,
        _schedule_id: &str,
    ) -> anyhow::Result<crate::server::server_api::ai::ScheduledAgentHistory> {
        Ok(crate::server::server_api::ai::ScheduledAgentHistory {
            schedule_id: "".to_string(),
            agent_ids: vec![],
            runs: vec![],
        })
    }

    async fn get_ai_conversation(
        &self,
        _server_conversation_token: crate::ai::agent::api::ServerConversationToken,
    ) -> anyhow::Result<(warp_multi_agent_api::ConversationData, crate::ai::agent::conversation::ServerAIConversationMetadata)> {
        Err(anyhow::anyhow!("Conversations require warp-server"))
    }

    async fn list_ai_conversation_metadata(
        &self,
        _conversation_ids: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<crate::ai::agent::conversation::ServerAIConversationMetadata>> {
        Ok(vec![])
    }

    async fn get_ai_conversation_format(
        &self,
        _server_conversation_token: crate::ai::agent::api::ServerConversationToken,
    ) -> anyhow::Result<crate::ai::agent::conversation::AIAgentConversationFormat> {
        Err(anyhow::anyhow!("Conversation format requires warp-server"))
    }

    async fn get_block_snapshot(
        &self,
        _server_conversation_token: crate::ai::agent::api::ServerConversationToken,
    ) -> anyhow::Result<crate::terminal::model::block::SerializedBlock> {
        Err(anyhow::anyhow!("Block snapshot requires warp-server"))
    }

    async fn delete_ai_conversation(
        &self,
        _server_conversation_token: String,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn list_agents(
        &self,
        _repo: Option<String>,
    ) -> anyhow::Result<Vec<crate::server::server_api::ai::AgentListItem>> {
        Ok(vec![])
    }

    async fn cancel_ambient_agent_task(
        &self,
        _task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_task_git_credentials(
        &self,
        _task_id: String,
        _workload_token: String,
    ) -> anyhow::Result<Vec<crate::server::server_api::ai::GitCredential>> {
        Ok(vec![])
    }

    async fn get_task_attachments(
        &self,
        _task_id: String,
    ) -> anyhow::Result<Vec<crate::ai::ambient_agents::task::TaskAttachment>> {
        Ok(vec![])
    }

    async fn create_file_artifact_upload_target(
        &self,
        _request: crate::server::server_api::ai::CreateFileArtifactUploadRequest,
    ) -> anyhow::Result<crate::server::server_api::ai::CreateFileArtifactUploadResponse> {
        Err(anyhow::anyhow!("Artifact upload requires warp-server"))
    }

    async fn confirm_file_artifact_upload(
        &self,
        _artifact_uid: String,
        _checksum: String,
    ) -> anyhow::Result<crate::server::server_api::ai::FileArtifactRecord> {
        Err(anyhow::anyhow!("Artifact confirmation requires warp-server"))
    }

    async fn get_artifact_download(
        &self,
        _artifact_uid: &str,
    ) -> anyhow::Result<crate::server::server_api::ai::ArtifactDownloadResponse> {
        Err(anyhow::anyhow!("Artifact download requires warp-server"))
    }

    async fn prepare_attachments_for_upload(
        &self,
        _task_id: &AmbientAgentTaskId,
        _files: &[crate::server::server_api::ai::AttachmentFileInfo],
    ) -> anyhow::Result<crate::server::server_api::ai::PrepareAttachmentUploadsResponse> {
        Err(anyhow::anyhow!("Attachment preparation requires warp-server"))
    }

    async fn download_task_attachments(
        &self,
        _task_id: &AmbientAgentTaskId,
        _attachment_ids: &[String],
    ) -> anyhow::Result<crate::server::server_api::ai::DownloadAttachmentsResponse> {
        Err(anyhow::anyhow!("Attachment download requires warp-server"))
    }

    async fn get_handoff_snapshot_attachments(
        &self,
        _task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<Vec<crate::ai::ambient_agents::task::TaskAttachment>> {
        Ok(vec![])
    }

    async fn send_agent_message(
        &self,
        _request: crate::server::server_api::ai::SendAgentMessageRequest,
    ) -> anyhow::Result<crate::server::server_api::ai::SendAgentMessageResponse> {
        Err(anyhow::anyhow!("Messaging requires warp-server"))
    }

    async fn list_agent_messages(
        &self,
        _run_id: &str,
        _request: crate::server::server_api::ai::ListAgentMessagesRequest,
    ) -> anyhow::Result<Vec<crate::server::server_api::ai::AgentMessageHeader>> {
        Ok(vec![])
    }

    async fn update_event_sequence_on_server(
        &self,
        _run_id: &str,
        _sequence: i64,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn report_agent_event(
        &self,
        _run_id: &str,
        _request: crate::server::server_api::ai::ReportAgentEventRequest,
    ) -> anyhow::Result<crate::server::server_api::ai::ReportAgentEventResponse> {
        Err(anyhow::anyhow!("Event reporting requires warp-server"))
    }

    async fn mark_message_delivered(&self, _message_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn read_agent_message(
        &self,
        _message_id: &str,
    ) -> anyhow::Result<crate::server::server_api::ai::ReadAgentMessageResponse> {
        Err(anyhow::anyhow!("Message reading requires warp-server"))
    }

    async fn get_public_conversation(
        &self,
        _conversation_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::json!({}))
    }

    async fn get_run_conversation(
        &self,
        _run_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::json!({}))
    }

    async fn generate_code_review_content(
        &self,
        _request: crate::ai::generate_code_review_content::api::GenerateCodeReviewContentRequest,
    ) -> Result<crate::ai::generate_code_review_content::api::GenerateCodeReviewContentResponse, anyhow::Error> {
        Err(anyhow::anyhow!("Code review requires warp-server"))
    }
}