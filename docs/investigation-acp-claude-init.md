# Investigation: ACP Claude Initialization Flow Comparison

## Summary

Investigating why the Tauri ACP template shows authentication prompt during Claude Code ACP initialization, while Zed using the same plugin on the same machine doesn't require authentication.

## Symptoms

- After successful `initialize` handshake, `session/new` completes successfully
- Session ID received: `27b57109-db88-47cf-b0da-41ced9c7e01b`
- Agent status changes to "started"
- Prompt sent successfully
- **Critical**: Failed to parse session update with error: `missing field 'type', keys=Some(["availableCommands", "sessionUpdate"])`
- `authMethods` returned in initialize response suggests running `claude /login`
- Zed with same Claude Code adapter on same machine works without authentication

## Initial Observations from Logs

```
[22:46:04] initialize response received:
- agentInfo: @zed-industries/claude-code-acp v0.13.1
- authMethods: [{"id":"claude-login","name":"Log in with Claude Code","description":"Run `claude /login` in the terminal"}]
- protocolVersion: 1

[22:46:11] session/new response:
- sessionId: 27b57109-db88-47cf-b0da-41ced9c7e01b
- models/modes returned successfully
- No auth error in the response

[22:46:11] Prompt sent successfully

[22:46:11] Parse error: missing field 'type', keys=Some(["availableCommands", "sessionUpdate"])
```

## Hypotheses

### H1: Authentication State Not Being Read/Passed

- Zed may be passing authentication tokens/credentials during initialization
- Our implementation might not be reading or forwarding auth state

### H2: Session Update Parsing Issue

- The parse error suggests we're receiving data but can't deserialize it properly
- Message has `availableCommands` and `sessionUpdate` keys but missing `type` field
- This could be a secondary symptom, not the root cause of auth prompt

### H3: Environment Variables or Configuration

- Claude Code might check specific env vars that Zed sets
- Working directory, HOME, or other environment context differences

### H4: Request Parameter Differences

- `initialize` or `session/new` requests might differ between Zed and our implementation
- Missing optional parameters that affect authentication flow

## Investigation Log

### [Initial] - Log Analysis

**Hypothesis:** Understanding the exact failure point
**Findings:**

- Handshake completes successfully
- Session created successfully
- Parse failure happens AFTER prompt is sent
  **Evidence:** Log timestamps show parse error after "Prompt sent successfully"
  **Conclusion:** Authentication prompt might be unrelated to parse error, or parse error prevents auth response handling

## Investigation Findings

### Phase 1: Tauri Implementation Analysis

**Evidence:** `src-tauri/src/runtime/agents.rs:361-376`

```rust
let mut cmd = Command::new(&command.path);
cmd.args(&command.args)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .env_clear();

// Only forward specific environment variables if they exist
if let Ok(base_url) = std::env::var("ANTHROPIC_BASE_URL") {
    cmd.env("ANTHROPIC_BASE_URL", base_url);
}
if let Ok(auth_token) = std::env::var("ANTHROPIC_AUTH_TOKEN") {
    cmd.env("ANTHROPIC_AUTH_TOKEN", auth_token);
}
```

**Finding 1: Limited Environment Variable Passing**

- Tauri ONLY forwards `ANTHROPIC_BASE_URL` and `ANTHROPIC_AUTH_TOKEN`
- Uses `.env_clear()` to start with a clean slate
- Only adds these vars IF they exist in parent process
- When launching from Finder (macOS), GUI apps don't inherit shell environment

**Evidence:** `src-tauri/src/protocols/acp/agent.rs:180-200`

```rust
let response: JsonRpcResponse<InitializeResponse> = serde_json::from_value(response_val)
    .map_err(|e| AcpError::ProtocolError(format!("Invalid initialize response: {}", e)))?;

if let Some(error) = response.error {
    return Err(AcpError::RpcError(error));
}

let result = response
    .result
    .ok_or_else(|| AcpError::ProtocolError("Missing result in initialize response".into()))?;

// Store session capabilities and agent info but DON'T USE authMethods
self.agent_info = Some(result.agent_info.clone());
self.session_capabilities = Some(result.agent_capabilities.session_capabilities.clone());
```

**Finding 2: authMethods Ignored**

- Initialize response contains `authMethods` array
- Tauri receives it but NEVER stores or uses it
- No authentication flow triggered based on this field

**Evidence:** `src-tauri/src/protocols/acp/agent.rs:520-560` (extract_session_update)

```rust
fn extract_session_update(
    params: &serde_json::Value,
    fallback_session_id: &SessionId,
) -> (SessionId, serde_json::Value) {
    // Only handles sessionUpdate as STRING
    // Doesn't handle sessionUpdate as OBJECT
    // Doesn't handle bare availableCommands
```

**Finding 3: Session Update Parsing Too Strict**

- Only handles `sessionUpdate` when it's a string type discriminator
- Doesn't handle `sessionUpdate` as nested object with `type` field
- Doesn't handle bare `availableCommands` arrays
- Falls back to `Raw` variant on any parse failure

### Phase 2: Zed Implementation Analysis

**Evidence:** `zed/crates/project/src/agent_server_store.rs:1420-1545`

```rust
impl LocalClaudeCode {
    fn get_command(...) -> Result<(AgentServerCommand, SpawnInTerminal)> {
        // Build comprehensive environment
        let mut env = ProjectEnvironment::local_directory_environment(
            &Shell::System,
            root_dir,
            project_env,
            cx,
        )?;

        // Set default (empty) ANTHROPIC_API_KEY
        env.insert("ANTHROPIC_API_KEY".into(), "".into());

        // Merge settings_env (can override ANTHROPIC_API_KEY)
        env.extend(settings_env.unwrap_or_default());

        // Add extra_env overrides
        command.env.get_or_insert_default().extend(extra_env);
```

**Finding 4: Zed's Comprehensive Environment**

- Starts with full project/shell environment (NOT env_clear)
- Adds ANTHROPIC_API_KEY with empty default
- Merges user settings environment
- Applies extra environment overrides
- Result: adapter gets full shell context + custom settings

**Evidence:** `zed/crates/agent_servers/src/acp.rs:200-220`

```rust
let response: JsonRpcResponse<InitializeResponse> = ...
let result = response.result.ok_or(...)?;

// Store auth_methods for UI
let auth_methods = result.auth_methods.unwrap_or_default();

Ok(Self {
    auth_methods,  // ← Stored and exposed
    agent_capabilities: result.agent_capabilities,
    // ...
})
```

**Finding 5: Zed Stores and Uses authMethods**

- authMethods from initialize response stored in `AcpConnection`
- Exposed via `auth_methods()` method to UI
- UI presents authentication options when needed
- Implements `authenticate(method_id)` to handle auth flows

**Evidence:** `zed/crates/agent_ui/src/acp/thread_view.rs:1960-2238`

```rust
fn authenticate(&mut self, method_id: String, ...) {
    // Check for terminal-auth meta
    if let Some(meta) = auth_method._meta {
        if meta.get("terminal-auth").is_some() {
            // Spawn terminal auth task
        }
    }

    // Hardcoded Claude login
    if method_id == "claude-login" && self.login.is_some() {
        spawn_external_agent_login(self.login, ...);
        return;
    }

    // Fallback to connection.authenticate()
    connection.authenticate(method_id, cx);
}
```

**Finding 6: Zed's Multi-Path Authentication**

- Checks for terminal-based auth in metadata
- Has hardcoded `"claude-login"` handler that runs `/login` command
- Falls back to ACP authenticate RPC call
- Monitors terminal output for success indicators

**Evidence:** `zed/crates/agent_servers/src/acp.rs:1080-1279`

```rust
fn session_notification(&mut self, notification: SessionNotification, cx: &mut ModelContext<Self>) {
    // Pre-process: update mode/config caches
    match &notification.update {
        SessionUpdate::CurrentModeUpdate(update) => { /* cache */ }
        SessionUpdate::ConfigOptionUpdate(update) => { /* cache */ }
        _ => {}
    }

    // Handle terminal meta
    if let SessionUpdate::ToolCall { meta, ... } = &notification.update {
        if let Some(terminal_info) = &meta.terminal_info {
            // Create terminal
        }
    }

    // Forward to thread
    thread.handle_session_update(notification.update.clone(), cx);

    // Post-process: terminal output/exit
    if let SessionUpdate::ToolCallUpdate { meta, ... } = &notification.update {
        if let Some(output) = &meta.terminal_output { /* emit */ }
        if let Some(exit) = &meta.terminal_exit { /* emit */ }
    }
}
```

**Finding 7: Zed's Robust Session Update Handling**

- Pre-processes updates to maintain state caches
- Extracts and handles terminal metadata
- Forwards typed updates to thread model
- Post-processes for terminal I/O events
- No parsing errors logged - proper type handling

## Root Cause Analysis

### Issue 1: Authentication Prompt

**Root Cause:** Combination of environment and authMethod handling

1. **Environment inheritance problem:**
   - Tauri uses `.env_clear()` and only forwards 2 specific vars
   - When launched from GUI (Finder), no shell env inherited
   - ANTHROPIC_AUTH_TOKEN likely not available
   - Claude Code adapter can't find credentials → triggers auth

2. **No authMethods handling:**
   - Tauri receives authMethods but ignores them
   - No UI to present authentication options
   - No `authenticate()` RPC implementation
   - User has no way to complete auth flow within app

3. **Why Zed works:**
   - Preserves full shell environment (including PATH, HOME, etc.)
   - Claude Code can find credentials in standard locations
   - OR implements full auth flow when credentials missing
   - Provides UI-driven authentication via authMethods

### Issue 2: Session Update Parse Errors

**Root Cause:** Inflexible message unwrapping logic

1. **extract_session_update() too narrow:**
   - Only handles `sessionUpdate: "string"` (type discriminator)
   - Doesn't handle `sessionUpdate: { type: "...", ... }` (nested object)
   - Doesn't synthesize type for bare `availableCommands` payloads
   - Falls through to returning raw params without `type` field

2. **Tagged enum deserialization:**
   - `AcpSessionUpdate` uses `#[serde(tag = "type")]`
   - REQUIRES a `type` field at top level
   - When extract_session_update returns object without `type` → parse error
   - Error logged, wrapped in `Raw` variant

3. **Why Zed works:**
   - Uses strongly typed enums directly from ACP protocol definitions
   - Proper serde derive attributes match protocol format
   - No intermediate extraction/transformation layer
   - Type system enforces correct message structure

## Recommendations

### Fix 1: Environment Variable Handling (HIGH PRIORITY)

**Option A: Preserve shell environment (like Zed)**

```rust
// In src-tauri/src/runtime/agents.rs
let mut cmd = Command::new(&command.path);
cmd.args(&command.args)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    // DON'T call .env_clear()

// Still add explicit overrides
if let Ok(base_url) = std::env::var("ANTHROPIC_BASE_URL") {
    cmd.env("ANTHROPIC_BASE_URL", base_url);
}
if let Ok(auth_token) = std::env::var("ANTHROPIC_AUTH_TOKEN") {
    cmd.env("ANTHROPIC_AUTH_TOKEN", auth_token);
}
```

**Option B: Build comprehensive environment**

```rust
// Collect parent env
let mut env: HashMap<String, String> = std::env::vars().collect();

// Add/override from preferences
if let Some(prefs) = load_preferences() {
    if let Some(api_key) = prefs.anthropic_api_key {
        env.insert("ANTHROPIC_API_KEY".into(), api_key);
    }
}

// Apply to command
cmd.env_clear();
cmd.envs(env);
```

**Trade-offs:**

- Option A: Simpler, matches Zed behavior, but exposes all env vars
- Option B: More control, can restrict sensitive vars, but more code

### Fix 2: Implement authMethods Support (MEDIUM PRIORITY)

**Step 1: Store authMethods**

```rust
// In src-tauri/src/protocols/acp/agent.rs
pub struct AcpAgent {
    auth_methods: Vec<AuthMethod>,  // Add field
    // ...
}

// In perform_acp_handshake()
self.auth_methods = result.auth_methods.unwrap_or_default();
```

**Step 2: Add authentication RPC**

```rust
impl AcpAgent {
    pub async fn authenticate(&mut self, method_id: &str) -> Result<()> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": Uuid::new_v4().to_string(),
            "method": "session/authenticate",
            "params": {
                "methodId": method_id
            }
        });

        self.send_request(request).await?;
        Ok(())
    }
}
```

**Step 3: Add UI for authentication**

- Emit `agent/auth_required` event with authMethods
- Show authentication modal in React
- Call `authenticate` command when user selects method

### Fix 3: Fix Session Update Parsing (HIGH PRIORITY)

**Apply the suggested fix to extract_session_update:**

```rust
fn extract_session_update(
    params: &serde_json::Value,
    fallback_session_id: &SessionId,
) -> (SessionId, serde_json::Value) {
    let session_id = params
        .get("sessionId")
        .or_else(|| params.get("session_id"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| fallback_session_id.clone());

    let update_value = if params.get("type").is_some() {
        // Already has type field
        params.clone()
    } else if let Some(su) = params.get("sessionUpdate") {
        match su {
            serde_json::Value::String(session_update_type) => {
                // sessionUpdate is type discriminator string
                let mut update_obj = serde_json::Map::new();
                update_obj.insert(
                    "type".to_string(),
                    serde_json::Value::String(session_update_type.to_string()),
                );
                // Copy other fields
                if let Some(obj) = params.as_object() {
                    for (key, value) in obj.iter() {
                        if key != "sessionUpdate" && key != "sessionId" && key != "session_id" {
                            update_obj.insert(key.clone(), value.clone());
                        }
                    }
                }
                serde_json::Value::Object(update_obj)
            }
            serde_json::Value::Object(_) => {
                // sessionUpdate is nested object - check if it has type
                if su.get("type").is_some() {
                    su.clone()  // Use the nested object directly
                } else {
                    params.clone()  // Fall through
                }
            }
            _ => params.clone(),
        }
    } else if params.get("availableCommands").is_some() {
        // Bare availableCommands - synthesize type
        serde_json::json!({
            "type": "availableCommandsUpdate",
            "availableCommands": params.get("availableCommands").cloned().unwrap_or(serde_json::Value::Null),
        })
    } else if let Some(update) = params.get("update") {
        update.clone()
    } else if let Some(notification) = params.get("notification") {
        notification.clone()
    } else if let Some(data) = params.get("data") {
        data.clone()
    } else {
        params.clone()
    };

    (session_id, update_value)
}
```

**File:** `src-tauri/src/protocols/acp/agent.rs:520-560`

### Fix Priority

1. **HIGHEST:** Fix session update parsing (Fix 3) - Immediate user-visible impact
2. **HIGH:** Fix environment handling (Fix 1) - Solves auth prompt issue
3. **MEDIUM:** Implement authMethods (Fix 2) - Enables proper auth UI flow

## Preventive Measures

1. **Protocol Conformance Testing:**
   - Add integration tests with real Claude Code adapter
   - Test all session update message types
   - Validate against ACP protocol specification

2. **Environment Audit:**
   - Document all required environment variables
   - Add debug logging for env passed to adapters
   - Test launch scenarios (terminal, Finder, etc.)

3. **Zed Compatibility:**
   - Regularly compare with Zed implementation
   - Track Zed's ACP-related commits
   - Maintain feature parity for core flows

4. **Error Visibility:**
   - Surface parse errors to UI (not just logs)
   - Add developer mode with verbose ACP logging
   - Include diagnostic info in error messages
