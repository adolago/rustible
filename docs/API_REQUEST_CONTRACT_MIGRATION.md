# API request contract corrections

This alpha change affects the optional REST API feature. It does not establish
complete API, authentication, cancellation or execution safety.

## Execution restrictions

Previously, the playbook submission endpoint accepted `limit`, `tags`,
`skip_tags` and `start_at_task` but did not pass them to execution. A request
could therefore run beyond the caller's intended hosts or tasks.

The endpoint now returns HTTP422 with an explicit unsupported-option diagnostic
before resolving the playbook or creating a job when `limit` or `start_at_task`
is present, including an empty string, or either tag list is nonempty. Omitting
the optional strings or using null, and omitting tag lists or using empty lists,
retains the existing unrestricted request behavior. The implementation checks
options in the order above and reports the first unsupported request.

Clients must not silently remove these controls and retry: that would discard
the caller's intended restriction. Keep restricted submissions disabled until
selection semantics are implemented and verified for this API path. CLI and
library selection behavior must be assessed separately. No selector is newly
implemented by this repair.

## JSON body size

`ApiConfig.max_body_size` now configures Axum's body-consuming extractors on the
server router, including JSON login and submission requests. When a participating
extractor consumes the body, a body exactly at the configured byte limit is
accepted for parsing; larger bodies receive HTTP413. The bound applies to observed
streamed bytes as well as known-length bodies. Routing or authentication can
reject a request before body extraction; other parsing and validation errors
still apply. Callers using `routes::api_routes` directly must configure their
own body-limit layer.

Previously, the configured field was unused and Axum's default limit applied.
The default configuration remains10MiB; it now permits JSON above Axum's old
2MiB default when otherwise valid. Choose an explicit byte limit suitable for
the deployment. This is a body-extractor bound, not a WebSocket message limit,
rate limit, request deadline or bound on every allocation after parsing.

Verification uses in-process HTTP requests, synthetic JSON and temporary empty
playbooks. It opens no listener, contacts no host and executes no task. The
added test dependency reuses the existing locked Tower version.
