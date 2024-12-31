FROM scratch
COPY ./target/wasm32-wasip1/release/my_envoy_wasm_filter.wasm /plugin.wasm
