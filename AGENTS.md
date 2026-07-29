# Smarty Pants fork

You are in [`Smarty-Pants-Inc/jcode`](https://github.com/Smarty-Pants-Inc/jcode),
a fork of [`1jehuang/jcode`](https://github.com/1jehuang/jcode). Upstream's
guidelines follow below and still apply. This section takes precedence where
they conflict.

## Patch stack

`master` is byte-identical to upstream. Our changes are a linear stack above it:

1. `patch/openrouter-catalog-deadlock` — catalog read/write lock inversion
2. `patch/picker-provider-routes` — oversized model-update frames dropped
   provider identity, so picker selections misrouted to a built-in provider
3. `patch/server-reload-subscribe` — `jcode server reload` sent a stateful
   request before subscribing, so the command always failed
4. `patch/smarty-fork-docs` — fork-local docs (this section) and the
   `upstream-merge` skill

The first three are upstream-bound. The tip is what we build and run.

Upstream's `CONTRIBUTING.md` asks for **issues, not PRs**, when a bug reproduces
easily: the maintainer rewrites fixes himself so he owns the assumptions. All 17
external PRs upstream are closed; every merged PR is his. So "upstream-bound"
here means *reported upstream as an issue*, and PR #1/#2 are on **our fork**, for
review and stack hygiene — not submissions to upstream.

| Patch | Upstream report |
| --- | --- |
| `patch/openrouter-catalog-deadlock` | not yet reported |
| `patch/picker-provider-routes` | not yet reported |
| `patch/server-reload-subscribe` | [issue #648](https://github.com/1jehuang/jcode/issues/648) |

## Rules

- **Never commit to `master`.** It has to fast-forward from upstream. A commit
  there breaks `pnpm jcode:refresh`, and because Jcode's auto-updater runs
  `git pull` in its own source clone, it can also break updates with
  `Cannot fast-forward to multiple branches`.
- **Keep exactly two remotes**, `origin` (this fork) and `upstream`. A third
  makes that same `git pull` ambiguous.
- **Add a new fix as a new patch branch** stacked on the current tip, then add it
  to `patch_branches` in smarty-dev's `bin/jcode-fork`. Do not fold unrelated
  changes into an existing patch.
- **Keep fork-local changes above upstream-bound ones** so the patches we intend
  to upstream stay clean and reviewable.
- **Prove behavior fixes by mutation.** Revert the fix and confirm the test
  fails. A test that passes with the bug present is not a regression test; this
  repo has already shipped one such test, caught only by mutation testing.
- **Establish the upstream baseline before blaming the stack.** Some upstream
  tests fail on a clean `master`. Check there before assuming a patch broke them.
- **Never patch a live binary or `~/.jcode/builds`.** Fix source here, then
  rebuild.

## Commands

**Run the `upstream-merge` skill** for the full refresh → patch → build →
activate → publish → pin cycle. It carries the procedure and the traps; this
section is only the map.

Building and activation are **Jcode's own self-dev machinery**, not a wrapper:

| Where | Command |
| --- | --- |
| In a self-dev session | `selfdev build`, `selfdev build-reload`, `selfdev test`, `selfdev reload` |
| From a shell | `jcode self-dev` (add `--build`) |

Outside a self-dev session the tool only exposes `enter`, `setup`, `reload`,
`status`, and `find-config`. That is deliberate: an ordinary session should not
rebuild the harness by accident. It also means a plain session cannot publish a
fresh build, only reload onto an already-installed one.

Stack and pin management lives in the [`smarty-dev`](https://github.com/Smarty-Pants-Inc/smarty-dev)
monorepo, which pins this repo at `repos/jcode`:

```sh
pnpm jcode:stack      # show the stack
pnpm jcode:refresh    # fast-forward master, rebase the stack onto it
pnpm jcode:publish    # push master and the patch branches
pnpm jcode:pin        # stage the parent gitlink at the current commit
pnpm jcode:check      # self-dev target, stack, pin, escape hatch
```

Upstream's own guardrails still apply before pushing: run
`scripts/check_guardrails.sh`.

If a self-dev build fails inside the AWS dependency tree, `dev_cargo.sh` picked a
stale default nightly. Set `JCODE_DEV_TOOLCHAIN` to a nightly >= 1.94.1.

## Runtime notes

- `jcode` runs this fork; `jcode-default` runs stock upstream. Use
  `jcode-default` to tell whether a bug is ours or upstream's before patching.
- `~/.local/bin/jcode` is a plain symlink into `~/.jcode/builds`, managed by
  Jcode itself. Do not wrap it; Jcode rewrites it on every publish.
- `~/.jcode/source/jcode` is Jcode's own clone and is runtime state, not a source
  of truth. Leave it on a clean `master`; keep patches here.
- Self-dev builds from whatever `JCODE_REPO_DIR` points at and publishes into
  `~/.jcode/builds`. It is exported to this repo, so `selfdev build` and
  `selfdev reload` act on the fork. If it is unset, self-dev silently targets the
  upstream clone and can publish unpatched code over the fork build. Run
  `pnpm jcode:check` after any self-dev reload, and commit source changes to a
  patch branch — a self-dev build alone leaves git state untouched.
- These are source builds, so the updater compares this checkout against its
  tracking branch rather than downloading releases. The stack tip tracks
  `origin/<stack-tip>`, so "update available" means the fork moved. Take upstream
  changes with `pnpm jcode:refresh`, not `git pull`.
- **Reload detection compares binary mtimes, not versions**, resolving candidates
  through the `current`/`shared-server` channel symlinks. If those already point
  at the new build, the daemon compares it against itself and reports "already
  newest" while still executing the old binary. Do not hand-repoint the channels;
  the installer sets them. Confirm what is actually running with
  `lsof -p <daemon-pid>` and force it with `jcode server reload --force`.

## More docs

- The `upstream-merge` skill (`.jcode/skills/upstream-merge/`) — the working
  procedure for refresh, patching, building, activating, publishing, pinning
- [Fork runbook](https://github.com/Smarty-Pants-Inc/smarty-dev/blob/main/integrations/jcode-fork/README.md)
  covers the patch stack, refresh, build and install, launchers, auto-update
  recovery, and rollback.
- [Jcode config runbook](https://github.com/Smarty-Pants-Inc/smarty-dev/blob/main/integrations/jcode/README.md)
  covers provider and model routes, MCP servers, and the swarm routing catalog.
- [`README.md`](README.md) carries the same fork summary for humans.

---

# Repository Guidelines

## Development Workflow

- **Commit as you go** - Make small, focused commits after completing each feature or fix
- If the git state is not clean, or there are other agents working in the codebase in parallel, do your best to still commit your work. 
- **Push when done** - Push all commits to remote when finishing a task or session
- **Run the guardrails before pushing** - `scripts/check_guardrails.sh` runs every gate in
  CI's Format + Quality Guardrails jobs (fmt, clippy `-D warnings`, and the warning,
  code-size, test-size, panic, swallowed-error, dependency-boundary, and wildcard-reexport
  ratchets). Use `--skip-slow` to skip cargo check/clippy, and `--fix` to rustfmt and
  rebaseline ratchets after intentional growth. CI tracks the `stable` toolchain, so run
  `rustup update stable` too: a stale local clippy passes on lints that CI enforces.
- **Use fast iteration by default** - Prefer `cargo check`, targeted tests, and dev builds while iterating
- **Rebuild when done** - When you are done making changes, build the source.
- **Bump version for releases** - Update version in `Cargo.toml` when making releases. When cutting a new release, look at all the changes that happened since the last release and determine what the version bump should be ie patch or minor, etc. 
- **Remote builds available** - Use `scripts/remote_build.sh` to offload heavy cargo work to another machine. If your build is terminated, likely is because there are not enough resources on this machine to build. use remote build in that case. Try checking the resource avaliablity on the machine before you run a build. 

## Logs
- Logs are written to `~/.jcode/logs/` (daily files like `jcode-YYYY-MM-DD.log`).

## Debug Socket
- Use the debug socket for runtime level debugging

## Install Notes
- `~/.local/bin/jcode` is the launcher symlink used from `PATH`.
- `~/.jcode/builds/current/jcode` is the active local/source-build channel; self-dev builds and `scripts/install_release.sh` point the launcher here.
- `~/.jcode/builds/stable/jcode` is the stable release channel; `scripts/install.sh` installs this and points the launcher here.
- `~/.jcode/builds/versions/<version>/jcode` stores immutable binaries.
- `~/.jcode/builds/canary/jcode` still exists for canary/testing flows, but it is not the primary self-dev install path.
- On Windows, the equivalents are `%LOCALAPPDATA%\\jcode\\bin\\jcode.exe` for the launcher, `%LOCALAPPDATA%\\jcode\\builds\\stable\\jcode.exe` for stable, and `%LOCALAPPDATA%\\jcode\\builds\\versions\\<version>\\jcode.exe` for immutable installs; `scripts/install.ps1` currently installs the stable channel.
- Ensure `~/.local/bin` is **before** `~/.cargo/bin` in `PATH`.

