# Weapon Evolution breaks the data-only weapon assumption

Status: amended by ADR-0010 — the Evolution now sits at Level 8 (path cap raised 6 → 8); the behavior-change principle below stands.

The original decision was that adding weapons later is data-only (stat tables + shared firing model). Weapon upgrade paths (per WeaponKind, 6 levels, 2 options per level) end in a Level-6 **Evolution (质变)** that is deliberately a *behavior* change, not a stat jump — e.g. the piercing projectile splits on hit. Pure stat jumps do not deliver a "qualitative transformation" worth planning a build around, so we accept per-weapon behavior code.

Scope guard: each weapon has exactly **one** Evolution behavior; hard-coding three behavior branches is acceptable and preferable to a general-purpose behavior DSL. The data-only rule still holds for *adding new base weapons*; it is waived only for the Level-6 Evolution.
