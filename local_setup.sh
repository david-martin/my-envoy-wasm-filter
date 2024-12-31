#!/bin/bash

set -ex

kind create cluster

# Gateway API CRDs
kubectl apply -f https://github.com/kubernetes-sigs/gateway-api/releases/download/v1.1.0/standard-install.yaml

# MetalLB
kubectl apply -f https://raw.githubusercontent.com/metallb/metallb/v0.14.9/config/manifests/metallb-native.yaml
sleep 40 # TODO: Wait for hooks

kubectl apply -n metallb-system -f - <<EOF
apiVersion: metallb.io/v1beta1
kind: IPAddressPool
metadata:
  name: kuadrant-local
spec:
  addresses:
    - 10.89.0.16/28
---
apiVersion: metallb.io/v1beta1
kind: L2Advertisement
metadata:
  name: empty
EOF

# Istio
helm repo add jetstack https://charts.jetstack.io --force-update
helm install sail-operator \
		--create-namespace \
		--namespace istio-system \
		--wait \
		--timeout=300s \
		https://github.com/istio-ecosystem/sail-operator/releases/download/0.1.0/sail-operator-0.1.0.tgz

kubectl apply -f - <<EOF
apiVersion: sailoperator.io/v1alpha1
kind: Istio
metadata:
  name: default
spec:
  # Supported values for sail-operator v0.1.0 are [v1.22.4,v1.23.0]
  version: v1.23.0
  namespace: istio-system
  # Disable autoscaling to reduce dev resources
  values:
    pilot:
      autoscaleEnabled: false
EOF

helm install \
  cert-manager jetstack/cert-manager \
  --namespace cert-manager \
  --create-namespace \
  --version v1.15.3 \
  --set crds.enabled=true

helm repo add kuadrant https://kuadrant.io/helm-charts/ --force-update
helm install \
 kuadrant-operator kuadrant/kuadrant-operator \
 --create-namespace \
 --namespace kuadrant-system

