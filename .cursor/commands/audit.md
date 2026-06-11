Audit the codebase against all `.cursor/rules/*.mdc` rules.

Spawn the `rules-compliance-auditor` subagent (via the Task tool) and pass it the current branch's changes from `git diff origin/main...HEAD` plus the always-apply rule files. The auditor will spawn parallel subagents per rule category and return a structured report of violations.

Do not fix anything until the audit returns and we agree which findings to act on.
