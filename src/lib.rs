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

proxy_wasm::main!({
    proxy_wasm::set_http_context(|_context_id, _root_context_id| -> Box<dyn HttpContext> {
        Box::new(MyHttpFilter)
    });
});

struct MyHttpFilter;

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
        if end_of_stream {
            let _ = proxy_wasm::hostcalls::log(LogLevel::Info, "Wasm filter: Got response body!");
            if let Some(body_bytes) = self.get_http_response_body(0, body_size) {
                if let Ok(body_str) = std::str::from_utf8(&body_bytes) {
                    if let Ok(json_val) = serde_json::from_str::<Value>(body_str) {
                        if let Some(usage_val) = json_val.get("usage") {
                            if let Ok(parsed) = serde_json::from_value::<Usage>(usage_val.clone()) {
                                let _ = proxy_wasm::hostcalls::log(LogLevel::Info, &format!("Found usage: {:?}", parsed));
                            }
                        }
                    }
                }
            }
        }
        Action::Continue
    }
}
