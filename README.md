# Envoy Custom Metrics via WASM (Proof of Concept)

The scripts and rust code in this project show how to intercept the response body from OpenAI http requests and parse out `usage` information, transforming it into metrics.
Envoy proxy is used to do this, and Istio with a WasmPlugin resource.
The scripts show how to bring up a local kind cluster with all the necessary components installed.
The steps below include commands to deploy the WasmPlugin and make http requests to an OpenAI endpoint.

A Gateway and HTTPRoute are used, with a backend proxying to the mock OpenAI service at https://api.openai-mock.com/.
As requests are made, metrics are accumulated and exposed on the `/stats/prometheus` endpoint of the gateway.
Prometheus is deployed in the cluster as well, and configured to scrape these metrics.
Instructions are included to call the endpoint directly and view the metrics in Grafana (also deployed).

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

**NOTE:** MetalLB is installed in the local kind cluster, with a hardcoded network of 10.89.0.16/28. You may need to change this to be a subnet of the `kind` docker/podman network in your local setup. You can check the network details with `docker network inspect kind` or `podman network inspect kind`.

Create the httproute, service and other resources to proxy requests on to the external OpenAI mock service:

```bash
./local_service.sh
```

## Testing the example endpoint

Make a request to the service:

```bash
export GATEWAY_IP=$(kubectl get gateway openai-gateway -o jsonpath='{.status.addresses[0].value}')

curl -v --resolve openai.dm.hcpapps.net:80:$GATEWAY_IP -H "Content-Type: application/json" -H "Authorization: Bearer sk-0"  "http://openai.dm.hcpapps.net/v1/chat/completions" -d '{
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
  targetRef:
    group: gateway.networking.k8s.io
    kind: Gateway
    name: openai-gateway
  url: "oci://quay.io/dmartin/my-wasm:latest"
  imagePullPolicy: Always
EOF
```

Enable wasm debug logging:

```bash
kubectl port-forward $(kubectl get po -l gateway.networking.k8s.io/gateway-name=openai-gateway -o name) 15000:15000 &
curl 127.0.0.1:15000/logging?wasm=debug -XPOST
```

Send some example requests to generate some metrics:

```bash
while true; do curl -v --connect-timeout 10 --resolve openai.dm.hcpapps.net:80:$GATEWAY_IP -H "Content-Type: application/json" -H "Authorization: Bearer sk-0"  "http://openai.dm.hcpapps.net/v1/chat/completions" -d '{
    "model": "gpt-3.5-turbo",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Hello!"}
    ]
  }' && sleep 30; done
```

Verify the metrics are included in the metrics endpoint

```bash
kubectl port-forward $(kubectl get po -l gateway.networking.k8s.io/gateway-name=openai-gateway -o name) 15000:15000 &
curl -s http://localhost:15000/stats/prometheus | grep my_wasm
```

## Visualing metrics

Port forward to grafana:

```bash
kubectl port-forward svc/kube-prometheus-grafana 3000:80
```

Then access it at http://127.0.0.1:3000/ with user/pass of `admin` `prom-operator`.
You can either execute some queries like these:

```promql
sum(rate(my_wasm_completion_tokens[5m]))
sum(rate(my_wasm_prompt_tokens[5m]))
sum(rate(my_wasm_total_tokens[5m]))
```

Or import the example dashboard json from `./dash.json`.
It should look something like this:

![metrics_dashboard](./dash.png)

## Troubleshooting

The Gateway listener has a hostname of `openai.dm.hcpapps.net`.
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

Check istio logs for any errors:

```bash
kubectl logs -n istio-system -l app=istiod -f
kubectl logs -l gateway.networking.k8s.io/gateway-name=openai-gateway -f
```

Check the envoy proxy config dump for the wasm filter:

```bash
kubectl port-forward $(kubectl get po -l gateway.networking.k8s.io/gateway-name=openai-gateway -o name) 15000:15000 &
curl -s http://localhost:15000/config_dump | grep -A 50 "usage-filter"
```
