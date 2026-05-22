use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::Value;

use crate::core::core_logger::CoreLogger;
use crate::core::orchestrator::OrchestratorRequest;
use crate::core::real_reasoning_config::RealReasoningConfig;
use crate::core::reasoning_errors::ReasoningEngineError;
use crate::core::reasoning_response_mapping::RawReasoningBackendResponse;

const OLLAMA_PROVIDER_NAME: &str = "ollama";
const OLLAMA_GENERATE_PATH: &str = "/api/generate";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaReasoningClient {
    config: RealReasoningConfig,
}

impl OllamaReasoningClient {
    pub fn new(config: RealReasoningConfig) -> Self {
        Self { config }
    }

    pub fn generate(
        &self,
        request: &OrchestratorRequest,
    ) -> Result<RawReasoningBackendResponse, ReasoningEngineError> {
        let endpoint = OllamaEndpoint::from_config(&self.config)?;
        let body = self.build_generate_request(request)?;

        CoreLogger::log("ollama_reasoning", "transport request started");

        let response_text = post_json(&endpoint, body.as_bytes(), self.config.timeout_ms)?;
        let raw_response = parse_ollama_response(&response_text)?;

        CoreLogger::log("ollama_reasoning", "transport response received");

        Ok(raw_response)
    }

    fn build_generate_request(
        &self,
        request: &OrchestratorRequest,
    ) -> Result<String, ReasoningEngineError> {
        let model_name = self.config.model_name.trim();

        if model_name.is_empty() || model_name == "real-reasoning-placeholder" {
            return Err(config_error("ollama model_name is not configured"));
        }

        serde_json::to_string(&serde_json::json!({
            "model": model_name,
            "system": reasoning_system_prompt(),
            "prompt": reasoning_user_prompt(&request.input.query.text),
            "stream": false,
            "think": false,
            "format": reasoning_response_schema(),
        }))
        .map_err(|error| ReasoningEngineError::InvalidModelOutput {
            message: format!("failed to build ollama request json: {error}"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OllamaEndpoint {
    host: String,
    port: u16,
    path: String,
}

impl OllamaEndpoint {
    fn from_config(config: &RealReasoningConfig) -> Result<Self, ReasoningEngineError> {
        if config.provider_name.trim().to_lowercase() != OLLAMA_PROVIDER_NAME {
            return Err(config_error("provider_name must be ollama"));
        }

        if config.timeout_ms == 0 {
            return Err(config_error("timeout_ms is zero"));
        }

        let endpoint = config.endpoint.trim();

        if endpoint.is_empty() || endpoint == "not-configured" {
            return Err(config_error("ollama endpoint is not configured"));
        }

        let endpoint = endpoint.trim_end_matches('/');
        let authority = endpoint
            .strip_prefix("http://")
            .ok_or_else(|| config_error("ollama endpoint must use local http base URL"))?;

        if authority.contains('/') {
            return Err(config_error(
                "ollama endpoint must be a base URL without path",
            ));
        }

        let (host, port) = parse_authority(authority)?;

        if host != "localhost" && host != "127.0.0.1" {
            return Err(config_error("ollama endpoint must be local"));
        }

        Ok(Self {
            host,
            port,
            path: OLLAMA_GENERATE_PATH.to_string(),
        })
    }
}

fn parse_authority(authority: &str) -> Result<(String, u16), ReasoningEngineError> {
    let (host, port_text) = authority
        .rsplit_once(':')
        .ok_or_else(|| config_error("ollama endpoint must include port"))?;
    let host = host.trim();

    if host.is_empty() {
        return Err(config_error("ollama endpoint host is empty"));
    }

    let port = port_text
        .parse::<u16>()
        .map_err(|_| config_error("ollama endpoint port is invalid"))?;

    Ok((host.to_string(), port))
}

fn post_json(
    endpoint: &OllamaEndpoint,
    body: &[u8],
    timeout_ms: u64,
) -> Result<String, ReasoningEngineError> {
    let timeout = Duration::from_millis(timeout_ms);
    let connect_host = if endpoint.host == "localhost" {
        "127.0.0.1"
    } else {
        endpoint.host.as_str()
    };
    let address = format!("{}:{}", connect_host, endpoint.port);
    let socket_address = address
        .parse()
        .map_err(|_| config_error("ollama endpoint socket address is invalid"))?;
    let mut stream = TcpStream::connect_timeout(&socket_address, timeout).map_err(|error| {
        CoreLogger::log("ollama_reasoning", "transport request failed");
        if error.kind() == std::io::ErrorKind::TimedOut {
            timeout_error("ollama connection timed out")
        } else {
            transport_error(format!("failed to connect to ollama: {error}").as_str())
        }
    })?;

    stream.set_read_timeout(Some(timeout)).map_err(|error| {
        transport_error(format!("failed to set read timeout: {error}").as_str())
    })?;
    stream.set_write_timeout(Some(timeout)).map_err(|error| {
        transport_error(format!("failed to set write timeout: {error}").as_str())
    })?;

    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        endpoint.path,
        endpoint.host,
        endpoint.port,
        body.len()
    );

    stream.write_all(request.as_bytes()).map_err(|error| {
        CoreLogger::log("ollama_reasoning", "transport request failed");
        transport_error(format!("failed to write ollama request headers: {error}").as_str())
    })?;
    stream.write_all(body).map_err(|error| {
        CoreLogger::log("ollama_reasoning", "transport request failed");
        transport_error(format!("failed to write ollama request body: {error}").as_str())
    })?;

    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|error| {
        CoreLogger::log("ollama_reasoning", "transport request failed");
        if error.kind() == std::io::ErrorKind::TimedOut {
            timeout_error("ollama response timed out")
        } else {
            transport_error(format!("failed to read ollama response: {error}").as_str())
        }
    })?;

    let (headers, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| transport_error("ollama http response is malformed"))?;

    if !headers.starts_with("HTTP/1.1 200") && !headers.starts_with("HTTP/1.0 200") {
        return Err(transport_error("ollama returned non-success http status"));
    }

    if headers
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        decode_chunked_body(body)
    } else {
        Ok(body.to_string())
    }
}

fn decode_chunked_body(body: &str) -> Result<String, ReasoningEngineError> {
    let bytes = body.as_bytes();
    let mut cursor = 0usize;
    let mut decoded = Vec::new();

    loop {
        let line_end = find_crlf(bytes, cursor)
            .ok_or_else(|| transport_error("ollama chunked response is malformed"))?;
        let size_line = std::str::from_utf8(&bytes[cursor..line_end])
            .map_err(|_| transport_error("ollama chunked response size is not utf-8"))?;
        let size_text = size_line.split(';').next().unwrap_or("").trim();
        let chunk_size = usize::from_str_radix(size_text, 16)
            .map_err(|_| transport_error("ollama chunked response has invalid chunk size"))?;
        cursor = line_end + 2;

        if chunk_size == 0 {
            return String::from_utf8(decoded)
                .map_err(|_| transport_error("ollama chunked response body is not utf-8"));
        }

        if bytes.len() < cursor + chunk_size + 2 {
            return Err(transport_error("ollama chunked response is incomplete"));
        }

        decoded.extend_from_slice(&bytes[cursor..cursor + chunk_size]);
        cursor += chunk_size;

        if bytes.get(cursor..cursor + 2) != Some(b"\r\n") {
            return Err(transport_error("ollama chunked response is malformed"));
        }

        cursor += 2;
    }
}

fn find_crlf(bytes: &[u8], start: usize) -> Option<usize> {
    bytes
        .get(start..)?
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|position| start + position)
}

fn parse_ollama_response(
    response_text: &str,
) -> Result<RawReasoningBackendResponse, ReasoningEngineError> {
    log_diagnostic("ollama raw http response body exact", response_text);

    let response_json = serde_json::from_str::<Value>(response_text).map_err(|error| {
        CoreLogger::log(
            "ollama_reasoning",
            format!(
                "ollama http response body json parse failed: {error}; exact body: {}",
                exact_diagnostic_text(response_text)
            )
            .as_str(),
        );

        ReasoningEngineError::InvalidModelOutput {
            message: format!(
                "ollama http response body is not valid json: {error}; body preview: {}",
                diagnostic_preview(response_text)
            ),
        }
    })?;
    log_json_diagnostic("ollama parsed envelope exact", &response_json);

    let response_value =
        optional_ollama_model_output_field(&response_json, "response", response_text)?;
    let source_field = "response";
    let response_value = match response_value {
        Some(response) if !response.is_empty() => response,
        Some(_) => {
            CoreLogger::log(
                "ollama_reasoning",
                "ollama response field is empty; thinking fallback disabled",
            );
            let message = response_field_diagnostic_message(
                "ollama response field is empty",
                "response present but empty; thinking fallback disabled",
                source_field,
                response_text,
                &response_json,
            );
            return Err(invalid_output(&message));
        }
        None => {
            CoreLogger::log(
                "ollama_reasoning",
                "ollama response field is missing; thinking fallback disabled",
            );
            let message = response_field_diagnostic_message(
                "ollama response field is missing",
                "response missing; thinking fallback disabled",
                source_field,
                response_text,
                &response_json,
            );
            return Err(invalid_output(&message));
        }
    };

    CoreLogger::log(
        "ollama_reasoning",
        format!("ollama model output source field: {source_field}").as_str(),
    );
    CoreLogger::log(
        "ollama_reasoning",
        "response-only path active; structured payload came from response field",
    );
    log_diagnostic("ollama extracted model output exact", response_value);

    let structured_output = extract_structured_json_object(
        response_value,
        source_field,
        response_text,
        &response_json,
    )?;
    log_diagnostic(
        "ollama structured json parse input exact",
        structured_output,
    );
    let reasoning_json = serde_json::from_str::<Value>(structured_output).map_err(|error| {
        ReasoningEngineError::InvalidModelOutput {
            message: format!(
                "ollama {source_field} field is not structured json: {error}; model output preview: {}",
                diagnostic_preview(response_value)
            ),
        }
    })?;

    Ok(RawReasoningBackendResponse {
        task: optional_string(&reasoning_json, "task")?,
        facts: optional_string_array(&reasoning_json, "facts")?,
        conclusions: optional_string_array(&reasoning_json, "conclusions")?,
        assumptions: optional_string_array(&reasoning_json, "assumptions")?,
        uncertainties: optional_string_array(&reasoning_json, "uncertainties")?,
        next_actions: optional_string_array(&reasoning_json, "next_actions")?,
        confidence: optional_f32(&reasoning_json, "confidence")?,
    })
}

fn optional_ollama_model_output_field<'a>(
    response_json: &'a Value,
    field_name: &'static str,
    response_text: &str,
) -> Result<Option<&'a str>, ReasoningEngineError> {
    match response_json.get(field_name) {
        Some(Value::String(text)) => Ok(Some(text.as_str())),
        Some(_) => {
            let message = response_field_diagnostic_message(
                format!("ollama {field_name} field is not a string").as_str(),
                "present but not string",
                field_name,
                response_text,
                &response_json,
            );
            Err(invalid_output(&message))
        }
        None => Ok(None),
    }
}

fn optional_string(
    value: &Value,
    field_name: &str,
) -> Result<Option<String>, ReasoningEngineError> {
    match value.get(field_name) {
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(invalid_output(
            format!("{field_name} must be a string").as_str(),
        )),
        None => Ok(None),
    }
}

fn optional_string_array(
    value: &Value,
    field_name: &str,
) -> Result<Option<Vec<String>>, ReasoningEngineError> {
    match value.get(field_name) {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str().map(str::to_string).ok_or_else(|| {
                    invalid_output(format!("{field_name} must contain only strings").as_str())
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(invalid_output(
            format!("{field_name} must be an array").as_str(),
        )),
        None => Ok(None),
    }
}

fn optional_f32(value: &Value, field_name: &str) -> Result<Option<f32>, ReasoningEngineError> {
    match value.get(field_name) {
        Some(Value::Number(number)) => number
            .as_f64()
            .map(|number| Some(number as f32))
            .ok_or_else(|| invalid_output(format!("{field_name} must be a number").as_str())),
        Some(_) => Err(invalid_output(
            format!("{field_name} must be a number").as_str(),
        )),
        None => Ok(None),
    }
}

fn reasoning_system_prompt() -> &'static str {
    "You are the LENS reasoning model. Return the final structured ReasoningResult only as one valid JSON object in the final answer, which Ollama exposes in the `response` field. Do not put the ReasoningResult in `thinking`. Do not duplicate the ReasoningResult in multiple fields. Do not return markdown. Do not use code fences. Do not add prose, labels, prefixes, suffixes, explanations, or comments before or after the JSON. The JSON object must match the provided schema and contain exactly these fields: task, facts, conclusions, assumptions, uncertainties, next_actions, confidence. If you cannot be certain, still return valid JSON and put uncertainty details in the uncertainties array."
}

fn reasoning_user_prompt(user_task: &str) -> String {
    format!(
        "Analyze the user task and return only the final structured ReasoningResult JSON object.\n\nRequired output contract:\n- The entire final answer must be one JSON object.\n- The JSON must be in the final response, not in thinking.\n- Do not place, copy, or duplicate the structured result in thinking.\n- No markdown.\n- No code fences.\n- No prose before or after the JSON.\n- No explanations, labels, prefixes, suffixes, or comments outside the JSON.\n- Use exactly these fields: task, facts, conclusions, assumptions, uncertainties, next_actions, confidence.\n\nUser task:\n{user_task}"
    )
}

fn extract_structured_json_object<'a>(
    model_output: &'a str,
    source_field: &str,
    response_text: &str,
    response_json: &Value,
) -> Result<&'a str, ReasoningEngineError> {
    let trimmed = model_output.trim();

    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Ok(trimmed);
    }

    let message = response_field_diagnostic_message(
        format!(
            "ollama model output is not JSON; model output preview: {}",
            diagnostic_preview(model_output)
        )
        .as_str(),
        "present but not JSON",
        source_field,
        response_text,
        response_json,
    );
    Err(invalid_output(&message))
}

fn log_diagnostic(label: &str, text: &str) {
    CoreLogger::log(
        "ollama_reasoning",
        format!("{label}: {}", exact_diagnostic_text(text)).as_str(),
    );
}

fn log_json_diagnostic(label: &str, value: &Value) {
    let text = serde_json::to_string(value)
        .unwrap_or_else(|error| format!("<failed to serialize json diagnostic: {error}>"));
    log_diagnostic(label, &text);
}

fn response_field_diagnostic_message(
    summary: &str,
    field_state: &str,
    source_field: &str,
    response_text: &str,
    response_json: &Value,
) -> String {
    let envelope = serde_json::to_string(response_json)
        .unwrap_or_else(|error| format!("<failed to serialize parsed envelope: {error}>"));

    format!(
        "{summary}; model output source field: {source_field}; field state: {field_state}; raw Ollama response body: {}; parsed Ollama envelope: {}",
        exact_diagnostic_text(response_text),
        exact_diagnostic_text(&envelope)
    )
}

fn exact_diagnostic_text(text: &str) -> String {
    if text.is_empty() {
        "<empty>".to_string()
    } else {
        format!("{text:?}")
    }
}

fn diagnostic_preview(text: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 600;
    if text.is_empty() {
        return "<empty>".to_string();
    }

    let sanitized = text.replace("\r\n", "\\n").replace('\n', "\\n");
    let mut preview = sanitized
        .chars()
        .take(MAX_PREVIEW_CHARS)
        .collect::<String>();

    if sanitized.chars().count() > MAX_PREVIEW_CHARS {
        preview.push_str("...");
    }

    preview
}

fn reasoning_response_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "task": { "type": "string" },
            "facts": { "type": "array", "items": { "type": "string" } },
            "conclusions": { "type": "array", "items": { "type": "string" } },
            "assumptions": { "type": "array", "items": { "type": "string" } },
            "uncertainties": { "type": "array", "items": { "type": "string" } },
            "next_actions": { "type": "array", "items": { "type": "string" } },
            "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
        },
        "required": [
            "task",
            "facts",
            "conclusions",
            "assumptions",
            "uncertainties",
            "next_actions",
            "confidence"
        ],
        "additionalProperties": false
    })
}

fn config_error(message: &str) -> ReasoningEngineError {
    ReasoningEngineError::ConfigError {
        message: message.to_string(),
    }
}

fn timeout_error(message: &str) -> ReasoningEngineError {
    ReasoningEngineError::Timeout {
        message: message.to_string(),
    }
}

fn transport_error(message: &str) -> ReasoningEngineError {
    ReasoningEngineError::TransportError {
        message: message.to_string(),
    }
}

fn invalid_output(message: &str) -> ReasoningEngineError {
    ReasoningEngineError::InvalidModelOutput {
        message: message.to_string(),
    }
}
