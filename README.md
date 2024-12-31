# Envoy Custom Metrics via WASM (Proof of Concept)

## Building

```bash
cargo build --target wasm32-wasip1 --release
```

The wasm binary will be output to `./target/wasm32-wasip1/release/my_envoy_wasm_filter.wasm`

To build an image with the file:

```bash
docker build -t quay.io/dmartin/my-wasm:latest .
```

To push to the image registry:

```bash
docker push quay.io/dmartin/my-wasm:latest
```

## Deploying to a local cluster

Bring up a local kind cluster with envoy proxy (Istio):

```bash
./local_setup.sh
```

Create the httproute, service and other resources to proxy requests on to an external service:

```bash
./local_service.sh
```

## Testing the example endpoint

Make a request to the service:

```bash
export GATEWAY_IP=$(kubectl get gateway openai-gateway -o jsonpath='{.status.addresses[0].value}')

curl -v -k --resolve localopenai.example:443:$GATEWAY_IP -H "Content-Type: application/json" -H "Authorization: Bearer sk-0"  "https://localopenai.example/v1/chat/completions" -d '{
    "model": "gpt-3.5-turbo",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

You should get a response that looks like this:

```json
{"id":"chatcmpl-123","object":"chat.completion","created":1677652288,"model":"gpt-3.5-turbo","usage":{"prompt_tokens":9,"completion_tokens":12,"total_tokens":21},"choices":[{"index":0,"message":{"role":"assistant","content":"this is a short sentence.","name":null},"delta":[null],"finish_reason":"stop"}]}
```

## Deploying the WASM Plugin

```bash
kubectl apply -f - <<EOF
apiVersion: extensions.istio.io/v1alpha1
kind: WasmPlugin
metadata:
  name: usage-filter
  namespace: default
spec:
  selector:
    matchLabels:
      istio.io/gateway-name: openai-gateway
  url: "oci://quay.io/dmartin/my-wasm:latest"
  imagePullPolicy: Always
  phase: STATS
EOF
```

Send an example request again to generate some metrics:

```bash
curl -v -k --resolve localopenai.example:443:$GATEWAY_IP -H "Content-Type: application/json" -H "Authorization: Bearer sk-0"  "https://localopenai.example/v1/chat/completions" -d '{
    "model": "gpt-3.5-turbo",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

Verify the metrics are included in the metrics endpoint

```bash
kubectl port-forward openai-gateway-istio-5fbb975c6b-6gk2b 15000:15000 &
curl http://localhost:15000/stats/prometheus
```

## Troubleshooting

The Gateway listener has a hostname of `localopenai.example`.
The HTTPRoute is configured with the backend of the the `openai-mock` Service.
That Service has an ExternalName of `api.openai-mock.com`, connecting to the mock OpenAI service at <https://api.openai-mock.com/#chat>.

To verify the upstream service is working, send a request to it directly:

```bash
curl -v https://api.openai-mock.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-0" \
  -d '{
    "model": "gpt-3.5-turbo",
    "messages": [{"role": "system", "content": "You are a helpful assistant."}, {"role": "user", "content": "Hello!"}]
  }'
```

You should get a response like this:

```json
{"id":"chatcmpl-123","object":"chat.completion","created":1677652288,"model":"gpt-3.5-turbo","usage":{"prompt_tokens":9,"completion_tokens":12,"total_tokens":21},"choices":[{"index":0,"message":{"role":"assistant","content":"this is a short sentence.","name":null},"delta":[null],"finish_reason":"stop"}]}
```

If that request fails, there may be a problem with that site.
If the request is successful, but requests to the local gateway fail, there may be a problem with the configuration of 1 or more resources,
or a networking problem locally.

Verify there is a value for the gateway IP:

```bash
export GATEWAY_IP=$(kubectl get gateway openai-gateway -o jsonpath='{.status.addresses[0].value}')
echo $GATEWAY_IP
```

If that is successful, verify you get some sort of response from the gateway:

```bash
curl -v -k https://$GATEWAY_IP
```