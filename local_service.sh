#!/bin/bash

set -ex

kubectl apply -f - <<EOF
apiVersion: networking.istio.io/v1beta1
kind: DestinationRule
metadata:
  name: openai-dr
  namespace: default
spec:
  host: api.openai-mock.com
  trafficPolicy:
    tls:
      mode: SIMPLE
      sni: api.openai-mock.com
---
apiVersion: networking.istio.io/v1beta1
kind: ServiceEntry
metadata:
  name: openai-mock-se
  namespace: default
spec:
  hosts:
  - api.openai-mock.com
  location: MESH_EXTERNAL
  ports:
  - number: 80
    name: http
    protocol: HTTP
  - number: 443
    name: https
    protocol: TLS
  resolution: DNS
---
apiVersion: gateway.networking.k8s.io/v1beta1
kind: Gateway
metadata:
  name: openai-gateway
  namespace: default
spec:
  gatewayClassName: istio
  listeners:
    - name: https
      port: 443
      protocol: HTTPS
      hostname: 'localopenai.example'
      allowedRoutes:
        namespaces:
          from: Same
      tls:
        mode: Terminate
        certificateRefs:
          - name: ingress-tls
            kind: Secret
---
apiVersion: gateway.networking.k8s.io/v1beta1
kind: HTTPRoute
metadata:
  name: openai-httproute
  namespace: default
spec:
  parentRefs:
    - name: openai-gateway
      sectionName: "https"
  hostnames:
    - "localopenai.example"
  rules:
    - backendRefs:
        - name: api.openai-mock.com
          kind: Hostname
          group: networking.istio.io
          port: 443
      filters:
        - type: URLRewrite
          urlRewrite:
            hostname: api.openai-mock.com
EOF

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
