// Taffy: ink dropped into a slow curl-noise current, pulled like warm candy.
//
// The music is a paper marbler working over a dark bath. Pure feedback: each
// frame moves the previous one and adds fresh ink. Every move is one of the
// marbler's real operations, written as "where did the ink now at this pixel
// sit a frame ago", so new ink pushes old ink aside instead of covering it,
// and what is left on the bath is a record of what was played.
//
// Each of the five voices has one tool, one ink and one part of the bath:
//
//   voice  what it does                            where               ink
//   -----  --------------------------------------  ------------------  ---------------
//   KICK   a fat drop of ink. It blooms at the     the middle of the   voiceCol(KICK)
//          pace of kickEnv (a heavy push, a slow   bath and just
//          release) and shoves all that is on the  below it; the
//          bath outwards, area for area. Its size  biggest thing on
//          is how hard the kick was. Four kicks    the bath
//          nest into a bullseye, then the marbler
//          moves on. A kick after a long gap (the
//          drop) lands much bigger, and slower.
//   SNARE  the stylus: one straight hair of ink,   one long steep      voiceCol(SNARE)
//          blazing for an instant, and a tug       slash through the
//          along it at the pace of snareEnv (a     middle of the bath
//          crack) that feathers the ink it cuts.
//   HATS   a flick of the brush: a short streak    along the top       voiceCol(HATS)
//          of pin-point droplets, each a hot ring  edge; the finest
//          round a clear eye, landed whole in one  thing on the bath
//          frame (a tick). Harder hat, fatter
//          droplets.
//   BASS   the tide: a body of ink comes up over   the bottom rim, in  voiceCol(BASS)
//          the bottom rim and lifts the lower      a low dome; the
//          bath with it, as high as the bass line  lowest mass
//          is strong (uBassLine), and sinks back
//          out when the bass stops. Nothing else
//          can shove it, and it does not fade
//          while it stands. The bass also
//          quickens the current low down, which
//          pulls wisps off the tide.
//   TUNE   the pipette: a thread trailed across    the middle, at the  voiceCol(TUNE)
//          the bath. Its weight is how much tune   height of the
//          there is (uTune; none, and it lifts     pitch (uPitch),
//          clear), and every note swells and       to which it glides
//          heats it (noteEnv).
//
// Ink lands hot, in its voice's colour, and cools in a moment to a body that
// sinks slowly to a faint stain: the record. Nothing is inked, and nothing
// moved, by a voice that is not playing.
//
//   chord    cannot pick the ink any more (the colour says who played), so
//            it shows in the banding: the rings of a kick's drop step round
//            the twelve pitch classes and each is as bright as its notes
//            (chroma). With no tune sounding the rings are all alike.
//   scenery  the slow current never stops. A full mix (uEnergy) fades ink
//            quickly so the bath never silts up; in a breakdown the picture
//            is left to hang. When a full mix turns to hiss (uCentroid and
//            uEnergy both high: a riser) the bath above the tide winds into
//            a whirlpool.
//   silence  the marbler works on alone, slowly, on its own clock: drops,
//            strokes and flicks in the palette's own colours (pal), which
//            follow the idle harmony, and a thread that wanders with the
//            idle tune. No tide.
//   uReact   how hard the marbler works. The envelopes already follow it (a
//            smaller drop that swells in more slowly, a shorter tug); here
//            it also sets how high the tide comes, how fat the droplets
//            are, how hot fresh ink lands, and the pace of the current and
//            of the fade, so the bath stays as full. At 1 nothing is scaled.
//
// One pixel, the top-left one, is not ink but memory, kept far too dim to
// see: where the tide and the pipette's tip stood a frame ago. Both follow
// the music, and a tide cannot rise and fall back, nor a thread be laid
// unbroken, without knowing where it last was.

const IDLE_P: f32 = 2.7; // seconds between drops when nothing is playing
const IDLE_Q: f32 = 4.3; // and between its strokes of the stylus
const IDLE_H: f32 = 1.9; // and its flicks of the brush
const RINGS: f32 = 7.0; // rings of ink in an ordinary kick's drop
const DRAG: f32 = 0.36; // how far a full snare tugs the ink along the stylus
const TIDE: f32 = 0.55; // how high a full bass line lifts the tide
const TIDE_TOP: f32 = 0.3; // the height above which the tide lifts nothing
const BAND: f32 = 0.05; // the depth of one band of the tide's ink
const WELL: f32 = 0.12; // how fast ink wells up over the rim, per second
const GLIDE: f32 = 5.0; // how fast the pipette follows the pitch, per second

// The shapes of the shared envelopes (see hitEnv, kickEnv and snareEnv in
// the header): seconds in at full reactivity, and seconds out.
const KICK_IN: f32 = 0.06;
const KICK_OUT: f32 = 0.20;
const SNARE_IN: f32 = 0.012;
const SNARE_OUT: f32 = 0.055;

// Reactivity (uReact). What is left of each thing at uReact = 0; at 1 all of
// it is, and the default, 0.5, is half way between.
// The bath's pace: the speed of the current and of the fade.
const BATH0: f32 = 0.45;
const BIG0: f32 = 0.40; // how much bigger a kick after a long gap lands
const HAT0: f32 = 0.60; // the radius of a hat's droplets
const HOT0: f32 = 0.65; // how hot fresh ink lands
const TIDE0: f32 = 0.50; // how high the tide comes

// The memory pixel holds whole numbers from 0 to 1023, each as a value
// between MEM and twice MEM: far too dim to see, and exactly what a 16-bit
// float can hold, so what is read back is what was written.
const MEM: f32 = 0.00048828125;

// Value noise with its analytic gradient (quintic, so the flow it makes has
// no creases along the lattice). Returns (value, d/dx, d/dy).
fn noised(p: vec2f) -> vec3f {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
  let a = hash21(i);
  let b = hash21(i + vec2f(1.0, 0.0));
  let c = hash21(i + vec2f(0.0, 1.0));
  let d = hash21(i + vec2f(1.0, 1.0));
  let k1 = b - a;
  let k2 = c - a;
  let k4 = a - b - c + d;
  return vec3f(a + k1 * u.x + k2 * u.y + k4 * u.x * u.y, du.x * (k1 + k4 * u.y), du.y * (k2 + k4 * u.x));
}

// One octave of divergence-free flow: the curl of a rotated noise potential.
// Returns the flow in xy and the potential itself (about zero) in z.
fn eddy(p: vec2f, m: mat2x2f, freq: f32, drift: vec2f) -> vec3f {
  let n = noised(m * p * freq + drift);
  let g = transpose(m) * n.yz;
  return vec3f(g.y * freq, -g.x * freq, n.x - 0.5);
}

// How much of the bass's pull on the current is felt at height y (all of it
// in the lower part of the bath, none near the top), and its slope.
fn low(y: f32) -> vec2f {
  let t = clamp((y + 0.8) / 1.2, 0.0, 1.0);
  return vec2f(1.0 - t * t * (3.0 - 2.0 * t), -6.0 * t * (1.0 - t) / 1.2);
}

// The current: two big slow eddies, at pace `big` everywhere plus `deep`
// more low down, and two fine ones at pace `fine`.
fn flow(p: vec2f, big: f32, deep: f32, fine: f32) -> vec2f {
  let t = uTime;
  let e = eddy(p, rot(0.0), 0.55, vec2f(t * 0.021, -t * 0.013)) * 0.26
    + eddy(p, rot(0.9), 1.3, vec2f(31.7 - t * 0.03, 11.3 + t * 0.024)) * 0.085;
  // Where the pace varies with height it scales the potential, not the
  // flow: that keeps the current free of sources and sinks.
  let l = low(p.y);
  var v = e.xy * (big + deep * l.x) + vec2f(e.z * deep * l.y, 0.0);
  v += eddy(p, rot(2.1), 3.1, vec2f(7.1 + t * 0.05, 51.9 + t * 0.04)).xy * 0.016 * fine;
  v += eddy(p, rot(3.7), 6.7, vec2f(77.7 - t * 0.07, 3.3 + t * 0.09)).xy * 0.0028 * fine;
  // It runs along the bottom rim, never across it: what comes up over that
  // rim is the bass's to say.
  v.y *= sstep(-1.0, -0.85, p.y);
  return v;
}

// How much of its push a hit has delivered by `age`, 0..1: the running
// total of the header's envelope (hitEnv with this attack and release), so
// that whatever it measures out moves at the pace of the envelope itself.
fn pushed(age: f32, attack: f32, release: f32) -> f32 {
  let rise = attack * (1.0 + 2.5 * (1.0 - uReact));
  let t = max(age, 0.0);
  let x = min(t / rise, 1.0);
  let sum = rise * x * x * x * (1.0 - 0.5 * x) + release * (1.0 - exp(-max(t - rise, 0.0) / release));
  return sum / (0.5 * rise + release);
}

// The tide lifts the bath below TIDE_TOP: by `s` at the bottom rim (and all
// that is under it), by nothing at TIDE_TOP, smoothly between. tideUp takes
// a height on the resting bath to where the tide shows it; tideDown is its
// exact inverse. (s must stay under half the depth that is lifted.)
fn tideUp(y: f32, s: f32) -> f32 {
  if (y >= TIDE_TOP) {
    return y;
  }
  if (y <= -1.0) {
    return y + s;
  }
  let u = (TIDE_TOP - y) / (TIDE_TOP + 1.0);
  return y + s * u * u;
}

fn tideDown(y: f32, s: f32) -> f32 {
  if (y >= TIDE_TOP) {
    return y;
  }
  if (y <= s - 1.0) {
    return y - s;
  }
  let d = TIDE_TOP + 1.0;
  let g = TIDE_TOP - y;
  return TIDE_TOP - d * 2.0 * g / (d + sqrt(max(d * d - 4.0 * s * g, 0.0)));
}

// The tide comes in as a low dome: highest in the middle of the rim.
fn dome(x: f32, ar: f32) -> f32 {
  let c = cos(clamp(x / (ar * 1.15), -1.0, 1.0) * PI * 0.5);
  return (0.55 + 0.45 * c * c) * (1.0 + 0.1 * sin(x * 2.3 + 0.7) + 0.05 * sin(x * 5.1 + 2.0));
}

// Catmull-Rom sample of the previous frame (9 bilinear taps). Repeated
// bilinear resampling is what turns advected ink to fog; this keeps edges
// crisp for many seconds.
fn prevSharp(uv: vec2f) -> vec3f {
  let sp = uv * uRes;
  let tc = floor(sp - 0.5) + 0.5;
  let f = sp - tc;
  let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
  let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
  let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
  let w3 = f * f * (-0.5 + 0.5 * f);
  let w12 = w1 + w2;
  let t0 = (tc - 1.0) / uRes;
  let t3 = (tc + 2.0) / uRes;
  let t12 = (tc + w2 / w12) / uRes;
  var c = prev(vec2f(t0.x, t0.y)) * (w0.x * w0.y);
  c += prev(vec2f(t12.x, t0.y)) * (w12.x * w0.y);
  c += prev(vec2f(t3.x, t0.y)) * (w3.x * w0.y);
  c += prev(vec2f(t0.x, t12.y)) * (w0.x * w12.y);
  c += prev(vec2f(t12.x, t12.y)) * (w12.x * w12.y);
  c += prev(vec2f(t3.x, t12.y)) * (w3.x * w12.y);
  c += prev(vec2f(t0.x, t3.y)) * (w0.x * w3.y);
  c += prev(vec2f(t12.x, t3.y)) * (w12.x * w3.y);
  c += prev(vec2f(t3.x, t3.y)) * (w3.x * w3.y);
  // Never beyond the four texels around the sample. Without this the
  // kernel's overshoot feeds on itself, frame after frame, and thin lines
  // of ink brighten and break into beads.
  let b = vec2i(tc);
  let top = vec2i(textureDimensions(tPrev)) - vec2i(1, 1);
  let a00 = textureLoad(tPrev, clamp(b, vec2i(0, 0), top), 0).rgb;
  let a10 = textureLoad(tPrev, clamp(b + vec2i(1, 0), vec2i(0, 0), top), 0).rgb;
  let a01 = textureLoad(tPrev, clamp(b + vec2i(0, 1), vec2i(0, 0), top), 0).rgb;
  let a11 = textureLoad(tPrev, clamp(b + vec2i(1, 1), vec2i(0, 0), top), 0).rgb;
  return clamp(c, min(min(a00, a10), min(a01, a11)), max(max(a00, a10), max(a01, a11)));
}

// How far across the bath the pipette's tip is at time t.
fn penX(t: f32, ar: f32) -> f32 {
  return ar * 0.72 * sin(t * 0.37 + 1.0);
}

fn scene(uv: vec2f) -> vec3f {
  let p = aspect(uv);
  let ar = uRes.x / uRes.y;
  let px = 2.0 / uRes.y;
  let dt = uDt;
  let hue = uHarmony * 3.0;
  // Reactivity. At full reactivity these are all 1.
  let bath = mix(BATH0, 1.0, uReact);
  let hot = mix(HOT0, 1.0, uReact);
  // The height of a full hit's envelope (as in the header's hitEnv).
  let reach = mix(0.35, 1.0, uReact);

  // ---- memory -----------------------------------------------------------
  // Last frame's tide, and the height of the pipette's tip (two digits).
  let mem = round((textureLoad(tPrev, vec2i(0, 0), 0).rgb / MEM - 1.0) * 1024.0);
  let known = min(mem.r, min(mem.g, mem.b)) >= 0.0 && max(mem.r, max(mem.g, mem.b)) <= 1023.0;

  // The pipette's tip: across the bath on its own clock, and as high as the
  // tune. It glides there: the pitch of a full mix flutters with every note
  // and drum, and a thread that followed the flutter would be a scribble.
  let penGoal = -0.45 + uPitch + 0.03 * sin(uTime * 1.3);
  var penWas = penGoal;
  if (known) {
    penWas = (mem.g * 1024.0 + mem.b) / 1048575.0 * 2.0 - 1.0;
  }
  let penN = round(clamp(mix(penGoal, penWas, exp(-dt * GLIDE)) * 0.5 + 0.5, 0.0, 1.0) * 1048575.0);
  let pen = vec2f(penX(uTime, ar), penN / 1048575.0 * 2.0 - 1.0);
  // How far down it is (0 lifted clear, 1 in the bath): no tune, no thread.
  var down = sstep(0.12, 0.3, uTune);

  // The bass line, squared: a full bass line is a great weight of ink and a
  // faint one next to none (a kick's tail or a low chord, with nothing else
  // playing, reads as a faint bass line). And none while idle: the marbler
  // alone pours nothing in.
  let bassLine = uBassLine * uBassLine * (1.0 - uIdle);
  // The tide follows it, a little more slowly, in steps too fine to see
  // that the memory can hold exactly (and by a step at least, so that it
  // always arrives).
  let goalN = mix(TIDE0, 1.0, uReact) * bassLine * 1023.0;
  var wasN = goalN;
  if (known) {
    wasN = mem.r;
  }
  let gapN = goalN - wasN;
  let stepN = max(abs(gapN) * (1.0 - exp(-dt * select(3.0, 6.0, gapN > 0.0))), min(abs(gapN), 1.0));
  let tideN = round(clamp(wasN + sign(gapN) * stepN, 0.0, 1023.0));
  let tide = tideN / 1023.0 * TIDE;
  let tideWas = wasN / 1023.0 * TIDE;

  // q walks backwards through this frame's operations to where the ink now
  // at p sat one frame ago. `fresh` and `keep` composite ink born this frame.
  var q = p;
  var fresh = vec3f(0.0);
  var keep = 1.0;

  // ---- KICK: a fat drop ------------------------------------------------
  let kickLife = KICK_IN * (1.0 + 2.5 * (1.0 - uReact)) + 6.0 * KICK_OUT;
  for (var k = 0; k < 5; k++) {
    var a = 10.0;
    var n = 0.0;
    var gain = 1.0;
    var size = 0.0;
    if (k < 4) {
      a = uKickAges[k];
      n = uKickN - f32(k);
      size = uKickHits[k] * reach;
      // A kick after a long gap (the drop, a lone kick in the intro) lands
      // as a much bigger drop than one in a run of kicks.
      if (k < 3) {
        gain = 1.0 + 2.2 * mix(BIG0, 1.0, uReact) * sstep(0.7, 3.0, uKickAges[k + 1] - a);
      }
    } else {
      // The marbler's own clock, while nothing is playing.
      n = floor(uTime / IDLE_P);
      a = uTime - n * IDLE_P;
      n += 1000.0;
      size = 1.5 * uIdle * reach;
    }
    // A heavier drop is a heavier push: it takes longer over the same
    // envelope, so its edge moves no faster than an ordinary drop's.
    let heavy = sqrt(gain);
    if (a - dt >= kickLife * heavy || size < 0.01) {
      continue;
    }
    // In the middle of the bath or below it. Four share a spot.
    let st = floor(n / 4.0);
    let j = n - st * 4.0;
    let h = fract(vec2f(0.31, 0.57) + st * vec2f(0.7549, 0.5698));
    var c = vec2f((h.x - 0.5) * ar * 0.9, -0.32 + 0.45 * h.y);
    c += 0.09 * vec2f(cos(j * 2.4 + st), sin(j * 2.4 + st));
    // (And clear of the tide, however high it comes.)
    c.y = max(c.y, tide * dome(c.x, ar) - 0.75);
    let rr = 0.34 * (0.75 + 0.5 * hash21(vec2f(n, 3.0)));
    let R2 = rr * rr * gain * size;
    // The bloom moves at the pace of kickEnv: its area so far is the
    // envelope's running total.
    let got = pushed(a / heavy, KICK_IN, KICK_OUT);
    // The pipette lifts clear of a drop that blooms under it, and comes back
    // down as the bloom slows. Ink laid into the first rush of a bloom is
    // torn into blocks as the drop spreads.
    let pd = pen - c;
    down *= 1.0 - (1.0 - sstep(1.0, 1.5, dot(pd, pd) / R2)) * (1.0 - sstep(0.7, 0.98, got));
    let A = R2 * got;
    let dA = A - R2 * pushed((a - dt) / heavy, KICK_IN, KICK_OUT);
    if (dA <= 0.0) {
      continue;
    }
    let d = q - c;
    let r2 = dot(d, d);
    let rho = sqrt(r2);
    let m = clamp((sqrt(dA) - rho) / px + 0.5, 0.0, 1.0);
    if (m > 0.0) {
      // The drop's own coordinate: x is 0 for the first ink out (the rim)
      // and 1 for the last (the centre), linear in the final radius.
      let u = clamp((A - r2) / R2, 0.0, 1.0);
      let sq = sqrt(1.0 - u);
      let x = 1.0 - sq;
      let wx = px * (rho + px) / (R2 * max(sq, 0.05));
      // Rings (seven; more in a bigger drop, fewer in a ghost of one), then
      // a clear eye.
      let rings = floor(RINGS * sqrt(clamp(gain * size / reach, 0.35, 3.2)) + 0.5);
      let t = x * rings / 0.82;
      let i = floor(t);
      let line = clamp(0.5 - (abs(fract(t) - 0.32) - 0.27) / (wx * rings / 0.82), 0.0, 1.0);
      let live = step(t, rings);
      // The kick's ink; the marbler's own drops are in the palette's.
      var tone = voiceCol(KICK);
      var rim = tone;
      if (k == 4) {
        tone = pal(hue + 0.04 * j + 0.175 * i / rings);
        rim = pal(hue + 0.04 * j + 0.12);
      }
      // The chord shows in the banding: the rings stand for the twelve
      // pitch classes in order and each is as bright as its notes are loud.
      // With no tune there is no chord, and the rings are all alike.
      let lit = mix(0.8, 0.3 + 0.7 * chromaAt((i + 0.5) / rings), sstep(0.05, 0.4, uTune));
      // Each ring is brightest at its leading edge and falls away behind it.
      let shade = mix(1.15 * hot, 0.5, clamp((fract(t) - 0.05) / 0.54, 0.0, 1.0));
      var ink = tone * lit * shade * line * live;
      if (i < 0.5) {
        ink = rim * hot * 1.35 * line;
      }
      fresh += keep * m * ink;
      keep *= 1.0 - m;
    }
    // The tide is too heavy to shove: the drop spreads over the bath above
    // it and flattens against it.
    let top = tide * dome(q.x, ar);
    let free = 1.0 - (1.0 - sstep(0.0, 0.15, q.y + 1.0 - top)) * sstep(0.0, 0.03, top);
    q = mix(q, c + d * sqrt(max(r2 - dA, 0.0) / max(r2, 1e-9)), free);
  }

  // ---- HATS: a flick of fine droplets ----------------------------------
  // These land whole, in the one frame that follows the hit: a tick, and a
  // droplet that took several frames to bloom would be smeared into a dotted
  // line whenever a kick's drop was shoving the bath past it. So reactivity
  // and the hit's strength make them finer, not slower.
  for (var k = 0; k < 5; k++) {
    var n = 0.0;
    var land = false;
    var size = 1.0;
    if (k < 4) {
      // A hit's age is 0 on the frame it is heard and dt on the next.
      let a = uHatAges[k];
      n = uHatN - f32(k);
      land = a > 0.0 && a <= dt * 1.5;
      size = mix(0.55, 1.0, uHatHits[k]);
    } else if (uIdle > 0.5) {
      n = floor(uTime / IDLE_H);
      land = uTime - n * IDLE_H < dt;
      n += 250.0;
    }
    if (!land) {
      continue;
    }
    // Along the top edge of the bath, in a streak that runs with the edge.
    let h = hash22(vec2f(n, 91.0));
    let c0 = vec2f((h.x - 0.5) * ar * 1.7, 0.58 + 0.34 * h.y);
    let th = (hash21(vec2f(n, 5.0)) - 0.5) * 0.9;
    let dir = vec2f(cos(th), sin(th));
    var tone = voiceCol(HATS) * 2.4;
    if (k == 4) {
      tone = pal(hue + 0.5 + 0.1 * h.x) * 1.7;
    }
    for (var i = 0; i < 6; i++) {
      let hi = hash22(vec2f(n, f32(i) + 0.5));
      let c = c0 + dir * (f32(i) - 2.5 + hi.x) * 0.12 + vec2f(-dir.y, dir.x) * (hi.y - 0.5) * 0.1;
      let rr = (0.012 + 0.036 * hi.y * hi.x) * mix(HAT0, 1.0, uReact) * size;
      let R2 = rr * rr;
      let d = q - c;
      let r2 = dot(d, d);
      let rho = sqrt(r2);
      let m = clamp((rr - rho) / px + 0.5, 0.0, 1.0);
      if (m > 0.0) {
        // A ring of ink round a clear eye.
        let x = 1.0 - rho / rr;
        let wx = px / rr;
        let ring = 1.0 - sstep(0.42 - wx, 0.42 + wx, x);
        fresh += keep * m * tone * hot * ring;
        keep *= 1.0 - m;
      }
      q = c + d * sqrt(max(r2 - R2, 0.0) / max(r2, 1e-9));
    }
  }

  // ---- SNARE: the stylus ------------------------------------------------
  let snareLife = SNARE_IN * (1.0 + 2.5 * (1.0 - uReact)) + 6.0 * SNARE_OUT;
  for (var k = 0; k < 5; k++) {
    var a = 10.0;
    var n = 0.0;
    var size = 0.0;
    var land = false;
    if (k < 4) {
      a = uSnareAges[k];
      n = uSnareN - f32(k);
      size = uSnareHits[k] * reach;
      land = a > 0.0 && a <= dt * 1.5;
    } else {
      n = floor(uTime / IDLE_Q);
      a = uTime - n * IDLE_Q;
      n += 500.0;
      size = uIdle * reach;
      land = a < dt;
    }
    if (a - dt >= snareLife || size < 0.01) {
      continue;
    }
    // One long straight stroke through the middle of the bath, steep, so it
    // cuts across the tide and the thread.
    let c = (hash22(vec2f(n, 41.0)) - 0.5) * vec2f(ar, 0.7);
    let th = PI * (0.2 + 0.6 * hash21(vec2f(n, 77.0)));
    let dir = vec2f(cos(th), sin(th));
    let rel = q - c;
    let along = dot(rel, dir);
    let across = abs(dot(rel, vec2f(-dir.y, dir.x)));
    // The stylus leaves a hair of ink where it went in, on the frame it
    // lands: hot enough to crack, and cooled a moment later.
    if (land) {
      let wid = (0.003 + 0.006 * size) * (1.0 - along * along / 0.81);
      let m = clamp((wid - across) / px + 0.5, 0.0, 1.0) * clamp(2.0 * wid / px, 0.0, 1.0);
      var tone = voiceCol(SNARE) * 3.0;
      if (k == 4) {
        tone = pal(hue + 0.3) * 1.8;
      }
      fresh += keep * m * tone * hot;
      keep *= 1.0 - m;
    }
    // And tugs the ink either side along it, at the pace of snareEnv.
    let dz = DRAG * size * (pushed(a, SNARE_IN, SNARE_OUT) - pushed(a - dt, SNARE_IN, SNARE_OUT));
    let a2 = along * along;
    q -= dir * dz * exp(-across / 0.075) / (1.0 + a2 * a2);
  }

  // ---- the current -----------------------------------------------------
  // It never stops; the bass line quickens it in the lower part of the bath.
  q -= flow(q, 0.7 * bath, 0.8 * bassLine * bath, 0.9 * bath) * dt;

  // A riser's whirlpool: when a full mix turns to hiss, wind the bath up and
  // draw it in. (Hats or a snare alone are bright too, but not full.)
  // It turns the bath above the tide, and leaves the tide where it is.
  let riser = sstep(0.5, 0.68, uCentroid) * sstep(0.25, 0.45, uEnergy) * (1.0 - uIdle)
    * sstep(0.0, 0.4, q.y + 1.0 - tide * dome(q.x, ar));
  q = rot(1.6 * riser * bath * dt / (dot(q, q) + 0.2)) * q * (1.0 + 0.09 * riser * bath * dt);

  // ---- BASS: the tide ----------------------------------------------------
  let lift = dome(q.x, ar);
  let well = WELL * bath;
  // Under the tide line the bass's ink wells up from the rim all the while,
  // slowing to nothing at the line, so whatever the current or the stylus
  // carries into the tide is borne up to the line, and the tide closes
  // behind it.
  if (tide * lift > 1e-4 && q.y < tide * lift - 1.0) {
    let z = (q.y + 1.0) / (tide * lift);
    q.y = select(q.y - well * dt, -1.0 + tide * lift * (1.0 - (1.0 - z) * exp(min(well * dt / (tide * lift), 4.0))), z > 0.0);
  }
  // Then from where the tide stands now, down to the resting bath, and back
  // up to where it stood a frame ago.
  q.y = tideUp(tideDown(q.y, tide * lift), tideWas * lift);

  // ---- TUNE: the pipette -------------------------------------------------
  {
    // A thread laid along the path the tip travelled during this frame. It
    // is measured on last frame's bath (q), from where the thread ended then
    // to where the tip is now (carried back by this pixel's own displacement,
    // p - q), so pushes and pulls never break the thread into dashes.
    let c0 = vec2f(penX(uTime - dt, ar), penWas);
    let seg = pen - (p - q) - c0;
    let rel = q - c0;
    let hgt = clamp(dot(rel, seg) / max(dot(seg, seg), 1e-9), 0.0, 1.0);
    let dist = length(rel - seg * hgt);
    // Its weight is how much tune there is; every note swells and heats it.
    let pluck = noteSum();
    let wid = (0.003 + 0.0065 * uTune + 0.006 * pluck) * down + 0.5 * px;
    let m = clamp((wid - dist) / px + 0.5, 0.0, 1.0) * sstep(0.0, 0.3, down);
    if (m > 0.0) {
      let tone = mix(voiceCol(TUNE), pal(hue + 0.3), uIdle);
      fresh += keep * m * tone * (0.9 + 1.6 * pluck);
      keep *= 1.0 - m;
    }
  }

  // ---- last frame, moved ------------------------------------------------
  let uq = unaspect(q);
  let e = min(uq, 1.0 - uq) * uRes;
  var old = prevSharp(uq);
  // Under the bottom rim is the bass's ink: a body of it, in soft bands
  // that come up over the rim one after another. With no bass line there is
  // nothing under there.
  if (e.y < 1.0 && uq.y > 0.5) {
    let t = fract(well * (uTime - dt) / BAND) + (tideWas * dome(q.x, ar) - 1.0 - q.y) / BAND;
    let wt = px / BAND;
    // Each band is brightest at its leading edge and falls away behind it.
    let f = fract(t);
    let shade = mix(0.5, 1.15 * hot, sstep(1.0 - 2.0 * wt, 1.0, f) + 1.0 - sstep(0.0, 0.7, f));
    let pool = voiceCol(BASS) * (0.6 + 0.5 * bassLine) * shade * sstep(0.0, 0.05, bassLine);
    old = mix(pool, old, clamp(e.y, 0.0, 1.0));
  }
  // Nothing comes in over the other three rims, except that the tide runs
  // on past the sides of the bath. (As a rate, so that the bath's edge is
  // as dark at any frame rate.)
  let wasTop = tideWas * dome(q.x, ar);
  let under = (1.0 - sstep(-0.02, 0.02, q.y + 1.0 - wasTop)) * sstep(0.0, 0.03, wasTop);
  old *= pow(clamp(min(mix(e.x, 1.0, under), uq.y * uRes.y), 0.0, 1.0), FR);

  // Fresh ink is hot. What cracked (a stroke of the stylus, a droplet) is
  // gone in a few frames; the rest cools quickly to its body colour.
  let lum = max(old.r, max(old.g, old.b));
  old *= 1.0 - (1.0 - pow(0.78, FR)) * sstep(1.3, 2.6, lum);
  old *= 1.0 - (1.0 - pow(0.93, FR)) * sstep(0.5, 1.6, lum);
  // Then it fades slowly, faster when the mix is full so the bath never
  // silts up, to a faint stain, and the stain lingers. All of it at the
  // bath's pace: the less ink arrives, the longer it is left.
  // (But not under the tide: the bass keeps its ink as long as it holds.)
  let fade = mix(0.9988, 0.9930, sstep(0.3, 0.9, uEnergy));
  let top = tide * dome(p.x, ar);
  let dry = 1.0 - (1.0 - sstep(-0.05, 0.1, p.y + 1.0 - top)) * sstep(0.0, 0.03, top);
  old *= pow(mix(0.9992, fade, sstep(0.008, 0.11, lum)), FR * bath * dry);

  // The memory pixel.
  if (uv.x * uRes.x < 1.0 && uv.y * uRes.y < 1.0) {
    let hi = floor(penN / 1024.0);
    return MEM * (1.0 + vec3f(tideN, hi, penN - hi * 1024.0) / 1024.0);
  }
  return fresh + keep * old;
}
