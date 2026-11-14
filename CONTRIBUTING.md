# Contributing to buttercandy

Contributions are welcome: new presets and palettes, fixes, ports, better
hearing. This page says how to send one, and on what terms.

## The terms, in short

buttercandy belongs to Parker Loflin and is published under the GNU Affero
General Public License, version 3 ([LICENSE](LICENSE)). It has one owner on
purpose: one person who can enforce the licence, and who can decide later to
offer the project under other terms as well.

To keep it that way, **anything you contribute becomes Parker Loflin's.** You
keep the right to use what you wrote, and you are credited for it. If that is
not an arrangement you are happy with, please do not send a contribution; you
are free to fork the project under the AGPL instead.

## Contributor terms

By submitting a contribution to this project (a pull request, a patch, or any
other material offered for inclusion), you agree to the following. "The owner"
means Parker Loflin.

1. **Assignment.** You assign to the owner all right, title and interest in
   your contribution, including the copyright, throughout the world and for
   the full term of those rights. The assignment takes effect when you submit
   the contribution.

2. **Fallback licence.** Where the law does not allow some or all of that
   assignment (for example where copyright or moral rights cannot be
   transferred), you grant the owner a perpetual, irrevocable, worldwide,
   royalty-free licence to use, copy, modify, publish and distribute your
   contribution, to license it to others on any terms, and to transfer this
   licence. The licence is exclusive where the law allows that and
   non-exclusive otherwise. To the extent the law allows, you agree not to
   assert moral rights in your contribution against the owner or anyone the
   owner licenses it to.

3. **Patents.** You grant the owner, and everyone who receives the project
   from the owner, a perpetual, irrevocable, worldwide, royalty-free,
   non-exclusive licence under any patent claims you own or control that your
   contribution infringes, on its own or combined with the project.

4. **Your right to your own work.** The owner grants you a perpetual,
   irrevocable, worldwide, royalty-free, non-exclusive licence to use, copy,
   modify and distribute your contribution, as you submitted it, for any
   purpose.

5. **What you promise.**
   - The contribution is your own original work, or you have the right to
     submit it on these terms.
   - If your employer, school or a client has rights in what you write, you
     have their permission to contribute it on these terms.
   - Anything in it that you did not write yourself is clearly pointed out in
     the pull request, with where it came from and its licence.
   - As far as you know, the contribution does not infringe anyone else's
     rights.

6. **No obligation.** The owner may accept, change, decline or remove any
   contribution, and may license the project, your contribution included,
   under any terms.

7. **No warranty.** Apart from the promises in 5, you provide your
   contribution as it is, with no warranty of any kind.

You confirm your agreement by ticking the box in the pull request template.
A contribution cannot be merged without it.

## Credit

Contributors are named in the README unless they ask not to be. The
repository's history is periodically squashed, so the README, not the commit
log, is the record.

## Sending a change

You need a recent stable Rust toolchain; the README lists the extra packages
each platform needs.

Before opening a pull request, please run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Describe what the change does and how you checked it. A screenshot helps for
anything visible: `--shot out.png` renders without a window or an audio
device.

### Presets

A preset is one WGSL file. Read "The five voices" in
`src/gfx/shaders/preset_header.wgsl` first: every preset gives the kick, the
snare, the hi-hats, the bass line and the tune each their own colour, motion
and place, and scales how hard it moves by the reactivity setting. Check
yours with:

```bash
cargo run --release -- --shot voices.png --preset-file my.wgsl --sheet voices
cargo run --release -- --shot palettes.png --preset-file my.wgsl --sheet palettes
```

A stranger should be able to label the five columns of the first sheet, and
the second should look good in every palette.

### Palettes

A palette is one entry in `src/palette.rs`: a cyclic gradient, a colour for
each of the five voices, and a pair of accents for the interface.
