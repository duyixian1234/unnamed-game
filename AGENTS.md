# AGENTS.md

Guidance for agents working in this repository.

## Project

A Brotato-like horde-survival roguelike in Bevy 0.17 (WebGPU / WebAssembly), with images and audio generated via the `mmx` CLI. The current project state and all settled gameplay + technical decisions live in `CONTEXT.md`; see below for how to consume domain docs.

## Agent skills

### Issue tracker

Issues and specs live as GitHub issues, operated via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical triage roles map to default labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` at the repo root plus `docs/adr/` for architecture decisions. See `docs/agents/domain.md`.
