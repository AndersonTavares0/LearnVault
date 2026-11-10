# Agent collaboration and independent review

## Before work

Inspect Git status, branches, worktrees, recent history, and open PRs/issues before acting.
Never assume exclusive ownership. Use repository state and shared policies to coordinate
across agents and harnesses; direct communication is supplementary, not the source of truth.
Avoid duplicating work already in flight. Use an isolated branch or checkout when ownership
could overlap, and preserve other agents' uncommitted and committed changes.

## Choosing the work structure

Use one agent for simple, bounded changes. Delegate only when separate responsibilities
improve the result and the active user or harness instructions permit delegation. Parallel
agents are optional; they are not evidence that a change was reviewed independently.

Before delegating, give each agent the source ref, relevant instructions, task boundary,
owned files or isolated checkout, acceptance criteria, and required evidence. Assign
non-overlapping writes. The coordinating agent checks the returned diff, test results,
and unresolved findings before integration and remains responsible for the whole change.
Preserve existing work; resolve ownership conflicts before editing shared files.

## Review according to risk

Changes to security controls, data persistence, or release/update behavior require an
independent review before merge. A human or a different agent may perform that review.
The implementer reviewing its own work does not satisfy this requirement. Other changes
follow the repository's normal review and CI requirements; a second agent is not mandatory.

Give the reviewer the actual proposed commit SHA, diff, applicable policy, and validation
evidence. The reviewer inspects the source independently and records the reviewed SHA,
identity or role, findings, and verification limitations in the PR. Reviewer agents remain
read-only and follow their execution permissions. A source review cannot certify tests
or CI results that the reviewer has not verified.

After additional commits, identify which changes affect the review. Refresh the review
for changes that invalidate its conclusions; record the new reviewed SHA and explain
any changes that do not affect those conclusions. Resolve blocking findings and complete
required checks before merge. This policy does not change GitHub protection settings or
make CODEOWNERS a mandatory approval gate.

## Integration

Review incoming changes against the applicable ADRs and contracts. Complete relevant tests,
lints, and builds before merging; unresolved conflicts, blocking findings, pending checks,
and hidden failures prevent merge. Continue independent authorized work and request guidance
only for genuine ownership conflicts, ambiguous requirements, or risk of losing existing work.

## Reporting evidence

Report checks actually executed, their result, and the source revision. Distinguish source
reasoning, synthetic reproduction, CI evidence, and untested paths. Keep secrets and personal
data out of review inputs. Missing tools or unavailable platforms remain explicit gaps;
they are not passing checks.
