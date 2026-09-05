# API cancellation status correction

The API previously acknowledged job cancellation by changing status and emitting
a cancelled event. Playbook workers had no cancellation handle, so execution could
continue and later overwrite the status. Kernel deployment checked the status only
after one operator-resume wait; this did not establish that its outer workflow or
all host operations had stopped.

`POST /api/v1/jobs/{id}/cancel` now returns HTTP422 with an unsupported diagnostic
for pending, running or action-required jobs. It does not change job status or
finish time, emit a cancellation event, or wake a kernel workflow. Completed jobs
still return HTTP409; unknown jobs now return HTTP404 instead of the prior generic
conflict. Authentication and identifier parsing retain their existing behavior.

The public legacy `AppState::cancel_job` signature remains unchanged. It now returns
false for every job without state or notification effects. The HTTP handler's
return type is now an explicit JSON result. These are alpha compatibility changes:
formerly acknowledged requests fail because execution stopping is not implemented.

Clients must check the HTTP result and keep showing the actual job status after
HTTP422. A button click or failed request is not a terminal cancellation event.
Use the existing status endpoint to observe completion. This patch does not stop
running commands, undo earlier effects, implement remote cancellation, or certify
worker shutdown and cleanup. It removes an unsupported acknowledgement.

Regression tests create job metadata only, without starting workers, listeners or
commands. They exercise REST and legacy-method behavior, event silence, and
registered kernel-resume waiters that must remain pending. No real deployment or
termination is used as a test fixture.
