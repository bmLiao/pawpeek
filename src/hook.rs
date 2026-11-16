use serde::Deserialize;

/// hook 資料。Claude Code 與 Copilot（VS Code / Copilot CLI）的相容格式都是 snake_case；
/// Copilot 的少數事件（如 Notification）混用 camelCase，用 alias 一併接受。
#[derive(Debug, Deserialize)]
pub struct HookPayload {
    #[serde(alias = "hookEventName")]
    pub hook_event_name: Option<String>,
    /// Copilot 原生格式（camelCase）的事件名稱，例如 `permissionRequest`（沒有 hook_event_name 時才用）
    #[serde(rename = "hookName")]
    pub hook_name: Option<String>,
    #[serde(alias = "sessionId")]
    pub session_id: Option<String>,
    pub event: Option<String>,

    // StopFailure sub-fields（Claude Code：字串）；Copilot 的 ErrorOccurred 的 error 可能是物件，用 Value 接
    #[serde(default, deserialize_with = "string_or_any")]
    pub error: Option<String>,

    // Copilot ErrorOccurred：錯誤能否自動恢復
    pub recoverable: Option<bool>,

    // Notification sub-fields
    #[serde(rename = "notification_type", alias = "notificationType")]
    pub notification_type: Option<String>,

    // PostToolUseFailure sub-fields
    #[serde(rename = "is_interrupt")]
    pub is_interrupt: Option<bool>,

    // SessionStart sub-fields
    pub source: Option<String>,

    /// 對話的工作資料夾（泡泡顯示最後一層的名稱，分辨是哪個專案的對話）
    pub cwd: Option<String>,

    /// PawPeek 自己加的：執行 hook 的 agent 程序 PID（見 agent.rs；agent 結束就把對話當作結束）
    pub pawpeek_agent_pid: Option<u32>,
}

/// 字串照收；其他型別（物件、數字…）轉成 JSON 文字，避免整筆資料解析失敗
fn string_or_any<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.map(|v| match v { serde_json::Value::String(s) => s, other => other.to_string() }))
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

    // Copilot 原生格式（只有 hookName）
    if payload.hook_event_name.is_none() {
        if let Some(ref name) = payload.hook_name {
            return map_copilot_native(name, payload);
        }
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

        // Copilot（VS Code / Copilot CLI）：沒有 StopFailure，錯誤走 ErrorOccurred
        Some("ErrorOccurred") => {
            if payload.recoverable.unwrap_or(true) { "tool_error" } else { "error_final" }
        }

        _ => "idle",
    }
}

/// Copilot 原生格式（camelCase 事件名）的映射。
/// `permissionRequest` 在 Copilot 是「每次工具呼叫、權限檢查前」都會送（不代表真的跳出確認），所以視為工作中；
/// 真的在等使用者確認要看 `notification`（permission_prompt / elicitation_dialog）。
fn map_copilot_native(name: &str, payload: &HookPayload) -> &'static str {
    match name {
        "notification" => match payload.notification_type.as_deref() {
            Some("permission_prompt" | "elicitation_dialog") => "need_confirm",
            _ => "idle",
        },
        "sessionStart" => "session_start",
        "sessionEnd" => "session_end",
        "userPromptSubmitted" | "preToolUse" | "postToolUse" | "postToolUseFailure" | "preCompact"
        | "permissionRequest" | "subagentStart" | "subagentStop" => "running",
        "agentStop" => "stop",
        "errorOccurred" => "tool_error",
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
    fn copilot_payloads() {
        // 相容格式（Copilot CLI 實測）
        assert_eq!(map(r#"{"hook_event_name":"SessionStart","session_id":"s","source":"new","initial_prompt":"x"}"#), "session_start");
        assert_eq!(map(r#"{"hook_event_name":"Stop","session_id":"s","stop_reason":"end_turn","stop_hook_active":false}"#), "stop");
        // Notification 混用 camelCase
        let p: HookPayload = serde_json::from_str(r#"{"hook_event_name":"Notification","sessionId":"abc","notification_type":"permission_prompt"}"#).unwrap();
        assert_eq!(p.session_id.as_deref(), Some("abc"));
        assert_eq!(map_hook_event(&p), "need_confirm");
        // ErrorOccurred：error 可能是物件
        assert_eq!(map(r#"{"hook_event_name":"ErrorOccurred","error":{"message":"boom"},"recoverable":true}"#), "tool_error");
        assert_eq!(map(r#"{"hook_event_name":"ErrorOccurred","error":"fatal","recoverable":false}"#), "error_final");
        assert_eq!(map(r#"{"hook_event_name":"ErrorOccurred"}"#), "tool_error");
        // 原生格式的 permissionRequest（Copilot CLI 實測：每次工具呼叫都會送）→ 維持工作中
        let p: HookPayload = serde_json::from_str(r#"{"hookName":"permissionRequest","sessionId":"abc","toolName":"powershell","toolInput":{"command":"echo"}}"#).unwrap();
        assert_eq!(p.session_id.as_deref(), Some("abc"));
        assert_eq!(map_hook_event(&p), "running");
        assert_eq!(map(r#"{"hookName":"notification","notification_type":"permission_prompt"}"#), "need_confirm");
        assert_eq!(map(r#"{"hookName":"agentStop"}"#), "stop");
    }

    #[test]
    fn unknown_or_missing_event_is_idle() {
        assert_eq!(map(r#"{"hook_event_name":"SomeFutureHook"}"#), "idle");
        assert_eq!(map(r#"{}"#), "idle");
    }
}
