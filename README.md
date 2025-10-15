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

 - [X] __Basic runner__

    - [X] Parse TOML test files
    - [X] Run HTTP requests against a configured server
    - [X] Assert status codes and header values
    - [X] Print test results in a nice way

 - [X] __CLI__

    - [X] Nice looking error messages with [Miette Error](https://github.com/zkat/miette)
    - [ ] Options for filtering tests, updating snapshots, verbose output

 - [ ] __Database support__

    - In-memory DB (SQLite, DuckDB)
    - External DB (user-provided URL)
    - Containerized DB (via testcontainers)
    - Reset state between tests

 - [ ] __Server lifecycle__

    - External mode (assume running server)
    - Binary mode (spawn local binary, wait for readiness)

 - [ ] __Snapshots__

    - Capture full HTTP responses or DB state as snapshots
    - Compare future runs against stored snapshots
    - Update snapshots when intentional changes are made

 - [ ] __Mock support__

    - Define mock services in TOML
    - Spin up lightweight mock servers with predefined routes

 - [ ] __Property testing__

    - Generate request bodies using strategies (string, int, regex, uuid, etc.)
    - Run multiple randomized inputs against the API
    - Assert invariants hold for all runs

 - [ ] __Fuzzing__

    - Send malformed or random inputs
    - Check that server fails gracefully without panics


## License

The MIT License (MIT)

Copyright (c) <year> Adam Veldhousen

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
