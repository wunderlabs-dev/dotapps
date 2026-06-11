Review open pull requests interactively.

1. Run `gh pr list --state open --limit 50 --json number,title,author,headRefName,isDraft,additions,deletions,changedFiles,labels,reviewDecision` and present the list.
2. Ask which PRs to review (single, range, or "all").
3. For each selected PR, create or update a Canvas at `canvases/pr-review.canvas.tsx` (read the canvas skill before authoring) with PR metadata and a review-decision UI.
4. For each PR, fetch the diff with `gh pr diff <num>`, run a focused review against `.cursor/rules/*.mdc`, and post the verdict to the canvas.
5. After all selected PRs are reviewed, ask whether to merge the approved ones.

Always use Canvas for this workflow, never a markdown dump.
