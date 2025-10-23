<p align="center">
    <img width="512" height="512" alt="Image" src="https://github.com/user-attachments/assets/673eff60-9a74-4092-a5b5-20fadba0c20f" />
</p>

# Test Quest

A declarative end-to-end testing framework for APIs.
Tests are defined in TOML files, so they are language-agnostic and easy to share across projects.
The runner executes your tests by sending HTTP requests, checking responses, and (optionally) inspecting the database.

## Example

```toml
[setup]
base_url = "http://localhost:6969"
command = "cargo"
args = ["r", "-p", "test_app"]
ready_when = "/health"
database_url_env = "DATABASE_URL"

[db]
db_type = "postgres"
migration_dir = "./utils/test_app/migrations"

# --------------------
# Group 1: Auth tests
# --------------------
[[test_groups]]
name = "auth"

[test_groups.before_group]
run_sql = ["""INSERT INTO users (id, name, password) VALUES
    (1, 'Alice', '123'),
    (2, 'Harry Potter', '1234'),
    (3, 'Charlie', '4321')
ON CONFLICT (id) DO NOTHING;"""]

[[test_groups.tests]]
name = "LoginUser"
method = "POST"
url = "/login"
body = { username = "Harry Potter", password = "1234" }
assert_status = 200
assert_headers = { Content-Type = "application/json" }

[[test_groups.tests]]
name = "ChangeUserPassword"
method = "PATCH"
url = "/login/password/change"
body = { username = "Harry Potter", password = "123123" }
assert_status = 200
assert_headers = { Content-Length = "0" }
assert_sql = { query = "SELECT password FROM users WHERE name = 'Harry Potter';", expect = "123123" }

[[test_groups.tests]]
name = "DeleteUser"
method = "DELETE"
url = "/users/1"
assert_status = 200
assert_json = { id = 1, name = "Alice", password = "23" }

[[test_groups.tests]]
before_run ={ run_sql = [
  "INSERT INTO users (id, name, password) VALUES (1, 'Alice', '123') ON CONFLICT (id) DO NOTHING;",
] }
name = "GetUser"
method = "GET"
url = "/users/1"
assert_status = 200

```

## Roadmap / TODO

 - [X] __Basic runner__

 - [X] __CLI__

 - [X] __Database support__

 - [X] __Server lifecycle__

 - [ ] __DSL__

    - Rich domain specific language for expressive, composable tests

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
