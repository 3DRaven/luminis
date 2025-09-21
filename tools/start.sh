#!/bin/sh

java -jar wiremock-standalone.jar \
  --port 8080 \
  --proxy-all="https://generativelanguage.googleapis.com" \
  --record-mappings \
  --verbose \
  --print-all-network-traffic
