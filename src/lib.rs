use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use proxy_wasm::hostcalls::{define_metric, increment_metric};
use serde::Deserialize;
use serde_json::Value;


#[derive(Deserialize, Debug)] // Derive Debug
struct Usage {
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
}

// Root context holds metric IDs
struct Root {
    metric_prompt_tokens: u32,
    metric_completion_tokens: u32,
    metric_total_tokens: u32,
}

impl Context for Root {}

impl RootContext for Root {
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
        Some(Box::new(MyHttpFilter {
            response_body: Vec::new(),
            metric_prompt_tokens: self.metric_prompt_tokens,
            metric_completion_tokens: self.metric_completion_tokens,
            metric_total_tokens: self.metric_total_tokens,
        }))
    }
}

// Per-request context
struct MyHttpFilter {
    response_body: Vec<u8>,
    metric_prompt_tokens: u32,
    metric_completion_tokens: u32,
    metric_total_tokens: u32,
}

impl Context for MyHttpFilter {}

impl HttpContext for MyHttpFilter {
    fn on_http_request_headers(&mut self, _: usize, _: bool) -> Action {
        let _ = proxy_wasm::hostcalls::log(LogLevel::Info, "Wasm filter: Got request headers!");

        Action::Continue
    }

    fn on_http_response_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        let _ = proxy_wasm::hostcalls::log(LogLevel::Info, "Wasm filter: Got response headers!");

        Action::Continue
    }
    
    fn on_http_response_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        let _ = proxy_wasm::hostcalls::log(LogLevel::Info, "Wasm filter: on_http_response_body called!");
        if let Some(chunk) = self.get_http_response_body(0, body_size) {
            let _ = proxy_wasm::hostcalls::log(LogLevel::Info, &format!("Wasm filter: Got response body chunk! size: {:?}", &chunk.len()));
            self.response_body.extend_from_slice(&chunk);
        }
        if end_of_stream {
            let _ = proxy_wasm::hostcalls::log(LogLevel::Info, &format!("Wasm filter: Got response body! body_size: {:?}", body_size));
            // if let Some(body_bytes) = self.get_http_response_body(0, body_size) {
            //     let _ = proxy_wasm::hostcalls::log(LogLevel::Info, &format!("Wasm filter: Got body bytes! size: {:?}", body_size));
                if let Ok(body_str) = std::str::from_utf8(&self.response_body) {
                    let _ = proxy_wasm::hostcalls::log(LogLevel::Info, &format!("Wasm filter: Got body string! str: {:?}", body_str));
                    if let Ok(json_val) = serde_json::from_str::<Value>(body_str) {
                        let _ = proxy_wasm::hostcalls::log(LogLevel::Info, &format!("Wasm filter: Got json value! json_val: {:?}", json_val));
                        if let Some(usage_val) = json_val.get("usage") {
                            let _ = proxy_wasm::hostcalls::log(LogLevel::Info, &format!("Wasm filter: Got usage value! usage_val: {:?}", usage_val));
                            if let Ok(parsed) = serde_json::from_value::<Usage>(usage_val.clone()) {
                                let _ = proxy_wasm::hostcalls::log(LogLevel::Info, &format!("Found usage: {:?}", parsed));

                                // Increments must be i64 (not u64)
                                let _ = increment_metric(self.metric_prompt_tokens, parsed.prompt_tokens);
                                let _ = increment_metric(
                                    self.metric_completion_tokens,
                                    parsed.completion_tokens,
                                );
                                let _ = increment_metric(self.metric_total_tokens, parsed.total_tokens);

                            }
                        }
                    }
                }
            // }
        }
        Action::Continue
    }
}

proxy_wasm::main!({
    proxy_wasm::set_root_context(|_| {
        Box::new(Root {
            metric_prompt_tokens: 0,
            metric_completion_tokens: 0,
            metric_total_tokens: 0,
        })
    });
});
