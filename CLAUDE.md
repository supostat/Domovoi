<!-- MNEME-CRITERIA-CONTRACT -->
## mneme: phase criteria contract

- A done-when criterion is ONE argv command — no quotes, no `&&`/`|`: the gate-runner spawns a
  single process, and any quote or shell operator is an instant red `malformed-command` gate.
- Executable-first: a criterion is a command with exit 0 and a DEFINITE target (a specific test
  file, a `grep -q MARKER path`) — never a bare full-suite run alone. An agent-judged criterion
  is allowed only where the outcome is fundamentally not machine-checkable, and is marked
  `agent-judged` explicitly.
- Every criterion command must EXIST in the project (a script in package.json, a file on disk) —
  verify before writing the spec, never guess.
<!-- /MNEME-CRITERIA-CONTRACT -->
