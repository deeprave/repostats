## 1. ScannerTask Initialization API

- [x] 1.1 Replace the current flat `ScannerTask::new(...)` API with a dedicated builder whose required inputs are `scanner_id`, `repository_path`, `repository`, and `queue_publisher`.
- [x] 1.2 Add builder configuration methods for optional scanner state including `requirements`, `query_params`, `checkout_manager`, and notification manager override.
- [x] 1.3 Implement `ScanRequires::NONE` as the default builder value for requirements.
- [x] 1.4 Resolve notification manager defaulting in `build()` so runtime code uses the global notification service unless an override is supplied.

## 2. Scanner Construction Call Sites

- [x] 2.1 Update `ScannerManager` task creation to use the builder-based scanner initialization API.
- [x] 2.2 Update test builders and helper construction paths to remain concise with the new initialization flow.
- [x] 2.3 Review scanner call sites to ensure checkout support is only supplied through the optional configuration path when required.

## 3. Validation

- [x] 3.1 Run scanner tests to confirm normal scan publishing behavior still requires and uses `QueuePublisher`.
- [x] 3.2 Run `cargo nextest run --workspace` to confirm scanner initialization behavior is unchanged.
- [x] 3.3 Run clippy validation and verify the `ScannerTask::new(...)` arity issue is resolved.
