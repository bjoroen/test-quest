# Proff (working name)

A declarative end-to-end testing framework for APIs.
Tests are defined in TOML files, so they are language-agnostic and easy to share across projects.
The runner executes your tests by sending HTTP requests, checking responses, and (optionally) inspecting the database or mocks.

## Example

```toml
[setup.server]
mode = "external"
url = "http://localhost:3000"

[[tests]]
name = "health check"
method = "GET"
url = "/health"
expect_status = 200

[[tests]]
name = "create user"
method = "POST"
url = "/users"
body = { name = "Alice" }
expect_status = 200
expect_jsonpath = { "$.name" = "Alice" }

[[tests]]
name = "list users"
method = "GET"
url = "/users"
expect_status = 200
expect_jsonpath = { "$.users[0].name" = "Alice" }
```

## Roadmap / TODO

 - [ ] __Basic runner__

    - Parse TOML test files
    - Run HTTP requests against a configured server
    - Assert status codes and JSONPath values


 - [ ] __CLI__

    - proff run tests/api.toml
    = Options for filtering tests, updating snapshots, verbose output

 - [ ] __Database support__

    - In-memory DB (SQLite, DuckDB)
    - External DB (user-provided URL)
    - Containerized DB (via testcontainers)
    - Reset state between tests

 - [ ] __Server lifecycle__

    - External mode (assume running server)
    - Binary mode (spawn local binary, wait for readiness)

 - [ ] __Mock support__

    - Define mock services in TOML
    - Spin up lightweight mock servers with predefined routes

 - [ ] __Property testing__

    - Generate request bodies using strategies (string, int, regex, uuid, etc.)
    - Run multiple randomized inputs against the API
    - Assert invariants hold for all runs

 - [ ] __Snapshots__

    - Capture full HTTP responses or DB state as snapshots
    - Compare future runs against stored snapshots
    - Update snapshots when intentional changes are made

 - [ ] __Fuzzing__

    - Send malformed or random inputs
    - Check that server fails gracefully without panics


## License

MIT
