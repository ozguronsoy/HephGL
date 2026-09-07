## Unit Tests

Unit tests reside within the `src` folder and should focus exclusively on isolated components that
do not require a window context.

You can run unit tests using the standard `cargo test` command.

## Integration Tests

Integration tests are located in the `tests` directory and focus on components that require
creating a window, such as the rendering backends.

Normal `cargo test` execution runs tests in worker threads, which causes failures because window
creation must occur on the main thread. HephGL uses `nextest` and `libtest-mimic` to execute each
test on the main thread of its own isolated process. You can run the integration tests using the
`cargo nextest run` command.

### Helpers

- `utils/`: Contains core testing macros and environment setups shared across all integration tests.
- `renderer/`: Provides the backend-agnostic renderer test suite. All tests in the suite can be configured via test flags:
    - `skip_test_*`: Skips the test.
    - `todo_test_*`: Marks the test as todo. This flag is used when a feature is not yet implemented by the renderer. These tests are expected to fail with `todo!()`.
    - `unimplemented_test_*`: Marks the test as unimplemented. This flag is used when a feature is not supported by the renderer. These tests are expected to fail with `unimplemented!()`.

## Code Coverage

Generating a complete code coverage report requires a two-step process since HephGL uses two
different test runners.

```bash
# Generate the report for unit tests
cargo llvm-cov --lib

# Run integration tests and combine the coverage data
cargo llvm-cov nextest
```
