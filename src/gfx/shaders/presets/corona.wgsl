// Corona: a star whose anatomy is the music.
//
// A molten disc, limb darkened, with a corona of fine hair streaming off it
// along a slowly winding spiral. The previous frame is blown outward along
// the same spiral, so everything the star throws off streaks away as wind.
//
// The five voices, from the centre of the star outward:
//
//   voice  what it does                          where                 colour
//   -----  ------------------------------------  --------------------  ---------------
//   KICK   A mass ejection. The whole disc       The disc, and from    voiceCol(KICK)
//          swells and its granules flush; a      the limb out across
//          shock front leaves the limb, bulging  the corona to twice
//          to a different side each time,        the star's width:
//          lights the hair it crosses and        the biggest thing
//          sweeps it thin behind; the wind       there is.
//          gusts. All of it kickEnv: a heavy
//          push, then a slow release.
//   TUNE   Prominences: a nest of loops over     On the limb, the      voiceCol(TUNE)
//          each note that sounds (twelve sites   middle of the
//          in circle-of-fifths order, so a       picture. The loops
//          chord is a cluster and a chord        arch higher as the
//          change moves it round), and a glow    tune rises.
//          on the limb under them. uPitch
//          glides their height; noteEnv plucks
//          them brighter as a note starts.
//   SNARE  A flare: one straight jet out of the  From a new foot on    voiceCol(SNARE)
//          limb, askew, so it cuts across the    the limb each time,
//          hair, with a flash at its foot.       out across the
//          snareEnv: there at once, gone in an   corona: the length
//          instant.                              of a limb.
//   HATS   Sparks: pinpoints, a different        Scattered round the   voiceCol(HATS)
//          handful each time. hatEnv: a tick.    outer corona, clear
//                                                of the star.
//   BASS   The outermost haze: the far corona    Beyond everything     voiceCol(BASS)
//          fills with glowing strands, which     else, in from the
//          reach further in and brighten as      edges of the
//          uBassLine swells, and ebb as it       picture.
//          falls.
//
// A voice that is not playing leaves none of this. What a voice does draw,
// the wind carries off: a hit dims at once to an afterglow in its colour, and
// the afterglow streaks away and is gone within half a second.
//
// What is there whoever is playing is scenery, slow and in pal() colours:
//
//   spectrum   The corona's hair. Bass streams from the sides, treble from
//              top and bottom, smooth all the way round (no seam): each
//              angle's streamer is as long and bright as its frequency has
//              been loud over the last half second or so, so it swells and
//              thins with the mix and never jumps with a hit.
//   harmony    Slides the whole palette, so the star changes colour with the
//              chord.
//   centroid   Brighter sound, finer granulation; and a fringe of needles on
//              the limb that a riser raises into a crown.
//   energy     How hot the disc runs, how far the corona reaches and how
//              hard the wind blows.
//   loudness   How fast the surface churns and the hair flows (uPhase).
//   silence    The star keeps a slow pulse of its own, and the loops and the
//              haze breathe on with the idle tune and bass line, all in
//              palette colours.
//   uReact     The voices' envelopes already carry it. Turned down, the wind,
//              the churn, the flow of the hair and the shock fronts also run
//              slower, and the corona follows the spectrum more slowly still.
//
// The disc also mirrors its sky at grazing angles (a circle inversion of the
// previous frame), which closes the prominences into rings through the limb.

// Value noise that is periodic in x with whole-number period `per`, so
// texture laid round the star closes on itself without a seam.
fn ringNoise(x: f32, y: f32, per: f32) -> f32 {
  let i = floor(x);
  let j = floor(y);
  let fx = x - i;
  let fy = y - j;
  let ux = fx * fx * (3.0 - 2.0 * fx);
  let uy = fy * fy * (3.0 - 2.0 * fy);
  let i0 = fmod(i, per);
  let i1 = fmod(i + 1.0, per);
  return mix(
    mix(hash21(vec2f(i0, j)), hash21(vec2f(i1, j)), ux),
    mix(hash21(vec2f(i0, j + 1.0)), hash21(vec2f(i1, j + 1.0)), ux),
    uy,
  );
}

// 1 along the middle level line of ringNoise, falling to 0 either side. The
// lines run mostly along y, merging and splitting like field lines.
fn ridge(x: f32, y: f32, per: f32) -> f32 {
  return 1.0 - abs(2.0 * ringNoise(x, y, per) - 1.0);
}

// Hair: those lines, drawn fine.
fn hair(x: f32, y: f32, per: f32) -> f32 {
  let n = ridge(x, y, per);
  return n * n * n;
}

// Only the last four hits of each voice are remembered. The oldest bows out
// as soon as a new one arrives (`newest` is that one's age), so that nothing
// is still on screen when it drops off the end of the list, however fast
// the hits come.
fn last4(n: i32, newest: f32) -> f32 {
  return select(1.0, sstep(0.09, 0.0, newest), n == 3);
}

// uReact: how fast whatever moves all the time moves, and how far a front
// runs in a frame. The wind's trails outlast it to match, so they keep their
// length. (How hard a hit lands is already in the voices' envelopes.)
fn paceAmt() -> f32 {
  return mix(0.5, 1.0, uReact);
}

// How much tune there is to show. Below a whisper it is the pitch of a drum
// passing through, and the limb stays bare.
fn tuneAmt() -> f32 {
  return sstep(0.04, 0.3, uTune);
}

// Churning cellular noise. x: distance to the nearest cell point; y: gap to
// the second nearest (0 on a cell wall).
fn cells(p: vec2f, t: f32) -> vec2f {
  let ip = floor(p);
  let fp = p - ip;
  var d1 = 8.0;
  var d2 = 8.0;
  for (var j = -1; j <= 1; j++) {
    for (var i = -1; i <= 1; i++) {
      let g = vec2f(f32(i), f32(j));
      let hh = hash22(ip + g);
      let o = 0.5 + 0.4 * sin(t * (0.6 + hh.yx) + TAU * hh);
      let d = length(g + o - fp);
      if (d < d1) {
        d2 = d1;
        d1 = d;
      } else if (d < d2) {
        d2 = d;
      }
    }
  }
  return vec2f(d1, d2 - d1);
}

// The disc: convection on a sphere, limb darkened. `note` is how strongly
// the tune is sounding the pitch class whose site is at this angle.
fn photosphere(p: vec2f, R: f32, hue: f32, spin: f32, note: f32, px: f32) -> vec3f {
  let s = p / R;
  let rr = min(dot(s, s), 0.9999);
  let mu = sqrt(1.0 - rr);
  // Stereographic chart of the near hemisphere: cells foreshorten at the limb.
  let st = rot(spin * TAU) * (s / (1.0 + mu));
  let churn = (uTime * 0.2 + uPhase * 0.25) * paceAmt();
  // Slow rivers of plasma: warped fbm.
  let w = vec2f(fbm(st * 1.7 + vec2f(0.0, churn * 0.05)), fbm(st * 1.7 + vec2f(5.2, -churn * 0.04))) - 0.5;
  let big = fbm((st + w * 0.7) * 2.4 + vec2f(churn * 0.03, 3.0));
  let hot = sstep(0.2, 0.62, big);
  let g = st + w * 0.16;
  let c1 = cells(g * 11.0, churn * 0.6);
  let c2 = cells(g * 34.0 + 9.7, churn * 1.5);
  let b1 = sstep(0.62, 0.05, c1.x);
  let b2 = sstep(0.60, 0.08, c2.x);
  // Brighter sound, finer grain; and never finer than the pixels can hold.
  let fineVis = sstep(2.0, 5.0, 2.0 * R / 34.0 / px) * sstep(0.0, 0.35, mu);
  let fineAmt = (0.3 + 0.7 * sstep(0.24, 0.55, uCentroid)) * fineVis;
  let net1 = sstep(0.0, 0.13, c1.y);
  let net2 = sstep(0.0, 0.16, c2.y);
  let gm = (0.6 + 0.4 * b1) * (0.8 + 0.2 * net1);
  let gf = mix(1.0, (0.3 + 0.8 * b2) * (0.45 + 0.55 * net2), fineAmt);
  let limb = mix(0.05, 1.0, pow(mu, 0.7));
  let lum = (0.55 + 0.3 * uEnergy) * gm * gf * (0.38 + 1.05 * hot * hot) * limb;
  var c = mix(pal(hue + 0.38), pal(hue + 0.04 + 0.1 * (1.0 - mu)), hot) * lum;
  // Granule cores run hotter in a fuller mix.
  let core = pow(b1 * mix(1.0, b2, fineAmt), 2.5) * hot * 1.6;
  c += pal(hue + 0.25) * core * limb * (0.2 + 0.25 * uEnergy);
  // KICK: as the disc swells its granules flush with the kick's colour.
  c += voiceCol(KICK) * kickSum() * limb * (1.1 * core + 0.3 * gm * gf);
  // TUNE: faculae. Under a sounding note the lanes near the limb glow.
  let tuneC = mix(voiceCol(TUNE), pal(hue + 0.5), uIdle);
  c += tuneC * note * (0.15 + (1.0 - net1)) * sstep(0.5, 0.98, rr) * 0.5 * (0.3 + 0.7 * mu);
  // The thin bright skin, seen edge on.
  c += pal(hue + 0.5) * pow(1.0 - mu, 5.0) * 0.35;
  return c;
}

fn scene(uv: vec2f) -> vec3f {
  let p = aspect(uv);
  let px = 2.0 / uRes.y;
  let r = length(p);
  let dir = p / max(r, 1e-4);
  let ang = atan2(p.y, p.x);
  let dt = FR / 60.0;
  let pace = paceAmt();
  let kick = kickSum();
  let tune = tuneAmt();

  // KICK: the whole disc swells.
  let R = 0.334 + 0.018 * kick;
  let h = r - R;
  let hc = max(h, 0.0);
  let hue = 2.0 * uHarmony + uTime * 0.006;

  // ---- the twelve notes sit round the limb in circle-of-fifths order -----
  let spin = uTime * 0.011;
  let x12 = fract(ang / TAU - spin) * 12.0;
  let k0 = floor(x12);
  let kA = i32(k0) % 12;
  let kB = (kA + 1) % 12;
  let cA = chroma((kA * 7) % 12) * tune;
  let cB = chroma((kB * 7) % 12) * tune;
  let xf = x12 - k0;
  let note = mix(cA, cB, xf * xf * (3.0 - 2.0 * xf));

  // ---- solar wind: the previous frame, blown outward with a twist --------
  let twist = 0.85 * sin(uTime * 0.05);
  let gust = (0.24 + 0.1 * uEnergy) * pace + 0.16 * kick;
  let vel = gust * (1.0 + 1.4 * hc);
  // The wind leaves along a spiral, as it does from any turning star. The
  // spiral winds up, relaxes and winds the other way over a couple of
  // minutes. `spiral` is how far round a streamline has turned by height h;
  // the hair is drawn along the same lines, so the wind combs it.
  let spiral = 2.6 * twist * log((1.0 + 2.5 * hc) / (1.0 + 1.4 * hc));
  let turn = 2.6 * twist * (2.5 / (1.0 + 2.5 * hc) - 1.4 / (1.0 + 1.4 * hc));
  var q = rot(-turn * vel * dt) * (p - dir * vel * dt);
  let tt = uTime * 0.17 * pace;
  let tb = vec2f(vnoise(p * 3.3 + vec2f(0.0, tt)), vnoise(p * 3.3 + vec2f(31.7, -tt))) - 0.5;
  q += tb * 0.08 * dt * sstep(0.0, 0.3, hc);

  // ---- KICK: every kick is a mass ejection, bulging to one side -----------
  var shock = 0.0;
  var wake = 0.0;
  for (var n = 0; n < 4; n++) {
    let age = uKickAges[n];
    let sd = hash21(vec2f(uKickN - f32(n), 1.7));
    let grow = 1.9 * pace * (1.0 - exp(-age * 0.8));
    let bulge = 1.0 + 0.3 * cos(ang - sd * TAU);
    let d = h - grow * bulge;
    // A sharp leading edge and a softer trailing one, long enough to cover
    // the ground the front gains on the wind in a frame, so the afterglow it
    // leaves in the feedback is smooth at any frame rate.
    let wf = max(1.5 * px, 0.004 + 0.008 * age);
    let wb = wf + 0.003 + 0.95 * pace * exp(-age * 0.8) * bulge * dt;
    // The front is as bright as the kick's push is strong: it swells as it
    // clears the limb and is spent a few star-widths out.
    let fade = kickEnv(n) * last4(n, uKickAges.x);
    shock += select(exp(-(d * d) / (wf * wf)), exp(d / wb), d < 0.0) * fade;
    // Behind the front the corona is swept thin for a moment.
    wake += sstep(0.0, -2.0 * wb, d) * exp(d / 0.13) * fade;
  }
  // With nothing playing the star keeps a slow pulse of its own (scenery).
  var pulse = 0.0;
  {
    let age = fmod(uTime, 6.0);
    let d = h - 0.9 * (1.0 - exp(-age * 0.5));
    let w = max(1.5 * px, 0.006 + 0.012 * age);
    let fade = uIdle * 0.8 * exp(-age * 0.9) * sstep(0.0, 0.5, age);
    pulse = exp(-(d * d) / (w * w)) * fade;
    wake += sstep(0.0, -2.0 * w, d) * exp(d / 0.13) * fade;
  }
  // The shock shoves the wind ahead of it.
  q -= dir * (shock + pulse) * 0.008 * FR;

  // ---- corona: the spectrum as streamers of fine hair (scenery) ----------
  // Bass at the sides, treble above and below, smooth all the way round.
  let axis = 0.2 * sin(uTime * 0.043);
  let a2 = ang - axis;
  // The lobes are pulled unevenly round the disc, and the pull drifts, so the
  // corona never settles into a cross.
  let a2s = a2 + spiral;
  let a3 = a2s + 0.36 * sin(a2s - uTime * 0.021) + 0.17 * sin(2.0 * a2s + 1.3 + uTime * 0.013);
  let f = 0.5 - 0.5 * cos(2.0 * a3);
  // The corona follows the spectrum of the last half second or so (longer,
  // turned down) and not of this instant: a streamer swells out under a
  // sound that stays, and no single hit makes it jump.
  let sx = mix(0.02, 0.86, f);
  let gap = mix(10.0, 5.0, uReact);
  var s = 0.0;
  for (var n = 0; n < 6; n++) {
    s += histRow(sx, uHistHead - 1.0 - gap * f32(n));
  }
  s /= 6.0;
  let au = a2s / TAU;
  // Finest hair the pixels can hold at the limb; finer still fades in further
  // out, where the lines have spread apart. They are counted round the star
  // at rest: counted round the swelling limb, the number would change as the
  // star swells, and each change deals the whole head of hair afresh.
  let Rk = 0.337;
  let kf = clamp(floor(TAU * Rk / (6.0 * px)), 40.0, 320.0);
  let k2 = floor(kf * 2.3);
  let k0f = floor(kf * 0.31);
  let flow = (uTime * 0.1 + uPhase * 0.1) * pace;
  // (Measured from the star at rest, so the hair does not slide as it swells.)
  let lr = log(r / Rk + 1e-3);
  let broad = ringNoise(au * k0f, lr * 1.5 - flow, k0f);
  let h1 = hair(au * kf, lr * 2.2 - flow * 0.7, kf);
  let h2 = hair(au * k2 + 0.5, lr * 3.1 - flow, k2) * sstep(3.0, 6.0, TAU * r / k2 / px);
  let hairs = max(h1, h2);
  let len = 0.05 + (0.35 + 0.25 * uEnergy) * s * s;
  let env = (0.04 + 0.9 * s * sqrt(s)) * exp(-hc / len) + (0.025 + 0.03 * uEnergy + 0.05 * uIdle) / (1.0 + 5.0 * hc * hc);
  let thin = 1.0 - 0.75 * min(wake, 1.0);
  // Close in the hair sits in a glow; further out only the hair is left.
  let glow = exp(-2.5 * hc);
  let lit = env * ((0.1 + 0.25 * broad) * glow + hairs * (0.36 + 1.4 * broad));
  var fresh = pal(hue + 0.33 + 0.22 * f + 0.3 * hc) * lit * (0.85 + 2.2 * pulse) * thin;
  fresh += pal(hue + 0.12) * pulse * (0.1 + 0.3 * hairs);
  // KICK: the front itself, and the hair it lights as it passes.
  fresh += voiceCol(KICK) * (shock * (0.55 + 0.9 * hairs + 3.0 * lit) + min(wake, 1.0) * 0.12 * (0.3 + hairs));

  // chromosphere: a thin bright skin, brighter where the tune sounds a note
  let skin = exp(-hc / max(0.006, 1.5 * px));
  let tuneC = mix(voiceCol(TUNE), pal(hue + 0.5), uIdle);
  fresh += (pal(hue + 0.5) * 0.3 + tuneC * 0.8 * note) * skin;

  // ---- a fringe of fine needles stands on the limb all the time, and grows
  // as the sound brightens: a riser raises a crown (scenery) ---------------
  if (hc < 0.19) {
    let crown = sstep(0.28, 0.7, uCentroid);
    let tall0 = ringNoise(au * k0f + 91.0, flow * 3.0, k0f);
    // Each needle is tallest along its own line and tapers to a point.
    let n0 = ridge(au * kf + 5.0, flow * 2.0, kf);
    let lc = (0.016 + 0.15 * crown) * (0.25 + tall0 * tall0) * n0 * n0 * n0;
    fresh += pal(hue + 0.62) * sstep(lc, lc * 0.2, hc) * n0 * n0 * (0.2 + 0.4 * crown);
  }

  // ---- HATS: sparks scatter round the outer corona, a different handful
  // each time ---------------------------------------------------------------
  let rim = sstep(0.5, 0.66, r);
  if (rim > 0.0) {
    var spark = 0.0;
    // A spark's speed: the wind's, without the gusts.
    let sv = 0.42 * pace * (1.0 + 1.4 * hc);
    for (var n = 0; n < 4; n++) {
      let age = uHatAges[n];
      let id = uHatN - f32(n);
      // Each hat scatters its own lattice of sparks, and the whole lattice
      // rides out on the wind, so no spark ever crosses the edge of its cell.
      let sc = vec2f(ang / TAU * 48.0, (lr - sv / max(r, 0.05) * age) * 5.0 + fract(id * 0.37));
      let si = floor(sc);
      let sf = sc - si;
      let cell = vec2f(fmod(si.x, 48.0), si.y) + floor(hash22(vec2f(fmod(id, 512.0), 2.3)) * 61.0) * vec2f(48.0, 16.0);
      let hh = hash22(cell);
      let pos = vec2f(0.3, 0.45) + vec2f(0.4, 0.35) * hash22(cell + 7.7);
      let dd = (sf - pos) * vec2f(TAU / 48.0, 0.2) * r;
      let sz = max(1.2 * px, 0.002 + 0.0028 * r);
      // The head is stretched by a frame's travel so it draws a line and not a
      // row of beads; behind it, a short tail.
      let head = max(sz, 0.7 * sv * dt);
      let tail = max(head, 0.5 * sv * min(age, 0.1));
      let along = select(dd.y / head, 1.4 * dd.y / tail, dd.y < 0.0);
      spark += step(0.9, hh.x) * exp(-dd.x * dd.x / (sz * sz) - along * along) * hatEnv(n);
    }
    fresh += voiceCol(HATS) * spark * rim * 4.0;
  }

  // ---- TUNE: prominences, loops over the notes that are sounding ---------
  if (h > 0.0 && hc < 0.36) {
    var prom = vec3f(0.0);
    let lth = max(1.3 * px, 0.003);
    // How high the loops arch is how high the tune sits.
    let arch = 0.04 + 0.28 * uPitch;
    // A note starting plucks them brighter.
    let pluck = 2.6 + 1.6 * noteSum();
    for (var n = 0; n < 2; n++) {
      let kk = select(kA, kB, n == 1);
      let c = select(cA, cB, n == 1);
      let lean = 0.35 * sin(uTime * 0.31 * pace + f32(kk) * 1.7);
      let a = (xf - f32(n)) * (TAU / 12.0) * R - lean * hc;
      let win = sstep(0.17, 0.11, abs(a));
      let tint = mix(voiceCol(TUNE), pal(f32(kk) / 12.0 + hue), uIdle);
      // No two sites are quite the same width.
      let wide = 0.048 + 0.026 * fract(f32(kk) * 0.618 + 0.2);
      var acc = 0.0;
      for (var j = 0; j < 3; j++) {
        let lv = 1.0 - 0.27 * f32(j);
        let w = wide * lv;
        // A loop stands up off the limb as its note comes in.
        let H = arch * (0.4 + 0.6 * c) * lv;
        let e = length(vec2f(a / w, hc / H));
        let gr = length(vec2f(a / (w * w), hc / (H * H))) / max(e, 1e-3);
        let d = abs(e - 1.0) / max(gr, 1e-3);
        let tp = atan2(hc / H, a / w);
        let blob = 0.35 + 0.65 * pow(0.5 + 0.5 * sin(tp * 5.0 - uTime * 2.2 * pace + f32(kk) * 2.4 + f32(j) * 1.9), 2.0);
        acc += exp(-(d * d) / (lth * lth)) * blob * (1.0 - 0.25 * f32(j));
        // a faint curtain hanging inside the outer loop
        acc += 0.12 * e * e * sstep(1.0, 0.9, e) * select(0.0, 1.0, j == 0);
      }
      prom += tint * acc * c * pluck * win;
    }
    fresh += prom;
  }

  // ---- SNARE: a flare, one straight jet out of the limb -------------------
  for (var n = 0; n < 4; n++) {
    let crack = snareEnv(n) * last4(n, uSnareAges.x);
    let id = uSnareN - f32(n);
    let fa = hash21(vec2f(id, 7.3)) * TAU;
    let fd = vec2f(cos(fa), sin(fa));
    // It leaves askew, to one side or the other, so it cuts across the hair
    // and is not taken for a strand of it.
    let skew = (0.55 + 0.45 * hash21(vec2f(id, 3.9))) * select(-1.0, 1.0, hash21(vec2f(id, 5.1)) > 0.5);
    let jd = rot(skew) * fd;
    let rel = p - fd * R;
    let along = dot(rel, jd);
    let across = rel.x * jd.y - rel.y * jd.x;
    let x = along / (0.6 + 0.3 * uSnareHits[n]);
    // A hard thin core in a soft sheath, tapering to its tip.
    let w = max(1.3 * px, 0.004) * (1.0 - 0.5 * clamp(x, 0.0, 1.0));
    let beam = exp(-(across * across) / (w * w)) + 0.22 * exp(-(across * across) / (25.0 * w * w));
    let jet = beam * sstep(-0.012, 0.012, along) * sstep(1.0, 0.2, x);
    let flash = exp(-dot(rel, rel) / 0.0012);
    fresh += voiceCol(SNARE) * (jet * 2.4 + flash * 1.4) * crack;
  }

  // ---- BASS: the outermost haze swells in from the edge -------------------
  // Strands of the far corona, lit: further in and brighter, the more bass
  // line there is. (The merest trace of one is the tail of a kick.)
  let bl = uBassLine * sstep(0.0, 0.3, uBassLine);
  let inner = 1.0 - 0.42 * bl;
  let far = sstep(inner - 0.24, inner + 0.3, r);
  let lobes = ringNoise(au * 7.0, lr * 1.3 - flow * 0.6, 7.0);
  let bassC = mix(voiceCol(BASS), pal(hue + 0.9) * 0.4, uIdle);
  let haze = bassC * bl * far * (0.03 + hairs * (0.3 + 0.9 * broad)) * (0.3 + 0.7 * lobes) * 0.27;

  // Three taps along the path, so thin things leave a continuous streak
  // whatever the frame rate. Between the hairs the wind thins out faster,
  // which keeps it stranded.
  let keep = 0.80 + 0.11 * clamp(hairs + 0.5 * broad, 0.0, 1.0);
  var wind = vec3f(0.0);
  for (var n = 1; n <= 3; n++) {
    wind = max(wind, prev(unaspect(mix(p, q, f32(n) / 3.0))));
  }
  // The wind does not fade to black out there but to the haze, and where the
  // haze is to be brighter than what is there, it gets there over half a
  // second: that is how it swells, and why it never jumps.
  let fill = mix(min(wind, haze), haze, 1.0 - exp(-0.035 * FR));
  // What a hit leaves behind dims at once to an afterglow, and the afterglow
  // is what the wind carries off: a crack is gone in an instant, a tick is a
  // tick, and only their ghosts streak away.
  let glare = sstep(0.2, 1.0, max(wind.r, max(wind.g, wind.b)));
  let stay = pow(keep * (0.6 + 0.4 * thin), FR * pace) * pow(1.0 - 0.4 * glare, FR);
  var col = max(max(wind * stay, fill), fresh);

  // ---- the disc on top ---------------------------------------------------
  if (h < 2.0 * px) {
    var disc = photosphere(p, R, hue, spin, note, px);
    // The molten surface mirrors its own sky, turned inside out.
    let inv = p * (R * R) / max(r * r, 1e-4);
    let graze = r / R;
    disc += prev(unaspect(inv)) * 0.55 * graze * graze * graze * graze * sstep(0.45, 0.8, graze);
    col = mix(col, disc, sstep(px, -px, h));
  }
  return col;
}
