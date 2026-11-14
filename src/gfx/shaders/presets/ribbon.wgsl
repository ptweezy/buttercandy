// Ribbon: an oscilloscope whose trace is left behind as a tunnel of rings.
//
// The live waveform runs across the middle. Every drum hit freezes it: the
// trace of that instant peels off as a bare bright line, curls round into a
// ring, opens out into a twisted two-faced ribbon and flies outwards past the
// camera. What is left behind is a tunnel of ribbon-rings, each one the
// actual waveform of one hit, so the wake is a score of the last few bars:
// the spacing of the rings is the rhythm, and their build and their colour
// are the drum that was hit.
//
// What the music does: the five voices of preset_header.wgsl. A ribbon keeps
// the colour of the voice that froze it for as long as it lives. Not every
// hit gets a ribbon (a ring is only so many slots, and a slow tunnel keeps
// its ribbons apart), so each drum also has an answer that needs none.
//
//   voice  what it does                       where                   colour
//   -----  ---------------------------------  ----------------------  ---------------
//   KICK   A heavy push (kickSum): every      The whole tunnel, from  voiceCol(KICK)
//          ring is shoved outwards and let    the centre. Its ribbon
//          back, and the kick's own ribbons   peels off the trace
//          swell and glow. Its ribbon is the  DOWNWARDS.
//          fat one: its spine is the bass of
//          the waveform, and it rolls like a
//          helix.
//   SNARE  A crack (snareSum): a straight     Across the whole        voiceCol(SNARE)
//          line flashes across the picture    picture, where the
//          and is gone, and the snare's own   trace lies. Its ribbon
//          ribbons flash with it. Its ribbon  peels off UPWARDS.
//          is the slim, straight one.
//   HATS   A tick (hatEnv): beads of light    The finest things in    voiceCol(HATS)
//          along the hats' ribbons sparkle,   the picture, brightest
//          a different handful each hit, and  on the outer rings.
//          so do the sparks a new one throws  Its ribbon peels off
//          outwards as it peels. Its ribbon   UPWARDS.
//          is thin lace.
//   BASS   A swell (uBassLine): the           The outermost rings in  voiceCol(BASS)
//          outermost rings grow wider and     sight, whichever drum
//          their faces fill with colour.      froze them.
//   TUNE   The live trace itself. It rides    The middle, across the  voiceCol(TUNE)
//          up and down with the pitch         picture, higher or
//          (uPitch) and stands taller the     lower with the pitch.
//          more tune there is (uTune). The
//          stretches whose notes are
//          sounding run white-hot (chroma: C
//          at the left, B at the right), and
//          a note starting plucks them
//          hotter and thicker (noteSum).
//
// Scenery: in pal() colours, slow, never flashing.
//   no tune   the trace is still the waveform, but dim, only faintly the
//             chord's colour (uHarmony), and back in the middle. (At all
//             times most of the waveform's low end is left out of it, so
//             that a kick or a bass note does not swing the tune's line.)
//   no drums  once no drum at all has played for a while (intro, breakdown,
//             idle) a slow clock lets a ribbon go, alternately down and up,
//             so the wake never dies. These are dim, in the chord's colour;
//             on tonal material they are the regular, flower-like ones.
//   loudness  the tunnel's clock is uPhase: rings fly out fast in a drop and
//             hang, tightly packed, in a breakdown. uEnergy lights the walls
//             (the soft streaks the rings leave behind them).
//   timbre    uCentroid twists the tunnel: a riser winds it up.
//   idle      everything is scenery, at full brightness: the clock's ribbons,
//             and a trace that drifts with uPitch while the outer rings
//             breathe with uBassLine.
//   setting   uReact (how hard to react) is how fast the tunnel flies. Turned
//             down, a ribbon curls, grows and leaves more slowly, so it
//             lasts longer and fewer hits (and clock ticks) get one, evenly
//             thinned to keep the rings apart. The live trace stands
//             shorter, less fuzzy and less glaring; a bright sound winds the
//             tunnel less; sparks fly less far. What the voices set off
//             shrinks and softens by itself (their envelopes follow uReact).
//
// How it works. The ribbons are not feedback smears: they are drawn afresh,
// exactly, every frame, from waveforms the preset keeps in its own previous
// frame. Nine pixel rows (five along the bottom edge, four along the top)
// are not picture but memory, written at values far too dark to see
// (about 0.0001):
//   rows 0-7  sixteen slots, two per row side by side. A slot is one frozen
//             waveform as three tracks: its spine (the slow part), its twist
//             (the rest, which becomes the ribbon's half-width) and the
//             spine a quarter-turn on. Kicks own slots 0-5, hats 6-11,
//             snares 12-15, each a ring buffer.
//   row 8     per slot: the phase it was born at, whether the clock let it
//             go, and the chord's colour then (which only the clock's
//             ribbons wear: a drum's ribbon is its voice's colour); the
//             three ring pointers; the clock; and the draw list (the live
//             slots sorted by age, so no pixel has to sort anything).
// Feedback proper is only the soft afterglow under the ribbons.

const K1: f32 = 6.103515625e-05;      // 2^-14: memory values live in [K1, 2 K1)
const KI: f32 = 4.76837158203125e-07; // 2^-21: unit for small integers
const NSLOT: i32 = 16;
const ROW_META: i32 = 8;
const BOTTOM_ROWS: i32 = 5;           // memory rows 0-4 run up from the bottom edge,
const TOP_ROWS: i32 = 4;              // rows 5-8 down from the top
const LIST0: i32 = 20;                // first column of the draw list in row 8
const KZ: f32 = 0.5;                  // how fast rings grow, per unit of uPhase
const D1: f32 = 1.3;                  // phase a ribbon takes to curl into a ring
const HLF: f32 = 0.59;                // half-length it shrinks to as it curls
const LIFE: f32 = 5.0;                // a ribbon's whole life, in phase
const TICK: f32 = 0.5;                // the quiet-passage clock, in phase
const QUIET: f32 = 0.9;               // seconds without a drum before the clock steps in
// (KZ, D1, LIFE and TICK are at full flight speed: see flight() below.)
const SLOW: f32 = 0.33;               // flight speed at no reactivity, as a share of full
const ROOM: f32 = 1.35;               // how much further apart a slow tunnel keeps its ribbons
const PUSH: f32 = 0.07;               // how far a full kick shoves the rings outwards, as a share of their radius
const LOWK: f32 = 0.35;               // how much of the waveform below ~230 Hz the live trace keeps
const SPAN: f32 = 0.6;                // how far the tune's pitch carries the live trace, bottom to top

// ---- reactivity ----------------------------------------------------------------
// What uReact does to the constant motion of this preset goes through these.
// They are written so that at uReact = 1 flight() is exactly 1, slack() 0 and
// calm(x) 1. (What the voices set off needs none of it: kickSum() and the
// other envelopes already follow uReact.)

// How fast the tunnel flies, as a share of full speed. A ribbon's age is
// multiplied by this before it is drawn, so at half speed it takes twice as
// long to curl, to grow and to leave, and lives twice as long (LIFE / flight()
// of phase, which has to stay under the 16 at which a slot is forgotten).
fn flight() -> f32 {
  let r2 = uReact * uReact;
  return 1.0 - (1.0 - SLOW) * (1.0 - r2 * r2);
}

// 0 at full speed, 1 from half speed down: how far the tunnel has gone over
// to letting fewer ribbons go, further apart.
fn slack() -> f32 { return min(1.0, 2.0 * (1.0 - flight())); }

// The quiet-passage clock's tick, in phase: longer as the flight slows, so
// that the ribbons it lets go stay as evenly spread as the drums' do.
fn tickLen() -> f32 { return TICK * mix(1.0, ROOM / flight(), slack()); }

// x at no reactivity, 1 at full: for sizes and rates that settle as it falls.
fn calm(x: f32) -> f32 { return 1.0 - (1.0 - x) * (1.0 - uReact); }

// ---- memory encoding ---------------------------------------------------------
// A half float keeps 10 bits within one power of two, so everything is
// stored as (1 + x) * K1 with x in 0..1. Zero means "nothing here".

fn enc01(x: f32) -> f32 { return (clamp(x, 0.0, 0.9995) + 1.0) * K1; }
fn dec01(v: f32) -> f32 { return clamp(v / K1 - 1.0, 0.0, 1.0); }
fn encI(n: f32) -> f32 { return (n + 128.0) * KI; }
fn decI(v: f32) -> f32 { return round(v / KI) - 128.0; }
fn encW(w: f32) -> f32 { return enc01(w * 0.5 + 0.5); }
fn decW3(v: vec3f) -> vec3f {
  return clamp(v / K1 - 1.0, vec3f(0.0), vec3f(1.0)) * 2.0 - 1.0;
}

// Pixel row of memory row `rb`.
fn rowPix(rb: i32) -> i32 {
  if (rb < BOTTOM_ROWS) { return i32(uRes.y) - 1 - rb; }
  return rb - BOTTOM_ROWS;
}
fn rowV(rb: i32) -> f32 { return (f32(rowPix(rb)) + 0.5) / uRes.y; }

// One cell of the memory, exactly.
fn cell(c: i32, rb: i32) -> vec3f {
  return textureLoad(tPrev, vec2i(c, rowPix(rb)), 0).rgb;
}

// ---- the waveform --------------------------------------------------------------

// Cubic B-spline through the 512 samples, in two linear taps: a smooth curve
// at any resolution, with the 8-bit steps averaged away.
fn waveS(u: f32) -> f32 {
  let x = u * 512.0 - 0.5;
  let i = floor(x);
  let f = x - i;
  let f2 = f * f;
  let f3 = f2 * f;
  let w0 = (1.0 - 3.0 * f + 3.0 * f2 - f3) / 6.0;
  let w1 = (4.0 - 6.0 * f2 + 3.0 * f3) / 6.0;
  let w2 = (1.0 + 3.0 * f + 3.0 * f2 - 3.0 * f3) / 6.0;
  let w3 = f3 / 6.0;
  let g0 = w0 + w1;
  let g1 = w2 + w3;
  let h0 = (i - 0.5 + w1 / g0) / 512.0;
  let h1 = (i + 1.5 + w3 / g1) / 512.0;
  return g0 * wave(h0) + g1 * wave(h1);
}

// The waveform below ~230 Hz.
fn lowAt(u: f32) -> f32 {
  var lo = 0.0;
  var wl = 0.0;
  for (var j = -18; j <= 18; j++) {
    let x = f32(j) * 2.0;
    let g = exp(-x * x / 392.0);
    lo += g * wave(u + x / 512.0);
    wl += g;
  }
  return lo / wl;
}

// The waveform split in two: x below ~230 Hz, y everything above it.
fn bandsAt(u: f32) -> vec2f {
  var fi = 0.0;
  var wf = 0.0;
  for (var j = -3; j <= 3; j++) {
    let x = f32(j);
    let g = exp(-x * x / 1.62);
    fi += g * wave(u + x / 512.0);
    wf += g;
  }
  let lo = lowAt(u);
  return vec2f(lo, fi / wf - lo);
}

// The same with the split higher up, for the hats: x below ~1.3 kHz, y above.
fn bandsHigh(u: f32) -> vec2f {
  var sm = 0.0;
  var ws = 0.0;
  var fi = 0.0;
  var wf = 0.0;
  for (var j = -7; j <= 7; j++) {
    let x = f32(j);
    let g = exp(-x * x / 12.5);
    let g2 = exp(-x * x / 1.62);
    let v = wave(u + x / 512.0);
    sm += g * v;
    ws += g;
    fi += g2 * v;
    wf += g2;
  }
  return vec2f(sm / ws, fi / wf - sm / ws);
}

// The waveform below a couple of kHz, cheaply: the trace without its fuzz.
fn softAt(u: f32) -> f32 {
  let h = 2.0 / 512.0;
  return 0.30 * wave(u) + 0.23 * (wave(u - h) + wave(u + h))
       + 0.12 * (wave(u - 2.0 * h) + wave(u + 2.0 * h));
}

// How tall the live trace stands (taller while there is a tune to show), and
// how much of its fuzz it keeps.
fn heroAmp() -> f32 { return calm(0.4) * (0.2 + 0.3 * uTune) + 0.2 * uIdle; }
fn heroFuzz() -> f32 { return calm(0.2); }
// How much of the waveform's low end (the kick and the bass line, which have
// places of their own) the live trace keeps: little, so that it is the tune's
// line and a kick does not swing it. All of it while idle.
fn lowKeep() -> f32 { return mix(LOWK, 1.0, uIdle); }
// How much the live trace is the tune's: 0 with no tune playing, 1 with one.
fn tuneIn() -> f32 { return sstep(0.04, 0.4, uTune) * (1.0 - uIdle); }
// Where the live trace sits: up and down with the pitch of the tune, and
// back to the middle when there is none.
fn lift() -> f32 { return SPAN * (uPitch - 0.5) * max(sstep(0.04, 0.4, uTune), uIdle); }
// The scenery's colour: the chord, as a place in the palette.
fn hueNow() -> f32 { return fract(uHarmony * 3.0 + uTime * 0.012); }

// ---- slots ---------------------------------------------------------------------

// Which drum owns slot s: 0 kick, 1 snare, 2 hat (the voices KICK, SNARE, HATS).
fn kindOf(s: i32) -> i32 { return select(select(2, 1, s >= 12), 0, s < 6); }
fn ringStart(b: i32) -> i32 { return select(select(6, 12, b == 1), 0, b == 0); }

struct Slot {
  age: f32,   // phase since birth; 1000 when empty
  hue: f32,
  soft: f32,  // 1 when let go by the clock rather than a drum
}

fn slotAt(s: i32) -> Slot {
  let m = cell(s, ROW_META);
  var o: Slot;
  o.age = 1000.0;
  o.hue = 0.0;
  o.soft = 0.0;
  if (m.r > 0.5 * K1) {
    let n = decI(m.g);
    o.soft = step(63.5, n);
    let birth = n - 64.0 * o.soft + dec01(m.r);
    o.age = fmod(fmod(uPhase, 64.0) - birth + 64.0, 64.0);
    o.hue = dec01(m.b);
  }
  return o;
}

// Where each ring buffer writes next: kick, snare, hat.
fn pointers() -> vec3f {
  let pp = cell(NSLOT, ROW_META);
  return clamp(vec3f(decI(pp.r), decI(pp.g), decI(pp.b)), vec3f(0.0), vec3f(5.0, 3.0, 5.0));
}

// x, y, z: the kick / snare / hat ring has something to freeze this frame.
// w: 1 when the kick ring's came from the clock, 2 when the hat ring's did.
fn events(tickStored: f32) -> vec4f {
  let tickNow = fmod(floor(uPhase / tickLen()), 64.0);
  let isTick = abs(tickNow - tickStored) > 0.5;
  let even = fmod(tickNow, 2.0) < 0.5;
  // An age of exactly zero: the hit happened this frame.
  let onL = uKickAges.x < 0.0002;
  let onM = uSnareAges.x < 0.0002;
  let onH = uHatAges.x < 0.0002;
  // The clock only steps in when no drum at all is playing: its ribbons
  // are scenery, and must not be taken for a drum's.
  let quiet = min(uKickAges.x, min(uSnareAges.x, uHatAges.x)) > QUIET;
  let ckL = isTick && even && quiet;
  let ckH = isTick && !even && quiet;
  var w = 0.0;
  if (ckL) { w = 1.0; }
  if (ckH) { w = 2.0; }
  return vec4f(f32(onL || ckL), f32(onM), f32(onH || ckH), w);
}

// What a memory cell should hold after this frame.
fn stash(c: i32, rb: i32, uvx: f32) -> vec3f {
  let me = cell(c, rb);
  var ptr = pointers();
  let ev = events(decI(cell(NSLOT + 1, ROW_META).r));
  var evv = ev.xyz;
  // A slot is only overwritten once its ribbon has flown out of sight; if
  // the ring is full the event goes without one, and nothing ever pops.
  // Flying slowly, a ribbon is in sight for longer and fewer events can have
  // one. A ring then also waits until the last ribbon it let go is `room`
  // (shared out over its slots) on its way, so that they leave evenly spaced
  // rather than a ring-full at a time. (At full reactivity `life` is LIFE
  // and `room` is zero, which is no wait at all.)
  let life = LIFE / flight();
  let room = ROOM * slack() * life;
  var fire = vec3f(0.0);
  for (var b = 0; b < 3; b++) {
    let len = select(6, 4, b == 1);
    let prior = ringStart(b) + (i32(ptr[b]) + len - 1) % len;
    if (evv[b] > 0.5 && slotAt(ringStart(b) + i32(ptr[b])).age > life
        && slotAt(prior).age >= room / f32(len)) {
      fire[b] = 1.0;
    }
  }
  let softL = ev.w > 0.5 && ev.w < 1.5;
  let softH = ev.w > 1.5;

  if (rb < ROW_META) {
    let s = rb * 2 + i32(uvx >= 0.5);
    let b = kindOf(s);
    if (fire[b] > 0.5 && ringStart(b) + i32(ptr[b]) == s) {
      // Freeze the waveform: spine, twist, and the spine a quarter-turn on
      // (its slope), which makes the ribbon roll like a helix.
      let u = fract(uvx * 2.0);
      let quad = 0.1 * (lowAt(u + 6.0 / 512.0) - lowAt(u - 6.0 / 512.0)) * 512.0 / 12.0;
      var t = vec3f(0.0);
      if (b == 2 && !softH) {
        let bd = bandsHigh(u);
        t = vec3f(bd.x, bd.y * 4.0, quad);
      } else {
        let bd = bandsAt(u);
        t = vec3f(bd.x, bd.y * 2.0, quad);
      }
      return vec3f(encW(t.x), encW(t.y), encW(t.z));
    }
    return me;
  }

  if (c < NSLOT) {
    let b = kindOf(c);
    if (fire[b] > 0.5 && ringStart(b) + i32(ptr[b]) == c) {
      let pb = fmod(uPhase, 64.0);
      var soft = 0.0;
      if ((b == 0 && softL) || (b == 2 && softH)) { soft = 1.0; }
      return vec3f(enc01(fract(pb)), encI(floor(pb) + 64.0 * soft), enc01(hueNow()));
    }
    // Long dead: forget it, before its age wraps round.
    if (slotAt(c).age > 16.0) { return vec3f(0.0); }
    return me;
  }
  if (c == NSLOT) {
    let np = ptr + fire;
    let len = vec3f(6.0, 4.0, 6.0);
    let wrapped = np - len * step(len - 0.5, np);
    return vec3f(encI(wrapped.x), encI(wrapped.y), encI(wrapped.z));
  }
  if (c == NSLOT + 1) {
    return vec3f(encI(fmod(floor(uPhase / tickLen()), 64.0)), 0.0, 0.0);
  }
  if (c >= LIST0 && c < LIST0 + NSLOT) {
    // The draw list: entry n names the n-th youngest live ribbon (plus one;
    // zero for none), so every pixel can paint them oldest-last without
    // sorting anything itself.
    let n = c - LIST0;
    var found = 0.0;
    for (var a = 0; a < NSLOT; a++) {
      let aa = slotAt(a).age;
      if (aa >= life) { continue; }
      var rank = 0;
      for (var o = 0; o < NSLOT; o++) {
        let ao = slotAt(o).age;
        if (ao < aa || (ao == aa && o < a)) { rank += 1; }
      }
      if (rank == n) { found = f32(a) + 1.0; }
    }
    return vec3f(encI(found), 0.0, 0.0);
  }
  return vec3f(0.0);
}

// ---- drawing -------------------------------------------------------------------

fn segDist(p: vec2f, a: vec2f, b: vec2f) -> f32 {
  let pa = p - a;
  let ba = b - a;
  let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
  return length(pa - ba * h);
}

// One ribbon, laid over `col`: the band between a trace (height Yt, slope mt)
// and its mirror image (Ym, mm). It turns over wherever the two cross, and
// shows its other face there. Y is this pixel's height in the same frame.
// cA and cB colour its two edge lines, fA and fB its two faces. `vis` fades
// the whole ribbon, `open` only its body and far edge, and `hot` is the
// brightness of the trace itself.
fn ribbonLook(col: vec3f, Y: f32, Yt: f32, mt: f32, Ym: f32, mm: f32, px: f32, r: f32,
              cA: vec3f, cB: vec3f, fA: vec3f, fB: vec3f,
              vis: f32, open: f32, hot: f32, nth: f32) -> vec3f {
  let invt = inverseSqrt(1.0 + mt * mt);
  let invm = inverseSqrt(1.0 + mm * mm);
  let wh = 0.5 * (Yt - Ym);
  let cc = 0.5 * (Yt + Ym);
  let front = wh >= 0.0;
  let sg = select(-1.0, 1.0, front);
  let it = (Yt - Y) * sg * invt;
  let im = (Y - Ym) * sg * invm;
  let inside = min(it, im);
  let cov = clamp(inside / px + 0.5, 0.0, 1.0);
  let s = clamp((Y - cc) / (wh + sg * 1e-5), -1.0, 1.0);

  // A dark casing, so whatever this ribbon crosses reads as passing under.
  let casing = 1.0 - sstep(r + px, r * 3.0 + 4.0 * px, -inside);
  var o = col * (1.0 - 0.7 * casing * vis * open);

  // Threads along the ribbon, fading out where they would crowd a pixel.
  let ms = 0.5 * (mt + mm) + s * 0.5 * (mt - mm);
  let sp = abs(wh) * (2.0 / nth) * inverseSqrt(1.0 + ms * ms) / px;
  let tv = sstep(2.5, 6.0, sp);
  let th = mix(0.27, pow(0.5 + 0.5 * cos(PI * nth * s), 4.0), tv);

  let tint = select(fB, fA, front);
  let body = tint * (0.04 + 0.26 * th);
  o = mix(o, body, cov * 0.75 * vis * open);

  let e1 = clamp((r - abs(Y - Yt) * invt) / px + 0.5, 0.0, 1.0);
  let e2 = clamp((r - abs(Y - Ym) * invm) / px + 0.5, 0.0, 1.0);
  o = mix(o, cB * 0.7, e2 * vis * open);
  o = mix(o, cA * hot, e1 * vis);
  return o;
}

fn scene(uv: vec2f) -> vec3f {
  let pix = floor(uv * uRes);
  let fromBottom = uRes.y - 1.0 - pix.y;
  if (fromBottom < f32(BOTTOM_ROWS)) {
    return stash(i32(pix.x), i32(fromBottom), uv.x);
  }
  if (pix.y < f32(TOP_ROWS)) {
    return stash(i32(pix.x), i32(pix.y) + BOTTOM_ROWS, uv.x);
  }

  let p = aspect(uv);
  let ar = uRes.x / uRes.y;
  let px = 2.0 / uRes.y;
  let spd = flight();
  let kick = kickSum();
  let voiced = 1.0 - uIdle;            // idle, everything is scenery
  // This frame's step of the tunnel: of uPhase, its clock, at flight speed.
  let dph = (0.3 + 1.1 * uLevel + 1.4 * uBass * uBass) * uDt * spd;
  // Twist per unit of age: a slow sway, wound up by a bright sound.
  let tw = 0.10 * sin(uTime * 0.13) + 0.7 * calm(0.5) * (uCentroid - 0.38);
  let Ah = heroAmp();
  let keep = lowKeep();
  let yP = lift();
  // The tunnel bends away to one side, slowly swinging round.
  let bend = 0.20 * vec2f(cos(uTime * 0.17), sin(uTime * 0.17));

  // ---- afterglow: the last frame, streaming outwards faster than the rings.
  // Each pixel reaches back a slightly different distance each frame, which
  // smears the trails smooth instead of leaving a comb of echoes ------------
  var col = vec3f(0.0);
  let rq = rot(tw * dph) * p;
  // (interleaved gradient noise: evenly spread over neighbouring pixels,
  // stepped on each frame, so the smear stays fine-grained)
  let jit = fract(52.9829189 * fract(dot(pix, vec2f(0.06711056, 0.00583715))) + uTime * 37.1);
  for (var i = 0; i < 3; i++) {
    let c = prev(unaspect(rq * exp(-KZ * (1.3 + 0.6 * (f32(i) + jit)) * dph)));
    let L = max(c.r, max(c.g, c.b));
    // Dim things trail as they are; bright lines leave only a faint glow,
    // and the live trace (brightest of all) next to nothing. (Fainter the
    // slower the flight: the same glow, laid down over more frames.)
    col += c * min(1.0, 0.18 * spd / L) * pow(1.0 + L * L, -FR);
  }
  // The glow fades with distance travelled, and faster when the mix is thin
  // (per frame of flight, so a streak is as long however slowly it is laid).
  col *= exp(-dph / 0.2) * pow(0.97 * (0.4 + 0.5 * uEnergy), FR * spd) / 3.0;

  // ---- how far out the tunnel reaches just now: the radius of its
  // outermost ring still in sight (at most 1, the edge of the picture). The
  // rings out there are the BASS's. (A ring counts by how far it has closed
  // and how far it has yet to fade, so the answer never jumps.) -------------
  var edge = 0.25;
  for (var n = 0; n < NSLOT; n++) {
    let slot = i32(decI(cell(LIST0 + n, ROW_META).r)) - 1;
    if (slot < 0) { break; }
    let d = slotAt(slot).age * spd;
    if (d >= LIFE) { continue; }
    let rc = HLF * exp(KZ * d) / PI;
    edge = max(edge, min(rc, 1.0) * sstep(0.0, D1, d) * (1.0 - sstep(1.35, 2.2, rc)));
  }

  // ---- ribbons, youngest first so the oldest (nearest) end up on top ------
  let crack = snareSum();
  let bassC = voiceCol(BASS);
  let hatC = voiceCol(HATS);
  for (var n = 0; n < NSLOT; n++) {
    let slot = i32(decI(cell(LIST0 + n, ROW_META).r)) - 1;
    if (slot < 0) { break; }
    let sl = slotAt(slot);
    let d = sl.age * spd;              // its age as flown: everything below runs on this
    if (d >= LIFE) { continue; }
    let kind = kindOf(slot);
    let hard = sl.soft < 0.5;          // frozen by a drum, not let go by the clock

    // The ribbon is an arc of a circle: flat at birth (g = 0), a closed
    // ring centred on the tunnel once g reaches 1, growing all the while.
    let g = sstep(0.0, D1, d);
    let hl = mix(ar, HLF, g) * exp(KZ * d);   // half-length along the arc
    let Rr = hl / PI;                         // radius once it has closed
    let kap = g / Rr;                         // curvature now
    let th = tw * d;
    let cs = cos(th);
    let sn = sin(th);
    // KICK: every ring is shoved outwards from the centre, heavily. (A
    // ribbon still lying on the trace is not: it leaves from where the
    // trace is, which is where the tune's pitch has put it.)
    let ps = p / (1.0 + PUSH * kick * g) - vec2f(0.0, yP * (1.0 - g));
    let pc = ps - bend * pow(max(Rr - 0.3, 0.0), 1.3) * g;
    var pr = vec2f(cs * pc.x + sn * pc.y, -sn * pc.x + cs * pc.y);
    var flip = 1.0;
    if (kind != 0) {   // kicks sag and curl up round the trace; the rest rise and curl down
      pr.y = -pr.y;
      flip = -1.0;
    }
    // Y: distance inwards from the arc; X: distance along it. Both stay
    // well behaved as the curvature goes to zero.
    let pp = vec2f(pr.x, pr.y + g * Rr);
    let t = 1.0 - kap * pp.y;
    let kr = sqrt(kap * kap * pp.x * pp.x + t * t);
    let Y = (2.0 * pp.y - kap * dot(pp, pp)) / (1.0 + kr);

    // Per kind: spine and twist amplitude (as a share of the radius), the
    // gains that fatten the twist once it has left the trace, threads, and
    // `heft`: half-width it has whatever the waveform was.
    var a_s = 0.10;
    var a_t = 0.16;
    var gain = vec3f(1.0, 0.5, 1.0);     // undoes the scaling the tracks were stored with
    var fat = vec2f(3.0, 1.4);           // twist gain, quarter-turn gain, once a ring
    var nth = 5.0;
    var heft = 0.0;
    var swell = 0.0;                     // how much wider it is drawn just now
    if (hard) {
      // KICK: the fat one, and every kick swells it.
      if (kind == 0) { heft = 0.2 + 0.35 * kick; swell = 0.35 * kick; }
      // SNARE: the slim, straight one.
      if (kind == 1) { a_s = 0.035; a_t = 0.09; nth = 3.0; fat = vec2f(2.5, 0.5); }
      // HATS: thin lace.
      if (kind == 2) { a_s = 0.05; a_t = 0.10; nth = 3.0; gain = vec3f(1.0, 0.25, 1.0); fat = vec2f(8.0, 0.25); }
    }
    // BASS: the outermost rings put on weight with the bass line.
    let outer = sstep(0.5 * edge, edge, HLF * exp(KZ * d) / PI * g);
    let mass = outer * uBassLine;
    heft += 0.1 * mass;
    swell += 0.4 * mass;
    let As = mix(Ah * keep, a_s * Rr, g);
    let At = mix(Ah, a_t * Rr * (1.0 + swell), g);
    let r = max(0.8 * px, 0.0016) * (0.8 + 0.5 * Rr);   // half-width of its edge lines
    let reach = 3.0 * r + 5.0 * px;                      // ... and of its dark casing
    let hat = kind == 2 && hard;
    let fling = select(0.0, 0.3 * calm(0.5) * Rr, hat);  // how far a hat throws its sparks
    if (abs(Y) > As + At + reach + fling) { continue; }

    var X = pp.x / t;
    if (kap > 1e-3) { X = atan2(kap * pp.x, t) / kap; }
    let u = X / (2.0 * hl) + 0.5;
    if (u <= 0.0 || u >= 1.0) { continue; }

    // The frozen waveform either side of this pixel, for height and slope.
    let e = 0.5 / 512.0;
    let row = rowV(slot / 2);
    let h0 = f32(slot % 2) * 0.5;
    let guard = 1.0 / uRes.x;
    let gt = mix(heroFuzz(), fat.x, g);   // at birth, as fuzzy as the live trace is drawn
    let gq = fat.y * g;
    let gh = heft * g;
    var Ta = decW3(prev(vec2f(h0 + 0.5 * clamp(u - e, guard, 1.0 - guard), row)));
    var Tb = decW3(prev(vec2f(h0 + 0.5 * clamp(u + e, guard, 1.0 - guard), row)));
    let win = sstep(0.03, 0.11, u) * sstep(0.97, 0.89, u);
    if (win < 1.0) {
      // Near the ends, ease into the mean of the two, so that when the
      // ribbon closes into a ring it closes without a seam.
      let ends = 0.5 * (decW3(prev(vec2f(h0 + 0.035, row))) + decW3(prev(vec2f(h0 + 0.465, row))));
      Ta = mix(ends, Ta, win);
      Tb = mix(ends, Tb, win);
    }
    Ta *= gain * flip;
    Tb *= gain * flip;
    let wa = Ta.y * gt + Ta.z * gq + gh;
    let wb = Tb.y * gt + Tb.z * gq + gh;
    // The loose ends taper to a point; once they have met, the seam heals.
    let tip = mix(sstep(0.0, 0.05, u) * sstep(1.0, 0.95, u), 1.0, sstep(D1, D1 + 0.5, d));
    // x: spine, y: half-width (soft-limited).
    let va = vec2f(Ta.x * As, wa * inverseSqrt(1.0 + wa * wa) * At * tip);
    let vb = vec2f(Tb.x * As, wb * inverseSqrt(1.0 + wb * wb) * At * tip);
    let v = 0.5 * (va + vb);
    let dv = (vb - va) / (2.0 * e * 2.0 * hl * kr);

    let Yl = Y * flip;
    let Yt = v.x + v.y;     // the trace as it was (at birth, exactly the live one)
    let Ym = v.x - v.y;     // its mirror image about the spine
    let mt = dv.x + dv.y;
    let mm = dv.x - dv.y;

    let vis = 1.0 - sstep(1.35, 2.2, Rr);
    // It leaves the trace as a bare bright line, and opens out into a
    // ribbon, cooling, as it curls.
    let open = sstep(0.3, 1.0, g);
    let hot = mix(2.0, 1.3, sstep(0.2, 0.8, g));
    let lit = mix(1.0, 0.5, sstep(0.3, 2.0, Rr));

    // Its colour: the voice that froze it, for as long as it lives. One the
    // clock let go is scenery: the chord's place in the palette, and dimmer.
    var cA = vec3f(0.0);
    var cB = vec3f(0.0);
    if (hard) {
      let vc = voiceCol(kind);           // kinds 0, 1, 2 are KICK, SNARE, HATS
      // KICK: its ribbons glow with every kick, slowly. SNARE: its ribbons
      // flash with every snare, and are back at once.
      var glow = 1.0;
      if (kind == 0) { glow += 0.8 * kick; }
      if (kind == 1) { glow += 1.5 * crack; }
      cA = vc * lit * glow;
      cB = vc * lit * glow * 0.45;
    } else {
      let hue = sl.hue + 0.12 * (u - 0.5);
      let dim = mix(0.4, 1.0, uIdle);
      cA = pal(hue) * lit * dim * mix(1.2 / hot, 1.0, uIdle);   // nor does it leave white-hot
      cB = pal(hue + 0.5) * lit * dim;
    }
    // BASS: ... and their faces fill with its colour.
    let fill = mass * voiced;
    let fA = mix(cA, bassC * 1.9 * lit, fill);
    let fB = mix(cB, bassC * 0.9 * lit, fill);

    // Clear of the ribbon and its casing: nothing of it to draw here.
    if (max(min(Yt, Ym) - Yl, Yl - max(Yt, Ym)) * inverseSqrt(1.0 + max(mt * mt, mm * mm)) <= reach) {
      var glint = 1.0;
      if (hat) {
        // HATS: beads of light along the lace. Every hat makes some of
        // them tick, a different handful each time.
        let gc = u * 96.0;
        let bead = floor(gc) + 131.0 * f32(slot);
        var tick = 0.0;
        for (var k = 0; k < 4; k++) {
          tick += hatEnv(k) * step(0.72, hash21(vec2f(bead, uHatN - f32(k))));
        }
        glint += 6.0 * min(tick, 1.0) * sstep(0.45, 0.1, abs(fract(gc) - 0.5)) * mix(0.5, 1.0, outer);
      }
      // (A bead that is lit is the hat's own colour, whatever fills the face.)
      col = ribbonLook(col, Yl, Yt, mt, Ym, mm, px, r, cA * glint, cB,
                       mix(cA * glint, fA, 1.0 / glint), fB, vis, open, hot, nth);
    }

    if (hat) {
      // ... and sparks, thrown outwards off the trace as it peels away,
      // that catch the light again on each hat that follows.
      let ci = floor(u * 40.0);
      let hs = hash22(vec2f(ci, f32(slot) * 7.0 + floor(sl.hue * 61.0)));
      if (hs.x > 0.35) {
        let us = (ci + 0.3 + 0.4 * hs.y) / 40.0;
        let Ts = decW3(prev(vec2f(h0 + 0.5 * clamp(us, guard, 1.0 - guard), row))) * gain * flip;
        let wsp = Ts.y * gt + Ts.z * gq + gh;
        let y0 = Ts.x * As + wsp * inverseSqrt(1.0 + wsp * wsp) * At * tip;
        let ys = y0 - flip * fling * (0.25 + 0.75 * fract(hs.x * 7.7)) * (1.0 - exp(-1.8 * d));
        let dd = length(vec2f((u - us) * 2.0 * hl * kr, Yl - ys));
        let rad = (1.0 + 1.2 * hs.y) * max(px, 0.0014) * (0.8 + 0.5 * Rr);
        let life = sstep(0.02, 0.12, d) * (1.0 - sstep(0.6, 1.3 + hs.y, d));
        var tick = 0.0;
        for (var k = 0; k < 4; k++) {
          tick += hatEnv(k) * step(0.4, hash21(vec2f(ci + 57.0 * f32(slot), uHatN - f32(k))));
        }
        let spark = clamp((rad - dd) / px + 0.5, 0.0, 1.0) * life * (0.3 + 0.7 * min(1.5 * tick, 1.0));
        col = mix(col, hatC * 3.0, spark);
      }
    }
  }

  // ---- TUNE: the live trace, over the ribbons. It sits where the pitch of
  // the tune puts it, in the tune's colour; with no tune playing it is
  // scenery, dim and all but colourless ------------------------------------
  let band = min(0.62, 1.4 * Ah + 0.04);
  if (abs(p.y - yP) < band) {
    // Distance to the curve through three points a pixel or so apart: even
    // thickness on steep transients, clean corners at peaks.
    let dxs = 1.25 * px;
    let du = dxs / (2.0 * ar);
    var ya = waveS(uv.x - du);
    var yb = waveS(uv.x);
    var yc = waveS(uv.x + du);
    let fuzz = heroFuzz();
    if (fuzz < 1.0) {
      // Turned down, the trace keeps its shape and loses some of its fuzz.
      let sa = softAt(uv.x - du);
      let sc = softAt(uv.x + du);
      ya = mix(sa, ya, fuzz);
      yb = mix(0.5 * (sa + sc), yb, fuzz);
      yc = mix(sc, yc, fuzz);
    }
    // Most of the low end is left out. It lies within -1..1, so it only
    // needs reading where taking it out could bring the trace this near.
    let r0 = max(1.5 * px, 0.0034);
    let yq = (p.y - yP) / Ah;
    let near = (1.0 - keep) + (1.4 * r0 + 8.0 * px) / Ah;
    if (yq > min(ya, min(yb, yc)) - near && yq < max(ya, max(yb, yc)) + near) {
      // (It changes too slowly along the trace to need more than one reading.)
      let lo = (1.0 - keep) * lowAt(uv.x);
      ya = (ya - lo) * Ah + yP;
      yb = (yb - lo) * Ah + yP;
      yc = (yc - lo) * Ah + yP;
      let d = min(segDist(p, vec2f(p.x - dxs, ya), vec2f(p.x, yb)),
                  segDist(p, vec2f(p.x, yb), vec2f(p.x + dxs, yc)));
      // Which notes: the twelve pitch classes lie along the trace, C at the
      // left, and the ones sounding are the stretches that run white-hot. A
      // note starting plucks them: hotter and a touch thicker for a moment.
      let tn = tuneIn();
      let pluck = noteSum();
      let glow = max(tn * min(chromaAt(uv.x) * (1.4 + pluck) + 0.25 * pluck, 1.0), 0.75 * uIdle);
      let r = r0 * (1.0 + 0.4 * glow * tn);
      // (With music playing but no tune, the scenery's trace is dim and
      // only faintly the chord's colour, so that it is not taken for a voice.)
      let chord = pal(hueNow() + 0.12 * (uv.x - 0.5));
      let scenery = mix(vec3f(dot(chord, vec3f(0.3, 0.5, 0.2))), chord, mix(0.3, 1.0, uIdle))
                  * mix(0.35, 1.0, uIdle);
      let tint = mix(scenery, voiceCol(TUNE), tn);
      col *= 1.0 - 0.75 * (1.0 - sstep(r + px, r + 7.0 * px, d));
      let core = clamp((r - d) / px + 0.5, 0.0, 1.0);
      let inner = clamp((0.4 * r - d) / px + 0.5, 0.0, 1.0);
      let heat = calm(0.6);   // white-hot at full reactivity, a little cooler below
      col = mix(col, tint * heat * (1.1 + 1.0 * glow), core);
      col = mix(col, tint * heat * (1.3 + 3.0 * glow), inner);
    }
  }

  // ---- SNARE: the crack. A straight line across the picture, there at once
  // and gone in an instant, where the trace lies (and from where, when the
  // snare gets a ribbon, the ribbon lifts off). Over the trace: for that
  // instant it is the brightest thing there is ------------------------------
  if (crack > 0.002) {
    let dc = abs(p.y - yP);
    let rc = max(1.5 * px, 0.0034) * (0.6 + crack);
    let line = clamp((rc - dc) / px + 0.5, 0.0, 1.0);
    let halo = exp(-dc * dc / (0.0015 + 0.004 * crack));
    let snareC = voiceCol(SNARE);
    col += snareC * 0.5 * crack * halo;
    col = mix(col, snareC * 2.4, line * min(2.0 * crack, 1.0));
  }

  // Fade into the memory rows, so they are never a visible edge.
  return col * sstep(4.0, 18.0, min(pix.y, fromBottom));
}
