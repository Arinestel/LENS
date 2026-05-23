use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::core::core_logger::CoreLogger;
use crate::core::language_engine::LanguageEngine;
use crate::core::orchestrator::CorePipelineError;
use crate::core::real_language_config::RealLanguageConfig;
use crate::core::reasoning_contract::ReasoningResult;

const REAL_LANGUAGE_SYSTEM_PROMPT: &str = "Machine output only. Ответ должен начинаться ровно с: Краткий вывод: Любой текст перед \"Краткий вывод:\" запрещён. Не упоминай поля, шаблон, инструкцию, модель, ReasoningResult, input, output. Без markdown. Без JSON. Без новых фактов.";
const OLLAMA_PROVIDER_NAME: &str = "ollama";
const OLLAMA_GENERATE_PATH: &str = "/api/generate";
const REAL_LANGUAGE_GENERATE_STREAM: bool = false;
const REAL_LANGUAGE_GENERATE_THINK: bool = false;
const REAL_LANGUAGE_PREVIEW_NUM_PREDICT_STATUS: &str = "disabled";
const REAL_LANGUAGE_PREVIEW_TEMPERATURE: f64 = 0.2;

#[derive(Debug, Clone, PartialEq)]
pub struct RealLanguageEngine {
    config: RealLanguageConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealLanguageAdapterRequest {
    pub model_name: String,
    pub endpoint: String,
    pub timeout_ms: u64,
    pub system_prompt: String,
    pub user_prompt: String,
}

impl RealLanguageEngine {
    pub fn new(config: RealLanguageConfig) -> Self {
        Self { config }
    }

    pub fn prepare_request(
        &self,
        reasoning_result: &ReasoningResult,
        requested_language: &str,
    ) -> Result<RealLanguageAdapterRequest, CorePipelineError> {
        Ok(RealLanguageAdapterRequest {
            model_name: self.config.model_name.clone(),
            endpoint: self.config.endpoint.clone(),
            timeout_ms: self.config.timeout_ms,
            system_prompt: REAL_LANGUAGE_SYSTEM_PROMPT.to_string(),
            user_prompt: Self::build_user_prompt(reasoning_result, requested_language),
        })
    }

    pub fn generate_preview(
        &self,
        reasoning_result: &ReasoningResult,
        requested_language: &str,
    ) -> Result<String, CorePipelineError> {
        Self::validate_config(&self.config)?;
        let request = self.prepare_request(reasoning_result, requested_language)?;
        let endpoint = LanguageEndpoint::from_request(&request, &self.config)?;
        let body = build_generate_request(&request)?;
        let diagnostics = RealLanguageGenerateDiagnostics::new(&self.config, &request, &endpoint);

        CoreLogger::log("real_language_preview_test", &diagnostics.started_log());
        let response_text = post_json(&endpoint, body.as_bytes(), &diagnostics)?;
        let generated_text = parse_generate_response(&response_text)?;
        CoreLogger::log(
            "real_language_preview_test",
            "language generation preview response received",
        );

        Ok(generated_text)
    }

    fn validate_config(config: &RealLanguageConfig) -> Result<(), CorePipelineError> {
        if let Some(reason) = config.incomplete_reason() {
            return Err(Self::language_error(
                format!("Real language engine config is incomplete: {reason}.").as_str(),
            ));
        }

        Ok(())
    }

    fn build_user_prompt(reasoning_result: &ReasoningResult, requested_language: &str) -> String {
        format!(
            "language = {}\n\nDATA:\ntask = {}\nfacts = {}\nconclusions = {}\nassumptions = {}\nuncertainties = {}\nnext_actions = {}\nconfidence = {:.2}\n\nОтвет должен начинаться ровно с: Краткий вывод:\nЛюбой текст перед \"Краткий вывод:\" запрещён.\nНе упоминай поля, шаблон, инструкцию, модель, ReasoningResult, input, output.\n\nRETURN EXACTLY THIS FORM:\nКраткий вывод: ...\nФакты: ...\nВыводы: ...\nНеопределённости: ...\nСледующие действия: ...\nУверенность: ...",
            Self::safe_language_label(requested_language),
            reasoning_result.task,
            Self::format_items(
                reasoning_result
                    .facts
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_items(
                reasoning_result
                    .conclusions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_items(
                reasoning_result
                    .assumptions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_items(
                reasoning_result
                    .uncertainties
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_items(
                reasoning_result
                    .next_actions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            reasoning_result.confidence,
        )
    }

    fn safe_language_label(requested_language: &str) -> &str {
        let requested_language = requested_language.trim();
        if requested_language.is_empty() {
            "uk"
        } else {
            requested_language
        }
    }

    fn format_items(items: Vec<&str>) -> String {
        if items.is_empty() {
            return "- none".to_string();
        }

        let mut formatted = String::new();
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                formatted.push('\n');
            }
            formatted.push_str("- ");
            formatted.push_str(item);
        }
        formatted
    }

    fn language_error(message: &str) -> CorePipelineError {
        CorePipelineError {
            message: message.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LanguageEndpoint {
    host: String,
    port: u16,
    path: String,
}

impl LanguageEndpoint {
    fn from_request(
        request: &RealLanguageAdapterRequest,
        config: &RealLanguageConfig,
    ) -> Result<Self, CorePipelineError> {
        if config.provider_name.trim().to_lowercase() != OLLAMA_PROVIDER_NAME {
            return Err(real_language_error("provider_name must be ollama"));
        }

        let endpoint = request.endpoint.trim();
        let endpoint = endpoint.trim_end_matches('/');
        let authority = endpoint.strip_prefix("http://").ok_or_else(|| {
            real_language_error("real language endpoint must use local http base URL")
        })?;

        if authority.contains('/') {
            return Err(real_language_error(
                "real language endpoint must be a base URL without path",
            ));
        }

        let (host, port) = parse_authority(authority)?;
        if host != "localhost" && host != "127.0.0.1" {
            return Err(real_language_error("real language endpoint must be local"));
        }

        Ok(Self {
            host,
            port,
            path: OLLAMA_GENERATE_PATH.to_string(),
        })
    }
}

fn parse_authority(authority: &str) -> Result<(String, u16), CorePipelineError> {
    let (host, port_text) = authority
        .rsplit_once(':')
        .ok_or_else(|| real_language_error("real language endpoint must include port"))?;
    let host = host.trim();

    if host.is_empty() {
        return Err(real_language_error("real language endpoint host is empty"));
    }

    let port = port_text
        .parse::<u16>()
        .map_err(|_| real_language_error("real language endpoint port is invalid"))?;

    Ok((host.to_string(), port))
}

fn build_generate_request(
    request: &RealLanguageAdapterRequest,
) -> Result<String, CorePipelineError> {
    serde_json::to_string(&serde_json::json!({
        "model": request.model_name,
        "system": request.system_prompt,
        "prompt": request.user_prompt,
        "stream": REAL_LANGUAGE_GENERATE_STREAM,
        "think": REAL_LANGUAGE_GENERATE_THINK,
        "options": {
            "temperature": REAL_LANGUAGE_PREVIEW_TEMPERATURE,
        },
    }))
    .map_err(|error| {
        real_language_error(format!("failed to build real language request json: {error}").as_str())
    })
}

fn post_json(
    endpoint: &LanguageEndpoint,
    body: &[u8],
    diagnostics: &RealLanguageGenerateDiagnostics<'_>,
) -> Result<String, CorePipelineError> {
    let timeout = Duration::from_millis(diagnostics.timeout_ms);
    let connect_host = if endpoint.host == "localhost" {
        "127.0.0.1"
    } else {
        endpoint.host.as_str()
    };
    let address = format!("{}:{}", connect_host, endpoint.port);
    let socket_address = address
        .parse()
        .map_err(|_| real_language_error("real language endpoint socket address is invalid"))?;
    let mut stream = TcpStream::connect_timeout(&socket_address, timeout).map_err(|error| {
        real_language_transport_error(
            "connection failed",
            format!("failed to connect to real language backend: {error}").as_str(),
            &diagnostics,
        )
    })?;

    stream.set_read_timeout(Some(timeout)).map_err(|error| {
        real_language_transport_error(
            "transport failure",
            format!("failed to set real language read timeout: {error}").as_str(),
            &diagnostics,
        )
    })?;
    stream.set_write_timeout(Some(timeout)).map_err(|error| {
        real_language_transport_error(
            "transport failure",
            format!("failed to set real language write timeout: {error}").as_str(),
            &diagnostics,
        )
    })?;

    let http_request = format!(
        "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        endpoint.path,
        endpoint.host,
        endpoint.port,
        body.len()
    );

    stream.write_all(http_request.as_bytes()).map_err(|error| {
        real_language_transport_error(
            "transport failure",
            format!("failed to write real language request headers: {error}").as_str(),
            &diagnostics,
        )
    })?;
    stream.write_all(body).map_err(|error| {
        real_language_transport_error(
            "transport failure",
            format!("failed to write real language request body: {error}").as_str(),
            &diagnostics,
        )
    })?;

    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|error| {
        let reason = if error.kind() == ErrorKind::TimedOut {
            "read timeout"
        } else {
            "transport failure"
        };
        real_language_transport_error(
            reason,
            format!("failed to read real language response: {error}").as_str(),
            &diagnostics,
        )
    })?;

    let (headers, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| real_language_error("real language http response is malformed"))?;

    if !headers.starts_with("HTTP/1.1 200") && !headers.starts_with("HTTP/1.0 200") {
        return Err(real_language_error(
            "real language backend returned non-success http status",
        ));
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

fn decode_chunked_body(body: &str) -> Result<String, CorePipelineError> {
    let bytes = body.as_bytes();
    let mut cursor = 0usize;
    let mut decoded = Vec::new();

    loop {
        let line_end = find_crlf(bytes, cursor)
            .ok_or_else(|| real_language_error("real language chunked response is malformed"))?;
        let size_line = std::str::from_utf8(&bytes[cursor..line_end])
            .map_err(|_| real_language_error("real language chunked response size is not utf-8"))?;
        let size_text = size_line.split(';').next().unwrap_or("").trim();
        let chunk_size = usize::from_str_radix(size_text, 16).map_err(|_| {
            real_language_error("real language chunked response has invalid chunk size")
        })?;
        cursor = line_end + 2;

        if chunk_size == 0 {
            return String::from_utf8(decoded).map_err(|_| {
                real_language_error("real language chunked response body is not utf-8")
            });
        }

        if bytes.len() < cursor + chunk_size + 2 {
            return Err(real_language_error(
                "real language chunked response is incomplete",
            ));
        }

        decoded.extend_from_slice(&bytes[cursor..cursor + chunk_size]);
        cursor += chunk_size;

        if bytes.get(cursor..cursor + 2) != Some(b"\r\n") {
            return Err(real_language_error(
                "real language chunked response is malformed",
            ));
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

fn parse_generate_response(response_text: &str) -> Result<String, CorePipelineError> {
    let response_json =
        serde_json::from_str::<serde_json::Value>(response_text).map_err(|error| {
            real_language_error(
                format!("real language response is not valid json: {error}").as_str(),
            )
        })?;

    let response = response_json
        .get("response")
        .and_then(|value| value.as_str())
        .ok_or_else(|| real_language_error("real language response field is missing"))?
        .trim()
        .to_string();

    if response.is_empty() {
        return Err(real_language_error("real language response field is empty"));
    }

    Ok(response)
}

struct RealLanguageGenerateDiagnostics<'a> {
    provider_name: &'a str,
    model_name: &'a str,
    endpoint: &'a str,
    timeout_ms: u64,
    generate_path: &'a str,
    stream: bool,
    think: bool,
    num_predict: &'a str,
    temperature: f64,
    system_prompt_len: usize,
    user_prompt_len: usize,
    total_prompt_len: usize,
    uses_generate_path: bool,
}

impl<'a> RealLanguageGenerateDiagnostics<'a> {
    fn new(
        config: &'a RealLanguageConfig,
        request: &'a RealLanguageAdapterRequest,
        endpoint: &'a LanguageEndpoint,
    ) -> Self {
        let system_prompt_len = request.system_prompt.chars().count();
        let user_prompt_len = request.user_prompt.chars().count();

        Self {
            provider_name: config.provider_name.as_str(),
            model_name: request.model_name.as_str(),
            endpoint: request.endpoint.as_str(),
            timeout_ms: request.timeout_ms,
            generate_path: endpoint.path.as_str(),
            stream: REAL_LANGUAGE_GENERATE_STREAM,
            think: REAL_LANGUAGE_GENERATE_THINK,
            num_predict: REAL_LANGUAGE_PREVIEW_NUM_PREDICT_STATUS,
            temperature: REAL_LANGUAGE_PREVIEW_TEMPERATURE,
            system_prompt_len,
            user_prompt_len,
            total_prompt_len: system_prompt_len + user_prompt_len,
            uses_generate_path: endpoint.path == OLLAMA_GENERATE_PATH,
        }
    }

    fn started_log(&self) -> String {
        format!(
            "real_language_generate_request_started; {}",
            self.format_fields()
        )
    }

    fn format_fields(&self) -> String {
        format!(
            "provider_name: {}; model: {}; endpoint: {}; timeout_ms: {}; generate_path: {}; stream: {}; think: {}; num_predict: {}; temperature: {}; system_prompt_len: {}; user_prompt_len: {}; total_prompt_len: {}; uses_generate_path: {}",
            self.provider_name,
            self.model_name,
            self.endpoint,
            self.timeout_ms,
            self.generate_path,
            self.stream,
            self.think,
            self.num_predict,
            self.temperature,
            self.system_prompt_len,
            self.user_prompt_len,
            self.total_prompt_len,
            self.uses_generate_path
        )
    }
}

fn real_language_transport_error(
    reason: &str,
    detail: &str,
    diagnostics: &RealLanguageGenerateDiagnostics<'_>,
) -> CorePipelineError {
    real_language_error(
        format!(
            "real language transport error: {reason}; {}; detail: {detail}",
            diagnostics.format_fields()
        )
        .as_str(),
    )
}

fn real_language_error(message: &str) -> CorePipelineError {
    CorePipelineError {
        message: message.to_string(),
    }
}

impl LanguageEngine for RealLanguageEngine {
    fn format_response(
        &self,
        reasoning_result: &ReasoningResult,
        requested_language: &str,
    ) -> Result<String, CorePipelineError> {
        Self::validate_config(&self.config)?;
        let _prepared_request = self.prepare_request(reasoning_result, requested_language)?;

        Err(CorePipelineError {
            message: "Real language engine adapter is prepared, but language generation is not connected yet.".to_string(),
        })
    }
}
