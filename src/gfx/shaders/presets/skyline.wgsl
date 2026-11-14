// Skyline: the classic spectrum bars as a city at night, standing on water
// under an aurora, with its own past receding behind it.
//
// The scenery is the spectrum itself, in pal() colours, and it answers no
// voice in particular:
//
// - The front row is the analyser: one tower per whole tone, low notes on the
//   left, each as tall as its band is loud, with a beacon on a spire at the
//   peak-hold height.
// - History. Every 8/60 s the skyline of that moment is left standing behind
//   the live row and glides away towards the horizon as a darker, hazier row,
//   so about two and a half seconds of the spectrogram are in view at once:
//   a riser is a ridge that slants to the right as it recedes. The haze at
//   street level is lit where a row's towers were loud.
// - Harmony. uHarmony turns the whole palette, so the city changes colour
//   with the chord.
// - Brightness and energy. The aurora climbs as the sound gets brighter
//   (uCentroid) and fills out with uEnergy; its rays are brightest over the
//   bands that have been loud for the last second. The water is calm in a
//   breakdown and choppy after a drop; the stars come out when the mix thins.
//
// On that stage each of the five voices has one colour, one motion and one
// place, and leaves no trace when it is not playing:
//
//   voice  what it does                         where                 colour
//   -----  -----------------------------------  --------------------  ---------------
//   KICK   pushes one broad ring out over the   the foreground water, voiceCol(KICK)
//          water, bending the reflections as    from the middle of
//          it passes: it wells up, spreads and  the bottom edge out
//          sinks back (kickEnv)                 to both sides
//   SNARE  a crack: one straight streak of      right across the sky, voiceCol(SNARE)
//          light, there and gone (snareEnv)     through the aurora
//   HATS   a tick: a handful of stars glint,    the stars, and the    voiceCol(HATS)
//          and the beacons of some treble       tops of the treble
//          towers glint and throw a spark       towers
//          (hatEnv)
//   BASS   a bank of harbour light that swells  along the quay at     voiceCol(BASS)
//          with uBassLine: longer, higher and   the foot of the low
//          further out over the water           towers
//   TUNE   lit windows: the columns of the      a belt of storeys     voiceCol(TUNE)
//          notes that are sounding (chroma),    across the middle of
//          faint all the way up and bright in   the city, in every
//          a belt that glides up and down with  row
//          uPitch; a note starting plucks the
//          belt brighter (noteEnv)
//
// - Silence. The idle spectrum keeps a small town lit, the aurora stays, and
//   a slow rain rings the water. There are no hits; the quay and the windows
//   breathe with the idle bass line and tune in the scenery's colours.
// - Reactivity. The voices' envelopes already follow uReact. Beyond that,
//   turned down, the towers swell instead of snapping and swing less far, the
//   history glides away more slowly (so more seconds of it are in view), and
//   the water, the aurora and the rings move more slowly. At 1 nothing is
//   held back.
//
// The camera drifts slowly, so the rows slide against each other. No feedback.

// One tower per whole tone, from C1 up: 51 towers span 8.5 octaves.
const BARS: f32 = 51.0;
// spec() coordinate of the left edge of the first tower (a quarter-tone
// below C1), and the width of a whole tone.
const SPEC_X0: f32 = 0.0091;
const SPEC_DX: f32 = 0.018398;
// Rows of towers, the live one included.
const ROWS: i32 = 20;
// Perspective: a row at depth d (in rows) is drawn at scale 1 / (1 + ROW_GAP d).
const ROW_GAP: f32 = 0.07;
// Sixtieths of a second between two rows at full reactivity; more when it is
// turned down (see the conveyor). Always the history's 256 rows divided by a
// whole number, so the conveyor is seamless when the history wraps round.
const ROW_LAG: f32 = 8.0;
// Depth of the water plane, in the front row's units, per unit of 1/scale.
const DEPTH: f32 = 2.5;
// With uReact at 0: what is left of the speed of whatever travels or churns,
// and of a live tower's swing about its own recent average. Both are 1 with
// uReact at 1.
const PACE0: f32 = 0.6;
const SWING0: f32 = 0.3;

// The colour of the tower at bar position bx (0..BARS), `turn` further round.
fn tower_tint(bx: f32, turn: f32) -> vec3f {
  return pal(bx / BARS * 0.55 + uHarmony * 2.0 + turn);
}

// The sky above the horizon hy: glow, stars and the aurora, and in them what
// the hats and the snare do. `dry` is 1 for the sky itself and 0 for its
// reflection in the water, which shows the scenery only: the water belongs
// to the kick and the bass line.
fn sky(p: vec2f, hy: f32, cw: f32, ar: f32, px: f32, dry: f32) -> vec3f {
  let hue = uHarmony * 2.0;
  let y = max(p.y - hy, 0.0);
  // The glow of the city on the horizon.
  var c = pal(0.62 + hue) * (0.03 + 0.10 * uEnergy) * exp(-9.0 * sqrt(y * y + 0.0012));
  c += pal(0.15 + hue) * 0.008 * exp(-y * 1.5);

  // Stars: out when the mix is thin, twinkling slowly on their own.
  let cs = 0.07;
  let sp = p / cs;
  let ci = floor(sp);
  let h2 = hash22(ci);
  let hb = hash21(ci + 17.0);
  let sv = (fract(sp) - 0.5 - (h2 - 0.5) * 0.6) * cs;
  let sd = length(sv);
  let tw = 0.6 + 0.4 * sin(uTime * (1.0 + 3.0 * h2.x) + h2.y * TAU);
  let star = sstep(1.7 * px, 0.5 * px, sd) * step(0.72, hb) * (hb - 0.72) * 3.5 * tw;
  c += pal(hb * 5.0 + hue) * star * (0.9 - 0.6 * uEnergy) * sstep(0.02, 0.3, y);
  // HATS: each hat makes a different handful of stars glint for an instant,
  // in the hats' colour.
  if (dry > 0.0) {
    var tick = 0.0;
    for (var j = 0; j < 4; j++) {
      let n = uHatN - f32(j);
      let pick = step(0.89, hash21(ci * 1.37 + vec2f(fmod(n, 61.0) * 7.31, fmod(n, 53.0) * 3.17)));
      tick += pick * hatEnv(j);
    }
    let av = abs(sv);
    let spike = max(exp(-av.x * 110.0 - av.y * 700.0), exp(-av.x * 700.0 - av.y * 110.0));
    c += voiceCol(HATS) * tick * dry * sstep(0.03, 0.2, y)
      * (2.2 * sstep(2.2 * px, 0.6 * px, sd) + 1.4 * spike);
  }

  // Aurora: a folded curtain, lit from its lower edge upwards. It hangs
  // higher the brighter the sound and reaches further up the fuller the mix;
  // each ray is as bright as the band of the spectrum underneath it has been
  // over the last second, so it shows where the sound is without answering
  // any one hit.
  let bx = p.x / cw + BARS * 0.5;
  let pace = mix(PACE0, 1.0, uReact);
  let t = (uTime * 0.05 + uPhase * 0.05) * pace;
  let fold = 0.07 * sin(p.x * 1.7 + t * 1.3) + 0.04 * sin(p.x * 3.9 - t * 2.1 + 1.7)
    + 0.02 * sin(p.x * 8.3 + t * 3.0);
  let d = p.y - (0.68 - 0.08 * p.x / ar + fold + (uCentroid - 0.35) * 0.4);
  let up = max(d, 0.0);
  let curtain = exp(-up / (0.16 + 0.22 * uEnergy)) * sstep(-0.05, 0.015, d);
  let ray = 0.45 + 0.55 * vnoise(vec2f((p.x + d * 0.25) * 26.0, t * 4.0));
  let ends = sstep(-4.0, 3.0, bx) * sstep(BARS + 4.0, BARS - 3.0, bx);
  let fx = SPEC_X0 + clamp(bx, 0.0, BARS) * SPEC_DX;
  var e = 0.0;
  for (var j = 0; j < 6; j++) {
    e += histRow(fx, uHistHead - 1.0 - 11.0 * f32(j)) / 6.0;
  }
  let lift = sstep(0.42, 0.62, uCentroid);
  let glow = (0.3 + 1.8 * e * e) * (0.05 + 0.14 * uEnergy + 0.08 * uIdle + 0.12 * lift);
  c += tower_tint(bx, 0.12 + up * 0.5) * curtain * ray * glow * ends;

  // SNARE: a crack of light, one straight streak right across the sky
  // through the aurora, in the snare's colour and gone at once. Each hit
  // lies at a slightly different height and tilt.
  if (dry > 0.0) {
    var crack = 0.0;
    let hem = clamp(0.70 + (uCentroid - 0.35) * 0.4, 0.64, 0.76);
    let wl = max(1.1 * px, 0.004);
    for (var j = 0; j < 4; j++) {
      let env = snareEnv(j);
      let hs = hash22(vec2f(uSnareN - f32(j), 4.7));
      let dy = p.y - (hem + (hs.x - 0.5) * 0.08 + ((hs.y - 0.5) * 0.08 - 0.03) * p.x / ar);
      // Brightest somewhere along its length, thinning towards the ends.
      let dx = (p.x / ar - (hs.y - 0.5) * 0.8) / 1.1;
      crack += env * exp(-dx * dx)
        * (1.8 * exp(-dy * dy / (wl * wl)) + 0.2 * exp(-abs(dy) / 0.022) * ray);
    }
    c += voiceCol(SNARE) * crack * dry;
  }
  return c;
}

fn scene(uv: vec2f) -> vec3f {
  let p = aspect(uv);
  let ar = uRes.x / uRes.y;
  let px = 2.0 / uRes.y;

  // The quay the front row stands on, the horizon, and the vanishing point.
  // The camera drifts, so the last two wander.
  let floor_y = -0.5;
  let hy = 0.36 + 0.03 * sin(uTime * 0.073 + 1.0);
  let cam = hy - floor_y;
  let vx = (0.55 + 0.10 * sin(uTime * 0.11)) * ar;
  let width = 2.0 * ar * 0.92;
  let cw = width / BARS;
  // Height of a storey.
  let fh = cw * 0.28;
  let haze_col = pal(0.62 + uHarmony * 2.0);
  // Reactivity: `pace` scales the speed of what travels or churns, and `ease`
  // is how far the towers are smoothed: not at all at 1, fully from the
  // default 0.5 down. The voices' envelopes follow uReact by themselves.
  let pace = mix(PACE0, 1.0, uReact);
  let ease = sstep(1.0, 0.5, uReact);
  // While idle nothing is playing, and what breathes with the idle bass line
  // and tune does so in the scenery's colours.
  let bass_col = mix(voiceCol(BASS), haze_col, uIdle);

  // ---- The water: ripples, and the ring each kick pushes across it ----
  // Scale of the water plane at this pixel (1 at the quay), and where on the
  // plane the pixel is: gx across, gz away from the camera.
  let sg = max((hy - p.y) / cam, 0.03);
  var swell = 0.0;
  var rip = 0.0;
  var ring = 0.0;
  var crest = 0.0;
  var push = 0.0;
  if (p.y < hy + px) {
    let gx = (p.x - vx) / sg + vx;
    let gz = (1.0 / sg - 1.0) * DEPTH;
    // Depth covered by one pixel: ripples must fade out before they alias.
    let dz = DEPTH * px / (sg * sg * cam);
    let chop = (0.55 + 0.6 * uEnergy) * mix(0.7, 1.0, uReact);
    let wt = uTime * 0.7 * pace;
    let n1 = vnoise(vec2f(gx * 2.5 + wt * 0.3, gz * 11.0 - wt)) - 0.5;
    let n2 = vnoise(vec2f(gx * 7.0 - wt * 0.4, gz * 34.0 + wt * 1.6)) - 0.5;
    swell = chop * n1 * sstep(0.30, 0.10, dz * 11.0);
    rip = swell + chop * 0.5 * n2 * sstep(0.30, 0.10, dz * 34.0);

    // KICK: each kick pushes one broad ring out across the whole foreground
    // from the middle of the water at the bottom of the picture. kickEnv is
    // how high the ring stands: it wells up, spreads and sinks back.
    let ky = -0.80;
    let ksg = (hy - ky) / cam;
    for (var j = 0; j < 4; j++) {
      let age = uKickAges[j];
      let amp = kickEnv(j);
      let seed = hash21(vec2f(uKickN - f32(j), 7.3));
      let kx = (seed - 0.5) * 0.5;
      let centre = vec2f((kx - vx) / ksg + vx, (1.0 / ksg - 1.0) * DEPTH);
      let d = length(vec2f(gx, gz) - centre) - 2.9 * pace * age;
      let wd = 0.09 + 0.12 * age;
      let packet = exp(-d * d / (wd * wd));
      ring += amp * packet * sin(d / wd * 2.2);
      let wc = 0.016 + 0.03 * age;
      push += amp * (exp(-d * d / (wc * wc)) + 0.9 * packet);
    }
    // With nothing playing, a slow rain keeps the water alive: small rings
    // in the scenery's colours.
    for (var j = 4; j < 6; j++) {
      let tt = uTime / 2.6 + f32(j) * 0.5;
      let age = fract(tt) * 0.95;
      let seed = hash22(vec2f(floor(tt), f32(j)));
      let centre = vec2f((seed.x - 0.5) * width, -0.15 - 0.5 * seed.y);
      let amp = 0.6 * uIdle * exp(-age * 1.9) * sstep(0.92, 0.5, age) * sstep(0.0, mix(0.06, 0.14, ease), age);
      let d = length(vec2f(gx, gz) - centre) - 2.7 * pace * age;
      let wd = 0.04 + 0.08 * age;
      let packet = exp(-d * d / (wd * wd));
      ring += amp * packet * sin(d / wd * 2.2);
      let wc = 0.010 + 0.02 * age;
      crest += amp * (exp(-d * d / (wc * wc)) + 0.2 * packet);
    }
    // The rings stay on the open water in front of the city.
    let open = sstep(0.25, 0.08, dz * 10.0) * sstep(0.12, -0.03, gz);
    ring *= open;
    crest *= open * (0.75 + 1.2 * rip);
    push *= open * (0.75 + 1.2 * rip);
  }

  // ---- The city ----
  // Behind the live row runs a conveyor: row k was left behind `k - 1 + slide`
  // rows ago and has moved that far back. Its spectrum is one fixed row of
  // the history, so it travels unchanged.
  // Turned down, it leaves a row behind less often, so the rows glide away
  // more slowly and more seconds are in view. `laps` is how many rows the
  // history holds at that spacing: a whole number, so the wrap stays seamless,
  // and no fewer than ROWS, so the last row is never older than the history.
  let laps = max(round(HIST_ROWS / ROW_LAG * mix(0.4, 1.0, uReact)), f32(ROWS));
  let row_lag = HIST_ROWS / laps;
  let conv = uHistHead / row_lag;
  let born = floor(conv);
  let slide = conv - born;
  let last = f32(ROWS - 1);

  // TUNE: how high the belt of lit windows sits above each row's waterline
  // (see the windows), how much tune there is to light them, and its notes.
  let belt_y = clamp(0.24 + (uPitch - 0.55) * 0.7, 0.05, 0.46);
  let tune_on = sstep(0.06, 0.4, uTune);
  let pluck = noteSum();
  let tune_col = voiceCol(TUNE);

  var col = vec3f(0.0);
  // What still shows through, front to back; and through the towers alone,
  // not counting their reflections.
  var trans = 1.0;
  var open_water = 1.0;
  for (var k = 0; k < ROWS; k++) {
    let kf = f32(k);
    let dep = select(kf - 1.0 + slide, 0.0, k == 0);
    let far = dep / last;
    let s = 1.0 / (1.0 + ROW_GAP * dep);
    // This pixel in the row's own frame: xl across, hh above its waterline.
    var xl = (p.x - vx) / s + vx;
    let hh = (p.y - hy) / s + cam;
    let pxl = px / s;
    // Below the waterline is the mirror image, pushed around by the ripples
    // and the rings, the more the further it is from what it reflects.
    let below = clamp(0.5 - hh / pxl, 0.0, 1.0);
    var y = abs(hh);
    let wob = below * (0.03 + 0.7 * y);
    xl += wob * (swell + 0.3 * rip) * 0.09;
    y = max(y + wob * (rip * 0.25 + ring * 0.3), 0.0);
    // Nothing reaches this high: a tower tops out at 1.21, and only the live
    // row has beacons and sparks above that.
    if (y > select(1.22, 1.45, k == 0)) {
      continue;
    }
    let soft = pxl + below * y * 0.06;
    let refl = mix(1.0, 0.5 * exp(-y * 1.9), below);
    let bx = xl / cw + BARS * 0.5;
    let id = floor(bx);
    // The history row this skyline was taken from, and a name for it that
    // survives the history wrapping round.
    let row = (born - kf + 1.0) * row_lag;
    let name = fmod(born - kf, laps) + 1.0;
    // The farthest row dissolves as a new one arrives at the front.
    let fade = clamp(last - dep, 0.0, 1.0);
    let turn = dep * 0.008;

    // The haze in front of this row, lit from street level by its towers.
    if (k > 0) {
      let near = sstep(BARS * 0.5 + 4.0, BARS * 0.5 - 1.0, abs(bx - BARS * 0.5));
      let e = histRow(SPEC_X0 + clamp(bx, 0.0, BARS) * SPEC_DX, row);
      let dens = (0.006 + 0.45 * e * e) * exp(-y * mix(4.0, 9.0, below)) * near * fade * sstep(0.0, 1.0, dep);
      col += trans * dens * mix(tower_tint(bx, turn), haze_col, 0.4) * 0.5;
      trans *= 1.0 - dens * 0.8;
    }

    if (id < 0.0 || id >= BARS) {
      continue;
    }
    // Towers stand apart in the live row and shoulder to shoulder behind it.
    let hw = cw * mix(0.34, 0.455, sstep(0.0, 2.5, dep));
    let lx = (bx - id - 0.5) * cw;
    let f = (id + 0.5) / BARS;
    let fs = SPEC_X0 + (id + 0.5) * SPEC_DX;
    var v = 0.0;
    // The live row's peak hold.
    var held = 0.0;
    if (k == 0) {
      // Three taps across the band: truer, and finer than one 8-bit step.
      v = (spec(fs - SPEC_DX * 0.3) + spec(fs) + spec(fs + SPEC_DX * 0.3)) / 3.0;
      held = peak(fs);
      if (ease > 0.0) {
        // Turned down, a tower shows its band as it has been rather than as
        // it is this instant, all of it read from the history. `recent` is
        // the same three taps across the band spread over the last tenth of
        // a second or so, so the tower swells to a new height over that long
        // instead of snapping to it.
        let now = uHistHead - 0.5;
        let gap = mix(2.67, 1.33, uReact);
        let recent = (histRow(fs, now) + histRow(fs + SPEC_DX * 0.3, now - gap)
          + histRow(fs - SPEC_DX * 0.3, now - 2.0 * gap)) / 3.0;
        // `usual` is its level over most of a second before that, which the
        // tower leans towards, so it swings less far. The taps are close
        // enough together that a hit passes from one to the next without a
        // step, and each tower looks back from a moment of its own, so that
        // what is left of one does not come round in step along the row.
        let then = now - 8.0 - 5.0 * hash21(vec2f(id, 3.7));
        var usual = 0.0;
        for (var j = 0; j < 8; j++) {
          usual += histRow(fs, then - 5.0 * f32(j)) / 8.0;
        }
        let swing = mix(SWING0, 1.0, uReact);
        let calm = mix(usual, recent, swing);
        // The peak hold swings as much less, and never sinks into the tower.
        held = mix(held, max(mix(usual, held, swing), calm), ease);
        v = mix(v, calm, ease);
      }
    } else {
      v = histRow(fs, row);
    }
    let h = 0.012 + 1.2 * v;
    // A silent band is an empty lot.
    let a = sstep(soft, -soft, abs(lx) - hw) * sstep(soft, -soft, y - h) * sstep(0.0, 0.04, v) * fade;
    if (a > 0.001) {
      let tint = tower_tint(id + 0.5, turn);
      // Windows: two columns, a semitone each, and a storey every fh. They
      // melt into an even sheen before they get too small to draw.
      let u = lx / hw;
      let side = step(0.0, u);
      let wv = y / fh;
      let storey = floor(wv);
      let ax = soft / hw;
      let ay = soft / fh;
      let detail = sstep(0.34, 0.17, ay);
      var pane = sstep(0.36 + ax, 0.36 - ax, abs(abs(u) - 0.5))
        * sstep(0.32 + ay, 0.32 - ay, abs(fract(wv) - 0.5));
      pane = mix(0.46, pane, detail);
      // No window in a storey the tower has not reached.
      let built = sstep(storey + 0.7, storey + 1.1, h / fh);
      // TUNE: a column is lit while its note sounds, in the tune's colour:
      // faintly all the way up, and brightly in a belt of storeys whose
      // height above the water glides with the pitch of the tune. A note
      // starting plucks the belt brighter. Some rooms stay dark anyway.
      // Turned down, a column comes on more gradually as its note gets
      // stronger, so it flickers less.
      let lit = sstep(mix(0.0, 0.12, uReact), mix(1.0, 0.7, uReact), chroma(i32(id) * 2 + i32(side)));
      let rnd = hash21(vec2f(id * 2.0 + side + select(name, 0.0, k == 0) * 37.0, storey));
      let home = mix(0.7, step(0.3, rnd), detail);
      let belt = sstep(0.10, 0.03, abs((storey + 0.5) * fh - belt_y));
      let sung = lit * tune_on * (0.28 * home + belt * mix(home, 1.0, 0.6) * (2.0 + 1.5 * pluck));
      // A few rooms are lit whatever is playing: scenery, in the tower's own
      // colour.
      let late = mix(0.05, step(0.95, fract(rnd * 7.13)), detail) * 0.4;
      // (While idle nothing is playing, and the windows that breathe with
      // the idle tune do so in the scenery's colour too.)
      let room_col = mix(tint, vec3f(1.0), 0.25);
      let windows = built * (mix(tune_col, room_col, uIdle) * sung * mix(pane, 1.0, 0.3 * belt)
        + room_col * late * pane);
      let roof = sstep(2.5 * soft, 0.5 * soft, h - y);

      var c = vec3f(0.0);
      if (k == 0) {
        // The live row: glass that brightens towards a hot roof line.
        let g = clamp(y / max(h, 0.001), 0.0, 1.0);
        let glass = mix(1.0, 0.6 + 0.55 * pane, detail);
        c = tint * (0.04 + 0.30 * g * g) * glass + windows * 1.2 + tint * 1.8 * roof;
      } else {
        // History: silhouettes with a roof line as bright as the band was
        // loud, sinking into the haze with distance. Turned down, a note
        // lights the windows back here less brightly (there are nineteen
        // rows of them to flash at once) and the roof lines glow less.
        c = tint * 0.012 + windows * 0.35 * mix(0.4, 1.0, uReact) * (1.0 - far)
          + tint * (0.03 + 1.1 * v) * mix(0.6, 1.0, uReact) * sstep(0.0, 0.08, v) * roof
            * (1.0 - 0.6 * far) * sstep(0.0, 0.6, dep);
        c = mix(c, haze_col * 0.05, far * 0.85);
      }
      col += trans * a * c * refl;
      trans *= 1.0 - a;
      open_water *= 1.0 - a * (1.0 - below);
    }

    // The live row only: the quay it stands on, and on each tower a beacon
    // on a spire, at the peak-hold height.
    if (k == 0) {
      let tint = tower_tint(id + 0.5, 0.0);
      let quay = sstep(1.6 * pxl, 0.0, abs(hh)) * sstep(0.0, 1.0, bx) * sstep(BARS, BARS - 1.0, bx);
      col += trans * tint * 0.22 * quay;

      let pv = held;
      let ph = 0.012 + 1.2 * pv + cw * 0.3;
      let spire = sstep(1.2 * soft, 0.0, abs(lx)) * step(h, y) * step(y, ph)
        * mix(0.25, 1.0, clamp((y - h) / max(ph - h, 0.001), 0.0, 1.0));
      let dv = vec2f(lx, y - ph);
      let beacon = sstep(cw * 0.11 + soft, cw * 0.11 - soft, length(dv));
      // HATS: each hat makes the beacons of some treble towers glint in the
      // hats' colour, and each of those throws a spark a little way up its
      // own column: a tick, over almost before it is seen.
      var flare = 0.0;
      var spark = 0.0;
      let sr = max(cw * 0.05, 1.3 * soft);
      for (var j = 0; j < 4; j++) {
        let age = uHatAges[j];
        let env = hatEnv(j);
        let hs = hash22(vec2f(id, uHatN - f32(j)));
        let pick = step(0.45, hs.x);
        flare += pick * env;
        let rise = (0.5 + 0.5 * hs.y) * pace * age / (1.0 + 1.6 * age);
        let dd = vec2f(lx - (hs.y - 0.5) * cw * 0.45, y - ph - cw * 0.2 - rise);
        let head = exp(-(dd.x * dd.x + max(dd.y, 0.0) * max(dd.y, 0.0)) / (sr * sr));
        let trail = exp(-max(-dd.y, 0.0) / (sr + 0.05 * exp(-age * 5.0)));
        spark += pick * head * trail * env;
      }
      let treble = sstep(0.35, 0.8, f);
      // A four-pointed glint, kept inside the tower's own cell.
      let ad = abs(dv) / cw;
      let glint = max(exp(-ad.x * 9.0 - ad.y * 60.0), exp(-ad.x * 60.0 - ad.y * 9.0))
        * (1.0 - sstep(0.25, 0.5, ad.x));
      // At rest a beacon is scenery, in its tower's colour. No beacon over
      // an empty lot.
      col += trans * refl * sstep(0.02, 0.10, pv)
        * (tint * 0.12 * spire + mix(tint, vec3f(1.0), 0.3) * 1.7 * beacon
          + voiceCol(HATS) * treble * (1.0 - below) * (3.5 * flare * beacon + 3.0 * (flare * glint + spark)));
    }
    if (trans < 0.01) {
      break;
    }
  }

  // ---- Behind it all: the sky, and its reflection in the water ----
  let wet = sstep(px, -px, p.y - hy);
  if (trans > 0.003) {
    let ys = abs(p.y - hy);
    let ps = vec2f(p.x + wet * rip * ys * 0.3, hy + ys * (1.0 + wet * (rip * 0.8 + ring * 0.5)));
    // Water reflects most at a grazing angle, towards the horizon.
    let fresnel = mix(1.0, mix(0.85, 0.3, sstep(0.0, 1.2, sg)), wet);
    col += trans * fresnel * sky(ps, hy, cw, ar, px, 1.0 - wet);
  }
  // The wavefront of a rain ring catches the light of the towers above it;
  // the kick's ring is lit in the kick's own colour.
  let bx0 = p.x / cw + BARS * 0.5;
  col += open_water * wet * (tower_tint(bx0, 0.0) * max(crest, 0.0) * 0.8
    + voiceCol(KICK) * max(push, 0.0) * 1.1);

  // BASS: the harbour lights under the low towers. A bank of light in the
  // bass line's colour lies along the quay there and pools on the water in
  // front of it; it reaches further along the quay, higher up the towers and
  // deeper into the water as the bass line swells, and is gone without it.
  let hq = p.y - floor_y;
  let reach = mix(0.30, 0.66, uBassLine) * BARS;
  let along = sstep(reach, 0.06 * BARS, bx0) * sstep(-2.5, 0.5, bx0);
  if (along > 0.0 && abs(hq) < 0.9) {
    // Lamps need a street: the bank lies only where low towers have been
    // standing over the last second (a steady yes or no, not their height).
    let fx = SPEC_X0 + clamp(bx0, 0.0, BARS) * SPEC_DX;
    var stood = 0.0;
    for (var j = 0; j < 6; j++) {
      stood += histRow(fx, uHistHead - 1.0 - 11.0 * f32(j)) / 6.0;
    }
    let street = sstep(0.03, 0.22, stood);
    let bank = exp(-max(hq, 0.0) / (0.02 + 0.11 * uBassLine));
    let pool = exp(-max(-hq, 0.0) / (0.03 + 0.20 * uBassLine)) * (0.75 + 1.2 * rip);
    let kerb = exp(-abs(hq) / (0.006 + 0.012 * uBassLine));
    col += bass_col * uBassLine * (1.9 - 0.9 * uBassLine) * along * street
      * (0.36 * select(bank, pool, hq < 0.0) + 0.6 * kerb);
  }
  return col;
}
