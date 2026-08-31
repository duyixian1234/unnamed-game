# Mechanic-only upgrade paths, capped at Level 8

Weapon upgrade paths previously mixed direct damage (+25%/+30%) with mechanic effects, ending in a Level-6 Evolution. We decided that paths grant **mechanic effects only** (attack speed, range, knockback, projectile/orbit stats, orb size, extra instances) and never direct damage: all damage growth comes from the Shop. This gives each weapon a mechanical identity the player builds around, instead of every path collapsing into "take the damage nodes". The path cap moves from Level 6 to Level 8: six 2-choice mechanic rows (Lv2–7) plus the fixed Evolution at Lv8, so the added depth is mechanic depth, not stat padding.

Considered alternative: keeping damage nodes alongside mechanics was rejected — with damage present, the other options are strictly dominated for players optimizing clear speed.

Consequences: the balance-checkpoint tests (survival with `buy_items: false`) are the guard that base weapon DPS plus mechanic growth stays sufficient across waves. Attack-speed options are stored as positive fractions (+15%) and applied as proportionally shorter cooldowns, so display and data agree.
