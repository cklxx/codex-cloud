use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use uuid::Uuid;

use crate::codex::TurnContext;
use crate::codex::run_task;
use crate::function_tool::FunctionCallError;
use crate::protocol::InputItem;
use crate::state::TaskKind;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct SubAgentHandler;

#[derive(Debug, Deserialize)]
struct SubAgentArgs {
    task: String,
    #[serde(default)]
    instructions: Option<String>,
}

#[async_trait]
impl ToolHandler for SubAgentHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "subagent handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: SubAgentArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e}"))
        })?;

        if args.task.trim().is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "subagent task must not be empty".to_string(),
            ));
        }

        let mut tools_config = turn.tools_config.clone();
        tools_config.include_subagent_tool = false;

        let combined_instructions = match (&turn.user_instructions, args.instructions) {
            (Some(existing), Some(extra)) if !extra.trim().is_empty() => Some(format!(
                "{existing}\n\n{sub_extra}",
                sub_extra = extra.trim()
            )),
            (None, Some(extra)) if !extra.trim().is_empty() => Some(extra.trim().to_string()),
            (Some(existing), _) => Some(existing.clone()),
            (None, _) => None,
        };

        let sub_turn_context = TurnContext {
            client: turn.client.clone(),
            tools_config,
            user_instructions: combined_instructions,
            base_instructions: turn.base_instructions.clone(),
            approval_policy: turn.approval_policy,
            sandbox_policy: turn.sandbox_policy.clone(),
            shell_environment_policy: turn.shell_environment_policy.clone(),
            cwd: turn.cwd.clone(),
            is_review_mode: false,
            final_output_json_schema: None,
        };
        let sub_turn_context = Arc::new(sub_turn_context);

        let sub_task_id = format!("subagent-{}", Uuid::new_v4().simple());
        let input = vec![InputItem::Text { text: args.task }];

        let last_agent_message = run_task(
            Arc::clone(&session),
            Arc::clone(&sub_turn_context),
            sub_task_id.clone(),
            input,
            TaskKind::Regular,
        )
        .await;

        session
            .on_task_finished(sub_task_id, last_agent_message.clone())
            .await;

        let (content, success) = match last_agent_message {
            Some(message) => (message, true),
            None => (
                "Sub-agent completed without returning a final message.".to_string(),
                false,
            ),
        };

        Ok(ToolOutput::Function {
            content,
            success: Some(success),
        })
    }
}
