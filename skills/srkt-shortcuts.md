---
name: srkt-shortcuts
description: Manage srkt text expansion shortcuts via CLI, including mandatory daemon reload.
version: 1.0.0
---

# srkt shortcuts

Use when the user asks to:
- add a shortcut / expansion to srkt
- remove a shortcut from srkt
- list or show current srkt shortcuts
- edit a trigger or expansion in srkt

## Context

- CLI: `srkt`
- Config: `~/.config/srkt/expansions.toml`
- Daemon reload: `srkt reload`

## Rules

1. Only make changes the user explicitly requested.
2. Before any change, check the current state with `srkt list`.
3. After every `srkt add` or `srkt remove`, run `srkt reload` to be sure
   (the CLI attempts it automatically, but an explicit reload is safer).
4. After the change, verify the result with `srkt list`.
5. If anything looks off, check `srkt status`.
6. Respect prefix conflicts: a trigger must not be a prefix of another trigger
   and vice versa — `srkt add` will error if this rule is violated.
7. For multiline expansions use `\n` in the CLI argument; srkt converts it to a newline.

## Workflow

### List expansions
```bash
srkt list
srkt status
```

### Add or overwrite an expansion
```bash
srkt add '/trigger' 'expansion text'
srkt reload
srkt list
```

### Multiline expansion
```bash
srkt add '/sig' 'Line 1\nLine 2'
srkt reload
srkt list
```

### Remove an expansion
```bash
srkt remove '/trigger'
srkt reload
srkt list
```

## What to report to the user

Keep it brief:
- what changed (trigger + resulting value)
- that `srkt reload` ran
- the error message verbatim if the operation failed (prefix conflict, trigger not found, etc.)

## Notes

- Triggers fire instantly on the last character — there is no terminator key (space/enter).
  Make sure the trigger is specific enough that it won't fire accidentally mid-word.
- If the daemon is not running, `srkt add` / `srkt remove` still write the config —
  changes take effect the next time the daemon starts.
- The daemon can also be managed via systemd: `systemctl --user start|stop|restart|status srkt`

---

## Installation

### Claude Code / oh-my-claudecode

Copy this file to your Claude Code skills directory:

```bash
# oh-my-claudecode (default location)
cp skills/srkt-shortcuts.md ~/.claude/skills/srkt-shortcuts.md

# Plain Claude Code
cp skills/srkt-shortcuts.md ~/.claude/skills/srkt-shortcuts.md
```

Then invoke it by name in your session:

```
/srkt-shortcuts
```

Or reference it from another skill / CLAUDE.md with:

```
Invoke the srkt-shortcuts skill when the user asks to manage text shortcuts.
```

### GitHub Copilot CLI / other agents

Place the file anywhere your agent discovers skills, then load it by name.
The YAML frontmatter (`name`, `description`) is what agents use for auto-detection.
