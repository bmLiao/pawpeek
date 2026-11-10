use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct HookPayload {
    pub hook_event_name: Option<String>,
    pub session_id: Option<String>,
    pub event: Option<String>,

    // StopFailure sub-fields
    pub error: Option<String>,

    // Notification sub-fields
    #[serde(rename = "notification_type")]
    pub notification_type: Option<String>,

    // PostToolUseFailure sub-fields
    #[serde(rename = "is_interrupt")]
    pub is_interrupt: Option<bool>,

    // SessionStart sub-fields
    pub source: Option<String>,
}

/// 将 Claude Code hook payload 映射为内部事件字符串。
///
/// 支持两种格式：
/// - 旧格式：直接 "event" 字段（向后兼容 / 手动测试）
/// - 新格式：hook_event_name + 各子字段
pub fn map_hook_event(payload: &HookPayload) -> &'static str {
    // 旧格式：直接 event 字段
    if let Some(ref event) = payload.event {
        return map_legacy_event(event);
    }

    match payload.hook_event_name.as_deref() {
        Some("SessionStart") => {
            // compact 已完成 → 完成态 session_start
            match payload.source.as_deref() {
                Some("compact") => "session_start",
                _ => "session_start",
            }
        }

        Some("PreCompact") => "running",

        Some("UserPromptSubmit") => "running",
        Some("PreToolUse") => "running",
        Some("PostToolUse") => "running",

        Some("PermissionRequest") => "need_confirm",

        Some("Notification") => match payload.notification_type.as_deref() {
            Some("permission_prompt" | "elicitation_dialog") => "need_confirm",
            _ => "idle",
        },

        Some("Stop") => "stop",

        Some("StopFailure") => match payload.error.as_deref() {
            Some("rate_limit" | "server_error") => "tool_error",
            Some(
                "authentication_failed"
                | "oauth_org_not_allowed"
                | "billing_error"
                | "invalid_request"
                | "model_not_found"
                | "max_output_tokens"
                | "unknown",
            ) => "error_final",
            _ => "tool_error",
        },

        // 單一工具失敗（grep 找不到、指令回傳非 0…）很常見，Claude 會自己接著做：維持工作中，
        // 避免 running ↔ review 來回切換；整輪真的失敗由 StopFailure 處理
        Some("PostToolUseFailure") => {
            if payload.is_interrupt.unwrap_or(false) {
                "stop"
            } else {
                "running"
            }
        }

        Some("SessionEnd") => "session_end",

        _ => "idle",
    }
}

/// 旧格式（直接 event 字段）的映射，用于向后兼容和手动测试。
fn map_legacy_event(event: &str) -> &'static str {
    match event {
        "session_start" => "session_start",
        "session_end" => "session_end",
        "running" => "running",
        "need_confirm" => "need_confirm",
        "tool_error" => "tool_error",
        "error_final" => "error_final",
        "stop" => "stop",
        "idle" => "idle",
        _ => "idle",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(json: &str) -> &'static str {
        let payload: HookPayload = serde_json::from_str(json).expect("valid payload");
        map_hook_event(&payload)
    }

    #[test]
    fn session_lifecycle() {
        assert_eq!(map(r#"{"hook_event_name":"SessionStart","source":"startup"}"#), "session_start");
        assert_eq!(map(r#"{"hook_event_name":"SessionStart","source":"compact"}"#), "session_start");
        assert_eq!(map(r#"{"hook_event_name":"SessionEnd"}"#), "session_end");
        assert_eq!(map(r#"{"hook_event_name":"Stop"}"#), "stop");
    }

    #[test]
    fn working_events_map_to_running() {
        for name in ["UserPromptSubmit", "PreToolUse", "PostToolUse", "PreCompact"] {
            let json = format!(r#"{{"hook_event_name":"{name}"}}"#);
            assert_eq!(map(&json), "running", "{name}");
        }
    }

    #[test]
    fn permission_and_notifications() {
        assert_eq!(map(r#"{"hook_event_name":"PermissionRequest"}"#), "need_confirm");
        assert_eq!(map(r#"{"hook_event_name":"Notification","notification_type":"permission_prompt"}"#), "need_confirm");
        assert_eq!(map(r#"{"hook_event_name":"Notification","notification_type":"elicitation_dialog"}"#), "need_confirm");
        // idle_prompt（閒置約 60 秒）會讓寵物回到閒置，也是 Esc 中斷後的恢復途徑
        assert_eq!(map(r#"{"hook_event_name":"Notification","notification_type":"idle_prompt"}"#), "idle");
    }

    #[test]
    fn tool_failure_keeps_running_unless_interrupted() {
        assert_eq!(map(r#"{"hook_event_name":"PostToolUseFailure"}"#), "running");
        assert_eq!(map(r#"{"hook_event_name":"PostToolUseFailure","is_interrupt":false}"#), "running");
        assert_eq!(map(r#"{"hook_event_name":"PostToolUseFailure","is_interrupt":true}"#), "stop");
    }

    #[test]
    fn stop_failure_severity() {
        assert_eq!(map(r#"{"hook_event_name":"StopFailure","error":"rate_limit"}"#), "tool_error");
        assert_eq!(map(r#"{"hook_event_name":"StopFailure","error":"server_error"}"#), "tool_error");
        assert_eq!(map(r#"{"hook_event_name":"StopFailure","error":"authentication_failed"}"#), "error_final");
        assert_eq!(map(r#"{"hook_event_name":"StopFailure","error":"billing_error"}"#), "error_final");
        assert_eq!(map(r#"{"hook_event_name":"StopFailure","error":"something_new"}"#), "tool_error");
        assert_eq!(map(r#"{"hook_event_name":"StopFailure"}"#), "tool_error");
    }

    #[test]
    fn legacy_event_field_takes_precedence() {
        assert_eq!(map(r#"{"event":"need_confirm","hook_event_name":"Stop"}"#), "need_confirm");
        assert_eq!(map(r#"{"event":"bogus"}"#), "idle");
    }

    #[test]
    fn unknown_or_missing_event_is_idle() {
        assert_eq!(map(r#"{"hook_event_name":"SomeFutureHook"}"#), "idle");
        assert_eq!(map(r#"{}"#), "idle");
    }
}
