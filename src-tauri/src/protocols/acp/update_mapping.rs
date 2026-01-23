use agent_client_protocol as acp;
use serde::Serialize;

use crate::api::types::{AcpSessionUpdate, SessionId};

pub fn parse_acp_session_notification_params(
    params: serde_json::Value,
    fallback_session_id: &SessionId,
) -> Result<(SessionId, acp::SessionUpdate), serde_json::Error> {
    match serde_json::from_value::<acp::SessionNotification>(params.clone()) {
        Ok(notification) => Ok((notification.session_id.to_string(), notification.update)),
        Err(_) => serde_json::from_value::<acp::SessionUpdate>(params)
            .map(|update| (fallback_session_id.clone(), update)),
    }
}

pub fn map_acp_update_to_api_update(update: acp::SessionUpdate) -> AcpSessionUpdate {
    match update {
        acp::SessionUpdate::UserMessageChunk(chunk) => AcpSessionUpdate::UserMessageChunk {
            content: to_value_or_null(chunk.content),
        },
        acp::SessionUpdate::AgentMessageChunk(chunk) => AcpSessionUpdate::AgentMessageChunk {
            content: to_value_or_null(chunk.content),
        },
        acp::SessionUpdate::AgentThoughtChunk(chunk) => AcpSessionUpdate::AgentThoughtChunk {
            content: to_value_or_null(chunk.content),
        },
        acp::SessionUpdate::ToolCall(tool_call) => AcpSessionUpdate::ToolCall {
            tool_call: to_value_or_null(tool_call),
        },
        acp::SessionUpdate::ToolCallUpdate(tool_call_update) => AcpSessionUpdate::ToolCallUpdate {
            tool_call_update: to_value_or_null(tool_call_update),
        },
        acp::SessionUpdate::Plan(plan) => AcpSessionUpdate::Plan {
            plan: to_value_or_null(plan),
        },
        acp::SessionUpdate::AvailableCommandsUpdate(update) => {
            AcpSessionUpdate::AvailableCommandsUpdate {
                available_commands: to_value_or_null(update.available_commands),
            }
        }
        acp::SessionUpdate::CurrentModeUpdate(update) => AcpSessionUpdate::CurrentModeUpdate {
            current_mode_id: to_value_or_null(update.current_mode_id),
        },
        acp::SessionUpdate::ConfigOptionUpdate(update) => AcpSessionUpdate::ConfigOptionUpdate {
            config_options: to_value_or_null(update.config_options),
        },
        _ => AcpSessionUpdate::Raw {
            json: to_value_or_null(update),
        },
    }
}

fn to_value_or_null<T: Serialize>(value: T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_session_notification_params() {
        let update = acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
            acp::ContentBlock::from("Hello"),
        ));
        let notification = acp::SessionNotification::new("session-1", update.clone());
        let params = serde_json::to_value(notification).expect("serialize notification");

        let (session_id, parsed_update) =
            parse_acp_session_notification_params(params, &"fallback".to_string())
                .expect("parse session notification");

        assert_eq!(session_id, "session-1");
        assert!(matches!(
            parsed_update,
            acp::SessionUpdate::AgentMessageChunk(_)
        ));
    }

    #[test]
    fn test_parse_session_notification_params_fallback_update() {
        let update = acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(
            acp::ContentBlock::from("Hi"),
        ));
        let params = serde_json::to_value(update.clone()).expect("serialize update");

        let (session_id, parsed_update) =
            parse_acp_session_notification_params(params, &"fallback-session".to_string())
                .expect("parse update params");

        assert_eq!(session_id, "fallback-session");
        assert!(matches!(
            parsed_update,
            acp::SessionUpdate::UserMessageChunk(_)
        ));
    }

    #[test]
    fn test_map_acp_update_to_api_update_content() {
        let update = acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
            acp::ContentBlock::from("Hello"),
        ));

        let mapped = map_acp_update_to_api_update(update);

        match mapped {
            AcpSessionUpdate::AgentMessageChunk { content } => {
                assert_eq!(
                    content,
                    serde_json::json!({"type": "text", "text": "Hello"})
                );
            }
            _ => panic!("Expected AgentMessageChunk"),
        }
    }
}
