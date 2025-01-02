#!/bin/bash

set -ex

kubectl apply -f - <<EOF
apiVersion: gateway.networking.k8s.io/v1beta1
kind: Gateway
metadata:
  name: openai-gateway
  namespace: default
spec:
  gatewayClassName: istio
  listeners:
    - name: http
      port: 80
      protocol: HTTP
      hostname: 'openai.dm.hcpapps.net'
      allowedRoutes:
        namespaces:
          from: Same
---
apiVersion: gateway.networking.k8s.io/v1beta1
kind: HTTPRoute
metadata:
  name: openai-httproute
  namespace: default
spec:
  parentRefs:
    - name: openai-gateway
  hostnames:
    - "openai.dm.hcpapps.net"
  rules:
  - backendRefs:
    - name: toystore
      port: 80
EOF

kubectl apply -f https://raw.githubusercontent.com/Kuadrant/Kuadrant-operator/main/examples/toystore/toystore.yaml -n default

kubectl apply -f - <<EOF
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: issuer
spec:
  selfSigned: {}
EOF

kubectl apply -f - <<EOF
apiVersion: kuadrant.io/v1
kind: TLSPolicy
metadata:
  name: tls
  namespace: default
spec:
  targetRef:
    name: openai-gateway
    group: gateway.networking.k8s.io
    kind: Gateway
  issuerRef:
    group: cert-manager.io
    kind: ClusterIssuer
    name: issuer
EOF
