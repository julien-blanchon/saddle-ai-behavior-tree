# Behavior Tree Lab

Crate-local standalone lab app for manually inspecting the shared `saddle-ai-behavior-tree` crate in a real Bevy application.

## Purpose

- verify shared-crate integration in a real app
- exercise reactive aborts, interval services, debug gizmos, and overlay inspection together
- provide a richer visual target than the minimal headless examples

## Status

Working

## Run

```bash
cargo run -p saddle-ai-behavior-tree-lab
```

## Notes

- Debug rings and target links rely on `BehaviorTreeDebugGizmos`, which this lab initializes explicitly.
- The lab keeps the scenario generic: one agent, one moving target, interval sensing, and a reactive fallback tree.
