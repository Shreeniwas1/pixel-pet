# Pixel Pet - Launch & Product Plan

## Vision
The one-stop utility pet for the desktop: a companion people keep running all day because it is
genuinely useful (focus, todos, system info) and genuinely charming (a pet that lives, reacts
and grows). Goal: better than every desktop pet currently available.

Pets today are either **decorative** (Shimeji, Desktop Goose, Desktop Mate, Neko) or
**useful but soulless** (Rainmeter, plain Pomodoro timers). Nobody does both well. That gap is ours.

## Launch decisions (settled)
- **One-time purchase**, no subscription, no backend, no accounts.
- **First launch: itch.io** (no upfront fee). Steam ($100 fee) is a later step once there are paying users.
- **Merchant of record only** (itch.io / Lemon Squeezy / Polar) so chargebacks and tax are not ours.
  Later: Microsoft Store (check current fee).
- Clear 14-day refund policy to keep disputes low.
- Windows only for now (Win32, Rust, tiny, no dependencies = our performance edge).

## Research summary
Sources: PC Gamer ("desktop pets are so back", "desktop pet renaissance"), Steam page for Desktop Mate,
Vaporlens review analysis, Deconstructor of Fun write-ups on Focus Friend and Finch, Forest UX case study.
Search results were thin and partly SEO junk, so treat as directional; verify with hands-on testing.

| Product | What it does well | Where it falls short |
|---|---|---|
| Desktop Mate (Steam, 3D mascots) | Big hit; characters walk, sit on windows, react; alarm | Little real function; reviewers bored in minutes; paid character DLC; mixed long-term reviews |
| V-pet style sims | Food, sleep, mood, mods; people want depth | Needs attention, not useful for work |
| Shimeji / Desktop Goose / Neko | Chaos, humour, nostalgia, mods | Gimmick; no utility; Goose is a prank |
| Bongo Cat | Reacts to your typing; streamers love it | Single trick |
| Focus Friend (Hank Green) | Pomodoro + a character that reacts when you lose focus; room that grows | Mobile only; engagement may fade |
| Finch | Pet lives its own life; widgets drive daily return; D1 ~54% / D7 ~37% (single secondary source) | Self-care, not desktop |
| Forest | Grow a tree by staying off your phone; strong goal loop | Mobile; no pet |

Take-aways:
1. Retention comes from **emotional attachment + a reason to come back daily** (pet that has needs/growth, streaks, a room/collection).
2. Pure decoration gets boring in minutes; **utility is what keeps it installed**.
3. Reacting to what the user is actually doing (typing, focus, idle, time of day) feels alive.
4. Customization and community content (skins, mods) extend life cheaply.
5. Low resource use matters: it runs all day beside games and work.

## Where we already are (v0.1)
Lightweight Win32 pixel pet, network speed (down/up), todo list, focus mode + Pomodoro,
usage stats, tray icon, dragging, multi-monitor, fullscreen detection, bundled "did you know" facts.

## Differentiators (our edge)
1. **Useful AND alive**: the pet is the interface for focus, todos, and system health.
2. **Tiny and fast**: single exe, near-zero CPU/RAM, no telemetry, offline. Say so loudly.
3. **Pet reacts to real work**: focus sessions, idle, typing rhythm, battery, CPU/RAM spikes, time of day.
4. **Gentle, not guilt-tripping**: reactions are encouraging; no dark patterns.
5. **Privacy first**: all data local; no account.

## Roadmap

### Phase 0 - Ship-readiness (do first)
- [ ] Replace Wikipedia-derived `data/facts.txt` with original content (CC BY-SA is awkward for a paid app); update `NOTICE.md`.
- [ ] First-run tutorial / tooltips; sensible defaults.
- [ ] Installer or clean zip, app icon, version info, uninstall that leaves no mess.
- [ ] Code signing plan (or honest SmartScreen note on the page).
- [ ] Verify DPI, multi-monitor, fullscreen-game hiding, sleep/wake, low-memory behaviour.
- [ ] Split `main.rs` (about 1,900 lines) into modules; add tests for pomodoro/todo/format logic.

### Phase 1 - Make the pet alive (the hook)
- [x] State machine: idle, sleep, cheer, love, stretch, tired, sad (walk/sit/perch still to do).
- [x] Needs and mood: energy/happiness that depend on how the user works (focus gives joy; long idle gives sleep).
- [x] Time-of-day behaviour (sleeps at night, stretches in the morning).
- [x] Reacts to idle, mouse petting, cursor (eyes follow). Typing hook skipped (AV-prone); window perching still to do.
- [ ] Sounds (optional, off by default, volume control).
- [ ] 3-5 pet species/skins at launch, each with a distinct personality and idle animations.

### Phase 2 - Make it indispensable (the retention)
- [x] Focus: Pomodoro presets, daily goal, streaks, session history, weekly recap, pet celebrates milestones.
- [x] Todos: quick-add hotkey, due times/reminders, recurring tasks (daily/weekly/weekdays, "every monday 9am"). Still to do: drag to reorder, "today" view.
- [x] Break coach: water, posture, 20-20-20 eye rest, stretch prompts.
- [x] System buddy: CPU/RAM/disk/battery/temperature alerts through the pet; network speed (already built).
- [ ] Quick notes / clipboard shelf; mini calendar glance.
- [x] Global hotkeys (Ctrl+Alt+T add task, Ctrl+Alt+F focus), tray menu. Pet hides and stays quiet while a fullscreen app/game is in front (meeting detection still to do).
- [ ] Weekly recap card ("You focused 9h 20m, longest streak 6 days") - shareable image for marketing.

### Done: pixel hub menu
- [x] Left click opens a pixel-style hub (Tasks, Focus, Recap, Pets, Options, Quit) with pet picker and options pages; right click keeps the full native menu. Add new features as hub tiles.

### Phase 3 - Collection and personalization (the long tail)
- [ ] Unlockables earned by real use: hats, rooms, accessories, backgrounds.
- [ ] Theme / palette / size / opacity customization.
- [ ] Achievements (local), later Steam achievements if we ship there.
- [ ] Skin pack format so artists can contribute (basis for later Workshop).

### Phase 4 - Growth
- [ ] Demo / "Lite" free build on itch.io (pet + timer); paid unlocks the rest.
- [ ] Trailer (30-60s on a real desktop), screenshots, GIFs for Reddit/TikTok/Shorts.
- [ ] Wishlist/mailing list building, devlog.
- [ ] Steam (once funded), Microsoft Store, optional macOS/Linux port.

## Success metrics
- Pet still running after 7 and 30 days (self-reported via opt-in, or proxy: weekly active feedback).
- Median idle RAM < 15 MB, CPU ~0%.
- Refund rate < 5%, chargebacks ~0.
- itch.io: wishlist/follower count and conversion from demo to paid.

## Open questions
- Price point ($3-6 range?) and whether a free Lite build is worth it.
- Art direction: keep the pixel style (cheap, charming, performant) vs. higher-res.
- Pet species for launch and which one is the "hero" for marketing.
- Mod/skin support timing.

## Next step
Start Phase 0 (facts replacement and first-run tutorial), then prototype the Phase 1 state machine
because that is what makes people fall in love with it.
