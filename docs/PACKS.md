# Character packs

A pack is one text file ending in `.petpack`. It can hold up to 128 characters with their drawings,
tools, colours, lines to say and moves of their own. A pack is only data: nothing in it is ever run,
and everything is checked before it is used.

## Using one

- Right-click the pet, **Looks > Packs > Import a pack**, or drop the file on the pet (not on macOS).
- The new characters are in **Looks > Character**; the arrows at the top go from pack to pack.
- **x** next to a pack removes it. The file you imported stays where it is.
- Imported packs are kept in the packs folder (**Open the packs folder**). Up to eight at a time.
- A pack with the same `id` replaces the one before, so an update keeps everybody's choice.

Only import and share packs whose characters you are allowed to use. Characters from films, comics and
games belong to somebody, also when you draw them yourself.

## Writing one

[`example.petpack`](example.petpack) is a complete small pack, and
[`../assets/heroes.petpack`](../assets/heroes.petpack) the big one that comes with the pet. If something is
wrong with a file, the pet says in which line.

The first line is `petpack 1`. Lines starting with `#` are notes. Then come:

| | |
|---|---|
| `id` | small letters, digits and `-`, at most 24. Keep it the same for updates |
| `name` | what the pack is called, at most 24 letters |
| `by`, `about` | optional |
| `colour X = #rrggbb` | makes the letter `X` a colour in this pack. Not `.`, `K` or `E` |

After that any number of `[tool name]`, `[move name]` and `[pet name]` parts.

Drawings are rows starting with `|`, all equally long, one letter a pixel. `.` is nothing, `K` the dark
outline, `E` an eye. The other letters every pack has:

`O` orange `o` dark orange `P` pink `p` sweet pink `Z` blush `W` cream `i` white `G` pale blue `w` pale lilac
`D` brown `d` dark brown `M` bark `H` dark hair `h` hair `l` blond `e` rust `S` `s` `I` skin `L` peach
`C` clay `R` red `r` dark red `Y` yellow `y` beak `z` spark `A` gold `a` dark gold `g` green `j` dark green
`J` bright green `b` blue `u` dark blue `U` grey blue `c` ice `k` pale light `q` screen `v` violet `F` lilac
`f` dark lilac `T` `t` slate `x` dark cloth `N` dark grey `n` steel `m` light steel `V` silver `B` black

### A character

| | |
|---|---|
| `name`, `tab` | at most 12 and 9 letters. A pack has up to 8 tabs of up to 16 |
| `head`, `torso` | drawings. The head overlaps the first row of the torso. With legs at most 22 wide and 20 high |
| `face`, `eye` | the colours behind and of the eyes |
| `build` | `legs` (if not said), `wheels` or `hover` |
| `legs` | distance from the middle, length, colour, foot colour |
| `arms` | length, colour, hand colour and, if you like, thickness |
| `tail` | length, colour, colour of the tip |
| `back` | `cape R`, `wings D d`, `pack N`, `swords`, `limbs j` (with `claws` for metal ones), or `disc RiRb i`: a round shield's rings from the outside in, and its dot |
| `tool` | the name of a tool, held in the front hand |
| `glow` | the colour of its powers |
| `gift` | how it gets about and saves itself from a fall: `fly`, `leap`, `web`, `cable`, `limb`, `warp`, `none` |
| `aloft` | what holds it up if it flies: `jets`, `board`, `fire`, `firewings`, `orbs`, `ring` |
| `board` | colours of the board: body, top and, if you like, flame |
| `lights` | up to six colours for what it throws or holds |
| `door` | `yes` to go through a portal when it warps |
| `mouth` | sideways from the middle, and the row of the head |
| `voice` | `aloof`, `eager`, `gentle`, `formal` or `machine` |
| `says`, `says EVENT` | its own words, instead. Events: wake morning focus task pet late tired sad back hungry fed hit. Can be there several times |
| `foods` | three of: fish milk bone meat cookie tea pizza burger coffee fuel oil nitro battery bolt ice-pop candy carrot apple fly plasma ramen cake donut shawarma water |
| `tricks` | one to eight moves: the pack's own, or the pet's |

The pet's moves: look wave dance hop yawn sneeze sit spin groove headbang disco type think groom wag pounce
jiggle phone sip backflip rev drift honk glitch slide roar spook loop vanish float hops tongue scan wheelie blast
soar flare punch storm stomp zap swing hang line portal levitate pulse fade shrink giant slice reach bloom lull
doubles moonrise big-fist kneel echo dash spikes bubble orbit blaze surf limbs lob flick

### A tool

`art` is a drawing of at most 16 x 16, `grip` the column and row that the hand holds, counted from 0.

### A move

`secs` says how long it takes, 0.3 to 8. Everything else is optional. Most of it is a list of `time:value`,
and the pet goes smoothly from one to the next. End where you began, or the pet will jump.

| | |
|---|---|
| `dx`, `dy` | sideways and up, in pixels of the drawing |
| `sx`, `sy`, `shear` | width, height (1 = its own), slant |
| `rot`, `pivot` | turned by so many degrees, about the feet (0) to the top (1) |
| `leg1` `leg2`, `lift1` `lift2` | each foot forward, and lifted |
| `crouch` | 0 to 1 |
| `arm1`, `arm2` | back and front arm: 0 hangs, 90 straight out, 180 up |
| `reach` | length of the arms, 1 = their own |
| `headx`, `heady`, `look`, `turn`, `mouth`, `tail`, `wing` | |
| `alpha`, `aura`, `clones` | see-through, glow, copies beside it, 0 to 1 |
| `eyes` | `open`, `half`, `closed`, `happy`, `wide`, `angry`, `dizzy` |
| `gone`, `infront` | `yes` while the tool is out of its hands, or the arms are in front of it |
| `fx` | an effect and its numbers: `0.4:ray,0.5` |
| `speed = V A B` | runs V pixels a second between the seconds A and B |
| `puff = WHAT EVERY` | `trail`, `shards`, `smoke`, `spark`, `ash`, `petal`, `heart` or `zed`, every so many seconds; two more numbers say from when to when |
| `hit = WHAT AT` | `ground`, `front`, `dust` or `click` at that second |
| `warp = AT` | is suddenly somewhere else |

Effects with one number from 0 to 1: ray eyeray rays ring portal bolt thrown slash cut fist spikes moon flash
jets lights heart flowers dome board cards, and bomb (0 to 1 in the air, to 2 going off). With a second number,
the seconds it has been going on: orbs sigil arrow hoops radar fire limbs firewings scrap. With none: none
thread hang claws.
