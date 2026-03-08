# How to Contribute

We're excited you're interested in contributing to Marquez! We'd love your help, and there are plenty of ways to contribute:

* Give the repo a star
* Join our [slack](https://join.slack.com/t/ilum-cloud/shared_invite/zt-3jufrwyr9-HFrIbybdr9A3vE3fYaxgyg) channel and leave us feedback or help with answering questions from the community
* Fix or [report](https://github.com/ilum-cloud/marquez/issues/new) a bug
* Fix or improve documentation
* For newcomers, pick up a ["good first issue"](https://github.com/ilum-cloud/marquez/labels/good%20first%20issue), then send a pull request our way (see the [resources](#resources) section below for helpful links to get started)

We feel that a welcoming community is important and we ask that you follow the [Contributor Covenant Code of Conduct](https://github.com/ilum-cloud/marquez/blob/main/CODE_OF_CONDUCT.md) in all interactions with the community.

If you’re interested in using or learning more about Marquez, reach out to us on our [slack](https://join.slack.com/t/ilum-cloud/shared_invite/zt-3jufrwyr9-HFrIbybdr9A3vE3fYaxgyg) channel.

# Getting Your Changes Approved

Your pull request must be approved and merged by a [committer](COMMITTERS.md).

# Development

To run the entire test suite:

```bash
$ cd api-rs && cargo test --workspace
```

You can also run tests by category:

```bash
$ cd api-rs && cargo test -p marquez-tests -- db_tests      # DAO / data access tests
$ cd api-rs && cargo test -p marquez-tests -- api_tests      # HTTP integration tests
$ cd api-rs && cargo test -p marquez-tests -- sql_parity_test  # SQL parity tests
```

We use `cargo fmt` and `cargo clippy` for code formatting and linting. Make sure your code passes both before pushing any changes, otherwise CI will fail:

```bash
$ cd api-rs && cargo fmt --all          # auto-format
$ cd api-rs && cargo clippy --workspace -- -D warnings  # lint
```

<details>
<summary>Legacy Java Backend (deprecated)</summary>

```bash
$ ./gradlew test                        # all tests
$ ./gradlew :api:testUnit               # unit tests only
$ ./gradlew :api:testIntegration        # integration tests only
$ ./gradlew :api:testDataAccess         # data access tests only
$ ./gradlew spotlessApply               # auto-format (Google Java Style)
```

</details>

# `.git/hooks`

We use [`pre-commit`](https://pre-commit.com/index.html) to manage git hooks:

```bash
$ brew install pre-commit
```

To setup the git hook scripts run:

```
$ pre-commit install
```

# `.github/workflows`

Each Pull Request executes a series of quality checks via [GitHub Actions](https://github.com/ilum-cloud/marquez/blob/main/.github/workflows/rust-ci.yml). The CI pipeline includes the following jobs:

1. **check** -- `cargo fmt --check`, `cargo clippy`, `cargo build`
2. **test** -- `cargo test --workspace` against a PostgreSQL 16 service container
3. **sql-catalog** -- SQL parity catalog generation (compares Java and Rust queries)
4. **parity-tests** -- SQL parity and edge case parity tests
5. **docker** -- Docker image build verification

You can run these checks locally:

```bash
cd api-rs
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

# Troubleshooting

There is an issue within the _act_ tool that prevents the _kind_ cluster from being deleted after execution of the action.
When this condition exists, you will experience the error below.

```bash
| Creating kind cluster...
| ERROR: failed to create cluster: node(s) already exist for a cluster with the name "chart-testing"
[Lint and Test Chart/lint-test]   ❌  Failure - Create kind cluster
```

Execute the command below to manually clean up the _kind_ cluster and resolve the problem.

```bash
kind delete clusters chart-testing
```

# Submitting a [Pull Request](https://help.github.com/articles/about-pull-requests)

1. [Fork](https://github.com/ilum-cloud/marquez/fork) and clone the repository
2. Make sure all tests pass locally: `cd api-rs && cargo test --workspace`
3. Create a new [branch](#branching): `git checkout -b feature/my-cool-new-feature`
4. Make a change on your cool new branch
5. Write a test for your change
6. Make sure formatting passes: `cd api-rs && cargo fmt --all -- --check`
7. Make sure `.rs` files contain a [copyright and license header](#copyright--license)
8. Make sure to [sign you work](#sign-your-work)
9. Push the change to your fork and [submit a pull request](https://github.com/ilum-cloud/marquez/compare)
10. Work with project maintainers to get your change reviewed and merged into the `main` branch
11. Delete your branch

To ensure your pull request is accepted, follow these guidelines:

* All changes should be accompanied by tests
* Do your best to have a [well-formed commit message](https://tbaggery.com/2008/04/19/a-note-about-git-commit-messages.html) for your change
* [Keep diffs small](https://kurtisnusbaum.medium.com/stacked-diffs-keeping-phabricator-diffs-small-d9964f4dcfa6) and self-contained
* If your change fixes a bug, please [link the issue](https://help.github.com/articles/closing-issues-using-keywords) in your pull request description
* Any changes to the API reference require [regenerating](#api-docs) the static `openapi.html` file.

> **Note:** A pull request should generally contain only one commit (use `git commit --amend` and `git push --force` or [squash](http://gitready.com/advanced/2009/02/10/squashing-commits-with-rebase.html) existing commits into one).

# Branching

* Use a _group_ at the beginning of your branch names:

  ```
  feature  Add or expand a feature
  bug      Fix a bug
  proposal Propose a change
  ```

  _For example_:

  ```
  feature/my-cool-new-feature
  bug/my-bug-fix
  bug/my-other-bug-fix
  proposal/my-proposal
  ```

* Choose _short_ and _descriptive_ branch names
* Use dashes (`-`) to separate _words_ in branch names
* Use _lowercase_ in branch names

# Dependencies

Rust dependencies are managed via workspace-level `Cargo.toml` in `api-rs/Cargo.toml`. All crates in the workspace share dependency versions defined under `[workspace.dependencies]`.

We use [renovate](https://github.com/renovatebot/renovate) to manage dependencies for non-Rust project modules (web, clients). Renovate automatically detects new dependency versions and opens pull requests to upgrade dependencies in accordance with the [configured rules](https://github.com/ilum-cloud/marquez/blob/main/renovate.json).

The following dependencies are managed manually:

* _Web code_ - it is challenging to programmatically validate web content
* _Spark versions_ - the internal query plans parsed by the Spark OpenLineage integration are not stable across Spark versions

# Sign Your Work

The _sign-off_ is a simple line at the end of the message for a commit. All commits need to be signed. Your signature certifies that you wrote the patch or otherwise have the right to contribute the material (see [Developer Certificate of Origin](https://developercertificate.org)):

```
This is my commit message

Signed-off-by: Remedios Moscote <remedios.moscote@buendía.com>
```

Git has a [`-s`](https://git-scm.com/docs/git-commit#Documentation/git-commit.txt---signoff) command line option to append this automatically to your commit message:

```bash
$ git commit -s -m "This is my commit message"
```

# API [Docs](https://github.com/ilum-cloud/marquez/tree/main/docs)

To bundle:

```bash
$ redoc-cli bundle spec/openapi.yml -o docs/openapi.html  --title "Marquez API Reference"
```

To serve:

```bash
$ redoc-cli serve spec/openapi.yml
```

Then browse to: http://localhost:8080

> **Note:** To bundle or serve the API docs, please install [redoc-cli](https://www.npmjs.com/package/redoc-cli).

# `COPYRIGHT` / `LICENSE`

We use [SPDX](https://spdx.dev) for copyright and license information. The following license header **must** be included in all `java`, `bash`, `py`, and `rs` source files:

`java`

```
/*
 * Copyright 2018-2022 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */
```

`bash`

```
#!/bin/bash
#
# Copyright 2018-2022 contributors to the Marquez project
# SPDX-License-Identifier: Apache-2.0
```

`py`

```
# Copyright 2018-2022 contributors to the Marquez project
# SPDX-License-Identifier: Apache-2.0
```

`rs`

```
// Copyright 2024-2026 contributors to the Marquez project
// SPDX-License-Identifier: Apache-2.0
```

# Resources

* [How to Contribute to Open Source](https://opensource.guide/how-to-contribute)
* [Using the Fork-and-Branch Git Workflow](https://blog.scottlowe.org/2015/01/27/using-fork-branch-git-workflow)
* [Understanding the GitHub flow](https://guides.github.com/introduction/flow/)
* [Keeping a Changelog](https://keepachangelog.com)
* [Code Review Developer Guide](https://google.github.io/eng-practices/review)
* [Signing Commits](https://docs.github.com/en/github/authenticating-to-github/signing-commits)

----
SPDX-License-Identifier: Apache-2.0
Copyright 2018-2025 contributors to the Marquez project.
