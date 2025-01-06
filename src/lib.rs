use proxy_wasm::hostcalls::{define_metric, increment_metric, log};
use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Debug)] // Derive Debug
struct Usage {
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
}

// Root context holds metric IDs
struct UsageRoot {
    metric_prompt_tokens: u32,
    metric_completion_tokens: u32,
    metric_total_tokens: u32,
}

impl Context for UsageRoot {}

impl RootContext for UsageRoot {
    // Called when VM starts
    fn on_vm_start(&mut self, _vm_configuration_size: usize) -> bool {
        // Register counters so they appear in Envoy’s /stats/prometheus
        self.metric_prompt_tokens =
            define_metric(MetricType::Counter, "my_wasm_prompt_tokens").unwrap();
        self.metric_completion_tokens =
            define_metric(MetricType::Counter, "my_wasm_completion_tokens").unwrap();
        self.metric_total_tokens =
            define_metric(MetricType::Counter, "my_wasm_total_tokens").unwrap();
        true
    }

    // **Tell Envoy this is an HTTP context**
    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }

    // Called to create an HttpContext for each HTTP stream
    fn create_http_context(&self, _context_id: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(UsageHttpFilter {
            response_body: Vec::new(),
            metric_prompt_tokens: self.metric_prompt_tokens,
            metric_completion_tokens: self.metric_completion_tokens,
            metric_total_tokens: self.metric_total_tokens,
        }))
    }
}

// Per-request context
struct UsageHttpFilter {
    response_body: Vec<u8>,
    metric_prompt_tokens: u32,
    metric_completion_tokens: u32,
    metric_total_tokens: u32,
}

impl Context for UsageHttpFilter {}

impl HttpContext for UsageHttpFilter {
    fn on_http_response_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        let _ = log(LogLevel::Debug, "on_http_response_body called");
        if let Some(chunk) = self.get_http_response_body(0, body_size) {
            let _ = log(
                LogLevel::Debug,
                &format!("Appending {} bytes to buffer", &chunk.len()),
            );
            self.response_body.extend_from_slice(&chunk);
        }
        if end_of_stream {
            let _ = log(
                LogLevel::Debug,
                &format!("Wasm filter: Got response body! body_size: {:?}", body_size),
            );
            if let Some(usage) = parse_usage(&self.response_body) {
                let _ = log(LogLevel::Debug, &format!("Parsed usage: {:?}", usage));

                // Increments must be i64 (not u64)
                let _ = increment_metric(self.metric_prompt_tokens, usage.prompt_tokens);
                let _ = increment_metric(self.metric_completion_tokens, usage.completion_tokens);
                let _ = increment_metric(self.metric_total_tokens, usage.total_tokens);
            }
        }
        Action::Continue
    }
}

// Helper function for usage parsing
fn parse_usage(body_bytes: &[u8]) -> Option<Usage> {
    let body_str = std::str::from_utf8(body_bytes).ok()?;
    let json_val = serde_json::from_str::<Value>(body_str).ok()?;
    let usage_val = json_val.get("usage")?;
    serde_json::from_value::<Usage>(usage_val.clone()).ok()
}

proxy_wasm::main!({
    proxy_wasm::set_root_context(|_| {
        Box::new(UsageRoot {
            metric_prompt_tokens: 0,
            metric_completion_tokens: 0,
            metric_total_tokens: 0,
        })
    });
});
