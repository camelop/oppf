# OPPF — Open-Prompt Project Format

**OPPF** is a specification for describing a software project entirely through
the prompts and acceptance criteria used to build it. Building on the idea of
open source, *open prompt* requires that the full contents of a project
directory can be reproduced by a generative AI, with an explicit, well-defined
acceptance process.

`opp` is the reference command-line tool for OPPF. It reads an `.opp/` directory
and drives a coding agent through the project lifecycle:

- **`opp impl`** — read the design and implement what it requires.
- **`opp review`** — have the agent check each acceptance property, with a
  pass/fail verdict per property.
- **`opp test`** — run the project's test suite and report the result.
- **`opp discuss`** — read the design and raise implementation uncertainties
  (conflicts, blocking questions, unspecified design decisions) before any code
  is written.
- **`opp clear`** — remove all generated files, reverting to the pre-generation
  state (keeps `.opp/`, `.git/`, and excluded paths); `--move <dir>` relocates
  them instead of deleting.
- **`opp upgrade`** — update `opp` itself to the latest release.

> The format itself is specified in [`.notes/guidelines.md`](.notes/guidelines.md).

## Project format

A directory conforms to OPPF when it contains an `.opp/` directory:

```
.opp/
  design/
    index.md            # the design — the single source of truth
    other_refs...        # additional reference material (optional)
  config.toml            # optional configuration
  review/                # optional: checked by the agent after generation
    property_0_name.md
    property_1_name.md
    ...
  test/                  # optional: run after review
    test.sh
    test_impl_files...
<implementation files>   # what the agent generates from the design
```

Notes:

1. If the design is a single file, you may use `.opp/design.md` instead of the
   `design/` folder.
2. A test bundle can itself be an OPPF project (e.g. `.opp/test/.opp/design.md`).
3. `review/` and `test/` are both optional.

## Configuration

`.opp/config.toml` (all fields optional):

```toml
agent = "claude-code"        # which coding agent to use
exclude = ["./resources"]    # external paths treated as read-only resources,
                             # not generated, checked, or modified
```

| Field     | Default        | Meaning                                                        |
| --------- | -------------- | -------------------------------------------------------------- |
| `agent`   | `claude-code`  | The coding agent that implements and reviews the project.      |
| `exclude` | `[]`           | Paths kept read-only — not generated, checked, or modified.    |

## Supported agents

`opp` drives agents through their command-line interfaces. The agent is selected
by the `agent` field in `config.toml`, or overridden per-run with `--agent`.

| `agent` value             | CLI invoked                                                  | Required on `PATH` |
| ------------------------- | ----------------------------------------------------------- | ------------------ |
| `claude-code` (`claude`)  | `claude --dangerously-skip-permissions --print <prompt>`    | `claude`           |
| `codex` (`codex-cli`)     | `codex exec --dangerously-bypass-approvals-and-sandbox <prompt>` | `codex`       |

Agents run with permission prompts skipped so they can write files
unattended. Adding another agent is a matter of implementing the `Agent` trait
in [`src/agent/`](src/agent/) and registering it in `agent::for_id`.

### Login preflight

Before driving the agent, `opp impl`, `opp review`, and `opp discuss` check that
you are logged in to the selected agent. If you are not, the command stops
immediately and tells you how to sign in, rather than failing deep inside a run:

```
opp: not logged in to codex — log in first:
      codex login                      # sign in with your ChatGPT account
      codex login --with-api-key       # or pipe an API key via stdin
```

(For Claude Code the hint is `claude auth login`, or exporting
`ANTHROPIC_API_KEY`.) When you are logged in, `opp` prints the active account
and proceeds. The check is skipped under `--dry-run`.

### Live progress

`opp` keeps its own output visually distinct from the agent's. opp's own
messages carry a bold-cyan `opp` badge; the agent's live activity is framed in a
dim gutter (`╭ │ ╰`) with its **process id**, **session id**, a numbered feed of
every action (file writes, commands, messages), and a highlighted final answer:

```
opp ✓ claude-code — logged in (claude.ai, you@example.com)
opp implementing /path with claude-code
╭─ claude-code · pid 1278707
│ session b93a131b-5887-4e98-b7f8-3f8d916cf117
│  1 Read .opp/design.md
│ · My task is to create a single shell script.
│  2 Write hello.sh
│  3 Bash bash hello.sh; echo "exit: $?"
│
│ ▌ Done. I implemented hello.sh at the project root …
╰─ done in 12s · 3 steps
```

Colors auto-disable when output is not a terminal or `NO_COLOR` is set. Pass
`-v, --verbose` to also echo the agent's raw streaming output. Agents without
structured streaming fall back to their native output (still framed).

When the run finishes, `opp impl` and `opp discuss` print how to continue the
**same** agent session, so you can iterate (or dig into the raised points)
without losing context:

```
Want changes? Continue this same claude-code session:
  claude --resume 6c4f75c9-…       # pick up where it left off, interactively
  claude --resume 6c4f75c9-… -p "…"   # send one more instruction headlessly
Or re-run `opp review` / `opp test` to check the result.
```

(For Codex the equivalents are `codex resume <id>` and
`codex exec resume <id> "…"`.)

## Prompts

The prompts `opp` sends to the agent are **not** hardcoded — they live in
[`templates/`](templates/) as [minijinja](https://docs.rs/minijinja) templates
(`impl.md.jinja`, `review.md.jinja`, `discuss.md.jinja`) and are embedded into
the binary at build time. Edit those files to tune the instructions; preview the
result with `opp --dry-run impl` / `--dry-run review` / `--dry-run discuss`.

## Usage

```sh
# Implement the design in the current OPPF project
opp impl

# Check every acceptance property
opp review

# Run the test suite
opp test

# Remove all generated files (keeps .opp/, .git/, excluded paths)
opp clear                    # asks for confirmation
opp clear --yes              # delete without prompting
opp clear --move ../backup   # move them out instead of deleting

# Discuss implementation uncertainties before coding
opp discuss                    # all tiers, printed to the terminal
opp discuss --level blocking   # only conflicts / blockers / clear flaws
opp discuss --level major      # blocking + major design decisions
opp discuss -o discussion.md   # write the discussion to a file
opp discuss -f "focus on error handling and the public API"   # add your own guidance
```

`opp discuss` reports concerns in three severity tiers — **blocking**
(contradictions, must-resolve questions, clear flaws), **major** (unspecified
but hard-to-change design decisions), and **minor** (small choices worth
agreeing first). `--level` sets the lowest tier to include; `-o/--output` writes
the result to a file instead of the terminal; `-f/--focus <TEXT>` adds your own
guidance to the prompt (what to pay attention to, or any requirements for the
review). It reads the design and streams its progress but never modifies the
project.

Global options:

| Option              | Description                                                              |
| ------------------- | ------------------------------------------------------------------------ |
| `-p, --path <DIR>`  | Project location. Defaults to discovering `.opp/` from the cwd upwards.  |
| `--agent <AGENT>`   | Override the agent from `config.toml` for this run.                      |
| `--dry-run`         | Print the agent commands and prompts that would run, without executing.  |
| `-v, --verbose`     | Echo the agent's raw streaming output alongside the parsed progress.     |

### Exit codes

| Code | Meaning                                                  |
| ---- | -------------------------------------------------------- |
| `0`  | Success (and, for `review`/`test`, everything passed).   |
| `1`  | A review property failed, or the test suite failed.      |
| `2`  | Usage or environment error (e.g. no `.opp/` found).      |
| `3`  | Not logged in to the selected agent.                     |

### How `opp test` runs the suite

`opp test` runs `.opp/test/test.sh` from the **project root** (the directory
that contains `.opp/`), so the script sees the implementation at the same paths
the design used — e.g. `bash hello.sh`. `OPP_PROJECT_ROOT` is also exported (the
absolute project root) for scripts that change directories.

## Install

One line installs (or upgrades) `opp` to `~/.local/bin`:

```sh
curl -fsSL https://oppf.dirp.dev/install.sh | sh
```

The installer is idempotent: re-run the same command to upgrade to the latest
release, or to no-op when you are already up to date. It downloads a prebuilt
binary for your platform (Linux/macOS) from the
[GitHub Releases](https://github.com/camelop/oppf/releases) and verifies its
SHA-256 checksum.

Once installed, `opp upgrade` self-updates in place (it runs the same installer,
targeting the directory of the running binary):

```sh
opp upgrade
```

```sh
# pin a version, choose a directory, or force a reinstall
curl -fsSL https://oppf.dirp.dev/install.sh | sh -s -- --version v0.1.2
OPP_INSTALL_DIR=/usr/local/bin curl -fsSL https://oppf.dirp.dev/install.sh | sh
```

## Building from source

Requires a [Rust](https://rustup.rs) toolchain.

```sh
cargo build --release      # binary at target/release/opp
cargo test                 # run the unit tests
```

## Example

[`examples/hello`](examples/hello) is a prompt-only OPPF project — it contains
only an `.opp/` directory (design, a review property, and a test). Walk the full
lifecycle on it:

```sh
git clone https://github.com/camelop/oppf.git
cd oppf/examples/hello

opp impl       # generate hello.sh from .opp/design.md
opp review     # check each acceptance property
opp test       # run the test suite
```

Use `opp --dry-run impl` to preview the agent call first, or `opp clear` to
reset back to just the `.opp/` design.

## License

MIT — see [LICENSE](LICENSE).
