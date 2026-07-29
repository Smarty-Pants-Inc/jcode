---
name: upstream-merge
description: Use when taking upstream changes into the Smarty Pants jcode fork, adding a new patch branch, or rebuilding and activating a fork build. Covers refresh, rebase-onto ordering, mutation-proving fixes, publishing, pinning, and the reload/activation path including why `selfdev reload` can silently no-op.
allowed-tools: bash, read, edit, write, grep, agentgrep, batch, todo
---

# Upstream merge (Smarty Pants jcode fork)

This fork keeps `master` byte-identical to upstream and stacks changes above it,
so upstream lands as a rebase rather than a merge. This skill covers the full
cycle: refresh, add a patch, build, activate, publish, pin.

Read `AGENTS.md` for the invariants. This file is the procedure and the traps.

## Ground rules

- **Never commit to `master`.** It must fast-forward. A commit there breaks
  refresh and Jcode's own auto-updater (`Cannot fast-forward to multiple branches`).
- **Keep exactly two remotes**, `origin` and `upstream`.
- **Upstream-bound patches stay below fork-local ones**, so the branches offered
  upstream stay clean.
- All stack commands run from the `smarty-dev` monorepo, which pins this repo at
  `repos/jcode`.

## Refresh from upstream

```sh
pnpm jcode:stack      # inspect before changing anything
pnpm jcode:refresh    # fast-forward master, rebase the stack onto it
pnpm jcode:stack      # confirm: same commit count, no duplicates
```

Verify the stack has exactly the expected number of commits. Duplicated subjects
mean commits were replayed on top of their own rebased twins.

**Trap: never rebase a stack with `git rebase <base> <branch>`.** That recomputes
the merge base, so once a lower patch is rebased, upper branches replay their old
commits on top of the new ones and self-conflict. Rebase each branch with its
*pre-refresh* tip as the old base:

```sh
git rebase --onto <new-base> <old-base> <branch>
```

`bin/jcode-fork refresh` already does this. If rebasing by hand, capture every
branch tip *before* starting.

## Adding a new patch

1. Branch from the correct point in the stack — above the last upstream-bound
   patch, below the fork-local docs patch:

   ```sh
   git switch -c patch/<name> patch/<last-upstream-bound-patch>
   ```

2. Make the fix. Add a regression test.

3. **Prove it by mutation.** Revert the fix, confirm the test fails, restore it:

   ```sh
   cp <file> /tmp/f.bak
   # remove the fix
   ./scripts/dev_cargo.sh test --test <name>   # MUST fail
   cp /tmp/f.bak <file>
   ./scripts/dev_cargo.sh test --test <name>   # MUST pass
   ```

   A test that passes with the bug present guards nothing. This repo has already
   shipped one such test, caught only by mutation testing. Test the real call
   site, not a helper in isolation.

4. Restack the fork-local patches on top of the new branch:

   ```sh
   git rebase --onto patch/<name> <old-base> patch/smarty-fork-docs
   ```

5. **Register the branch in `bin/jcode-fork`.** The `patch_branches` array is the
   single source of truth for stack order; `stack`, `refresh`, `publish`, and the
   pinned tip all derive from it. A branch missing here is silently not published.

## Building and testing

`scripts/dev_cargo.sh` wraps cargo with the repo's toolchain, linker, and memory
policy. Pass the cargo command as arguments; sourcing it bare runs an empty
command and fails with ``no such command: `` ``.

```sh
export JCODE_DEV_TOOLCHAIN="nightly-2026-04-29"   # already in ~/.exports
./scripts/dev_cargo.sh build --bin jcode
./scripts/dev_cargo.sh test --test <name>
```

If a build fails inside the AWS dependency tree (`aws-lc-sys`), `dev_cargo.sh`
picked its stale default nightly (1.91). Set `JCODE_DEV_TOOLCHAIN` to >= 1.94.1.

**Establish the baseline before blaming the stack.** Some upstream tests fail on
a clean checkout. Confirm by checking out `master` and running the same tests:

```sh
git switch master && ./scripts/dev_cargo.sh test -p jcode-app-core --lib -- <filter>
```

Known pre-existing failures on upstream `master` (not ours):
`tool::bash::tests::test_stdin_forwarding_*`,
`agent::tests::build_memory_prompt_nonblocking_defers_pending_memory_during_tool_loop`.

Integration tests that need a socket can point the binary at a stub with
`JCODE_SOCKET`. Note that `jcode server reload` opens a throwaway liveness-probe
connection first and closes it without sending data, so a stub must keep
accepting until a connection actually delivers a frame.

## Activating a build

Building and activation are Jcode's own self-dev machinery. Do not wrap the
launcher: `~/.local/bin/jcode` is a symlink Jcode rewrites on every publish.

| Where | Command |
| --- | --- |
| In a self-dev session | `selfdev build`, `selfdev build-reload`, `selfdev reload` |
| From a shell | `jcode self-dev --build` |

Outside a self-dev session, `selfdev` only exposes `enter`, `setup`, `reload`,
`status`, `find-config`. `build`/`build-reload` are gated on `session.is_canary`,
so a plain session **cannot publish a fresh build** — only reload onto an
already-installed one.

### Trap: `selfdev reload` reports "already newest" while running old code

Reload detection compares **binary mtimes**, resolving candidates through the
`current` / `shared-server` channel symlinks — not version strings. So if those
channels already point at the new build, the daemon compares the new binary
against itself, finds nothing newer, and keeps executing the old one.

Do not hand-repoint `shared-server`; the installer sets the channels. Check what
the daemon is actually executing rather than trusting the message:

```sh
ps ax -o pid,args | grep 'jcode serve' | grep -v grep
lsof -p <pid> | awk '$4=="txt"{print $NF}'   # resolves to builds/versions/<hash>/jcode
```

If it is stale, force it. This re-execs the daemon in place, keeping its PID:

```sh
jcode server reload --force
```

A forced reload interrupts in-flight tool calls in the running session. That is
expected.

## Publish, pin, verify

```sh
pnpm jcode:publish    # push master and every registered patch branch
pnpm jcode:pin        # stage the parent gitlink at the stack tip
pnpm jcode:check      # self-dev target, stack, pin, escape hatch
```

`publish` prints the branch count — confirm it matches the stack. `pin` only
*stages* the gitlink; `check` reads committed state, so it keeps reporting drift
until the parent commit lands. Commit the gitlink together with any
`bin/jcode-fork` change, since the pin is only meaningful once the branch is
registered.

`check` warns that `JCODE_REPO_DIR` is unset in shells started before it was
exported. Verify in a login shell:

```sh
zsh -l -c 'cd <monorepo> && bin/jcode-fork check'   # expect "Jcode fork ready"
```

If `JCODE_REPO_DIR` is genuinely unset, self-dev silently targets the upstream
clone at `~/.jcode/source/jcode` and can publish unpatched code over the fork.

## Smoke test after activating

```sh
jcode run "reply with exactly: fork ok"
jcode run --provider-profile cliproxyapi -m gpt-5.6-sol "reply with exactly: sol routed"
```

The second exercises `patch/picker-provider-routes`: `gpt-5.6-sol` must route to
cliproxyapi, not a built-in provider.

Use `jcode-default` (stock upstream) to decide whether a bug is ours or
upstream's before writing a patch.
