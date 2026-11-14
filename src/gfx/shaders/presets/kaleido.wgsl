// Kaleido: neon line-work folded through a tunnel of kaleidoscope dials.
//
// The picture is a tunnel seen end-on: concentric dials, each 1.9 times the
// size of the one inside it, and each a kaleidoscope of its own with its own
// number of mirrors (four to eight). Inside a dial the wedge is folded again
// and again, every fold drawing its mirror lines and a circle, every level
// finer than the last. There are twelve dials, one for each step round the
// circle of fifths, and then the tunnel repeats.
//
// The five voices (see preset_header.wgsl). Each lights the line-work in its
// own colour, voiceCol(VOICE), and only while it sounds:
//
//   voice  what it does                         where                colour
//   -----  -----------------------------------  -------------------  ------
//   KICK   pushes the whole figure outward      the whole picture,   KICK
//          (kickSum) and sends a ring out       from the centre
//          through the lines, thickening and
//          lighting each as it passes; fires
//          the light at the tunnel's far end
//   SNARE  a crack (snareEnv): one straight     across one diameter  SNARE
//          line through the centre, and the     of twelve, a new
//          lines it crosses flash               one each hit
//   HATS   a tick (hatEnv): a scatter of        round the edge       HATS
//          pin-point glints, and the finest
//          lace glints round each (open or
//          not); a new scatter each tick
//   BASS   swells with uBassLine: the big       the outer part of    BASS
//          bones grow heavier and glow          the figure
//   TUNE   which dial is in view (uHarmony);    a band in the        TUNE
//          the sounding notes (chroma) swell    middle, moving
//          and light their beads on the rims    outward as the
//          and throw beams across their         pitch rises
//          sectors; a new note plucks them      (uPitch)
//          brighter (noteSum)
//
// The tune in more detail. Harmony is where we are in the tunnel: a chord
// change flies the view to that chord's dial (a fifth is one dial), so the
// symmetry on screen is set by the harmony and changes by travel, never by a
// cut. Twelve beads sit on every rim like the hours of a clock, C at the top,
// semitones clockwise, so a chord is a figure of lit beads and beams. A kick
// and a bass note have a pitch class too, so the beads only answer while
// there is a tune (uTune).
//
// Scenery, in pal() colours, slow, and dimmer than anything a voice lights:
//
// - The line-work itself, the rims, the unlit beads, the fringes where the
//   waves ringing off the two coarsest levels interfere, and a small light
//   at the far end of the tunnel.
// - Fullness (uEnergy): how deep the recursion goes. A breakdown is one or
//   two levels, sparse and crystalline; a full mix is seven, a little heavier
//   and with more fringes.
// - Brightness (uCentroid): tension. A riser pulls the view back out of the
//   tunnel, winds the dials against each other, thins the lines and lifts
//   the finest levels to full strength; the drop lets it all go at once.
// - Idle: no events, and the voices' colours are put away. The ambient
//   harmony drifts round the circle, so the view falls steadily through all
//   twelve dials, and its notes light their beads and beams on every rim in
//   the palette's own colours.
//
// Reactivity (uReact): the envelopes carry it. Besides them, the ring is
// dimmer and travels slower below 1, the bass's swell is lighter, and the
// riser's pull and winding shrink fastest of all. The flight between dials is
// not scaled: it is as fast as uHarmony glides.
//
// The symmetry is nearly, not quite, perfect: the far end of the tunnel sways
// off the axis and the fold offsets tilt slowly across the figure.
//
// Line widths are measured in pixels: a line thinner than a pixel is drawn a
// pixel wide and dimmed, and a level too small to resolve fades out before it
// can alias. The afterglow is the previous frame pushed outward and squeezed
// to a dim veil, so the lines themselves stay sharp; a voice's light settles
// into it for a moment after the voice has gone.

// Dials in one trip round the circle of fifths, and the size ratio between
// neighbouring dials.
const DIALS: f32 = 12.0;
const RATIO: f32 = 1.9;
// Fold levels inside a dial, and how much finer each is than the last.
const LEVELS: i32 = 7;
const LSCALE: f32 = 1.6;

// Mirrors in dial k. Neighbours always differ.
fn dialOrder(k: f32) -> f32 {
  var T = array<f32, 12>(5.0, 6.0, 8.0, 5.0, 7.0, 4.0, 6.0, 8.0, 5.0, 7.0, 6.0, 4.0);
  return T[clamp(i32(k), 0, 11)];
}

// Coverage of a line of half-width w at distance d, where one pixel is px. A
// line thinner than a pixel is drawn a pixel wide and dimmed instead.
fn stroke(d: f32, w: f32, px: f32) -> f32 {
  let ww = max(w, 0.5 * px);
  return clamp((ww - d) / px + 0.5, 0.0, 1.0) * min(1.0, w / (0.5 * px));
}

fn scene(uv: vec2f) -> vec3f {
  let ps = aspect(uv);
  let px = 2.0 / uRes.y;
  let live = 1.0 - uIdle;

  // Reactivity, for what is not one of the shared envelopes (those carry it
  // already). `riser` sizes the pull and winding of a riser, which shifts the
  // whole figure (a sixteenth at the default of 0.5). `pace` slows the kick's
  // ring and `amp` sizes it as the envelopes size a hit: the ring has to be
  // timed from the kick's age, because it travels. `heft` is how much weight
  // the bass line puts on (1 at the default).
  let riser = pow(mix(0.2, 1.0, uReact * uReact), 3.0);
  let pace = mix(0.5, 1.0, uReact);
  let amp = mix(0.35, 1.0, uReact);
  let heft = mix(0.67, 1.33, uReact);

  // The tunnel is not quite straight: its far end sways off the axis.
  let sway = 0.08 * vec2f(cos(uTime * 0.19), sin(uTime * 0.23));
  let p = ps - sway * exp(-1.2 * dot(ps, ps));
  let r = max(length(p), 1e-5);
  let ang = atan2(p.y, p.x);
  let lr = log(r) / log(RATIO);

  // Tension: how far the sound has climbed above an ordinary mix.
  let tension = sstep(0.46, 0.68, uCentroid) * live;

  // KICK: a push down the tunnel, and a ring that runs out from the centre
  // to the edge, fading as it goes.
  let push = kickSum();
  var ring = 0.0;
  for (var k = 0; k < 4; k++) {
    let a = uKickAges[k];
    let x = (log(r) + 3.2 - 6.5 * pace * a) / 0.32;
    ring += exp(-x * x) * exp(-a * 2.0) * uKickHits[k];
  }
  ring = min(ring * amp, 1.0);

  // BASS: the outer part of the figure. A little low end left between the
  // kicks is not a bass line, so the bottom of the range is pressed down.
  let bass = sstep(0.1, 0.85, uBassLine) * sstep(0.65, 1.15, r);

  // HATS: each tick scatters pin-points round the edge, about one cell in
  // five of a grid of forty round and six to each doubling of the radius,
  // and the fine lace round each point glints (`spots`). Every tick draws its
  // own scatter.
  let outer = sstep(0.62, 0.95, r);
  var spots = 0.0;
  var stars = 0.0;
  if (outer > 0.0) {
    let gc = vec2f(ang / TAU * 40.0, log(r) * 6.0);
    for (var k = 0; k < 4; k++) {
      let e = hatEnv(k) * outer;
      if (e < 0.003) { continue; }
      let sd = uHatN - f32(k);
      let g = gc + vec2f(0.0, hash21(vec2f(sd, 3.1)));
      let id = vec2f(fmod(floor(g.x), 40.0), floor(g.y));
      let h = hash22(id + 1.7 * sd);
      if (h.x < 0.82) { continue; }
      let c = fract(g) - 0.5 - 0.3 * (vec2f(h.y, fract(h.x * 57.0)) - 0.5);
      let d2 = dot(c, c);
      spots += e * exp(-d2 * 24.0);
      stars += e * exp(-d2 * 300.0);
    }
  }

  // Where we are in the tunnel, in dials. The harmony sets it (cyclic: one
  // trip round the circle of fifths is one trip through all twelve dials);
  // the kick pushes it forward and tension pulls it back.
  let flow = uHarmony * DIALS + uPhase * 0.03 + 0.06 * push - 0.9 * riser * tension;

  // Which dial this pixel is in, and that dial's outer radius.
  let o = flow - lr;
  let zk = floor(o);
  let zf = o - zk;
  let zm = fmod(zk, DIALS);
  let R = r * pow(RATIO, zf);
  let order = dialOrder(zm);
  let wedge = TAU / order;
  let hw = 0.5 * wedge;

  // The dials turn slowly, neighbours in opposite directions, and a riser
  // winds them against each other.
  let dir = 1.0 - 2.0 * fmod(zm, 2.0);
  let turn = dir * (uTime * 0.025 + 1.2 * riser * tension) + zm * TAU * 5.0 / DIALS;

  // Fold into a wedge, in the dial's own units (its rim is at 1).
  let a = abs(fmod(ang + turn, wedge) - hw);
  var q = (r / R) * vec2f(cos(a), sin(a));

  // TUNE: the twelve notes own twelve sectors, C at the top, rising
  // clockwise. A kick and a bass note have a pitch class too, so a note only
  // counts while there is a tune. The tune's band is a ring in the middle of
  // the figure that moves outward as the pitch rises.
  let pc = fract(0.25 - ang / TAU + 0.5 / 12.0) * 12.0;
  let pci = floor(pc);
  let lobe = 0.5 - 0.5 * cos(TAU * pc);
  let ch = chroma(i32(pci)) * sstep(0.05, 0.4, uTune);
  let band = exp(-pow(log(r / mix(0.2, 0.65, uPitch)) / 0.5, 2.0));
  let sing = ch * mix(band * (1.0 + 0.8 * noteSum()), 1.0, uIdle);
  let beam = sing * lobe * lobe;

  // Nearly, not quite, symmetric: the fold offsets tilt across the figure.
  let tilt = 0.03 * dot(p, vec2f(cos(uTime * 0.11), sin(uTime * 0.11)));

  // How many levels are open.
  let full = sstep(0.35, 0.9, uEnergy);
  let depth = mix(mix(1.5, 7.0, full) + 1.5 * tension, 2.6 + 0.5 * sin(uTime * 0.23), uIdle);
  let sparse = mix(1.0 - full, 1.0, uIdle);
  // Lines are a little finer in the small, far dials.
  let thick = pow(clamp(R, 0.05, 1.3), 0.4);
  // The line-work is a little heavier in a full mix.
  let body = 0.3 + 0.25 * full;

  // How far each voice takes over the lines it lights. With nothing playing
  // the voices' colours are put away.
  let kick = min(2.5 * ring, 1.0);
  let low = bass * live;
  let tune = min(1.8 * sing * lobe, 1.0) * live;

  var scenery = vec3f(0.0);
  var lines = 0.0;
  var bones = 0.0;
  var wake = 0.0;
  var lace = 0.0;
  var scale = 1.0 / R;
  var field = 0.0;
  let lam = max(0.018, 6.0 * px);
  let ph = uPhase * 0.1 + zm * 2.4;
  let hue0 = zm * 3.0 / DIALS - uTime * 0.02;
  for (var i = 0; i < LEVELS; i++) {
    let fi = f32(i);
    // A level fades in with the depth, and out when it is too small to draw.
    // The hats can glint on lace that is not open.
    let fine = sstep(3.0 * px, 9.0 * px, 0.3 / scale);
    let wgt = sstep(fi - 0.5, fi + 0.5, depth) * fine;
    let glintW = fine * sstep(0.5, 2.5, fi) * spots;
    if (wgt < 0.002 && glintW < 0.002) { break; }
    let off = vec2f(0.60 + 0.10 * sin(ph * 1.1 + fi * 1.3), 0.14 + 0.10 * sin(ph * 0.7 + fi * 2.1));
    q = abs(q) - off - tilt * vec2f(sin(fi * 2.0), cos(fi * 2.0));
    // The kick's ring thickens every line it passes; the bass line thickens
    // the big bones of the outer part.
    let bone = pow(0.35, fi);
    let w = 0.0058 * pow(0.64, fi) * (0.75 + 0.6 * body + 0.9 * ring + 0.6 * heft * bass * bone) * (1.0 - 0.3 * tension) * thick;
    let dl = min(abs(q.x), abs(q.y)) / scale;
    let dc = abs(length(q) - (0.20 + 0.08 * sin(ph * 0.5 + fi * 0.7))) / scale;
    let d = min(dl, dc);
    let core = stroke(dl, w, px) + stroke(dc, w * 0.7, px) + 0.6 * stroke(d, w * 0.35, px);
    let halo = exp(-d / (w * 5.0 + 0.004));
    // The hue slides along each line, and the halo sits a step further round
    // the palette than its line, like a thin film.
    let hue = hue0 + fi * 0.12 + 0.07 * sin(q.x * 5.0 + q.y * 3.0 + uTime * 0.3);
    let gain = 0.7 * (0.5 + 0.3 * sparse + 0.6 * body) * pow(0.68 + 0.27 * tension, fi) * (1.0 + 0.5 * tension);
    let light = core * gain * wgt;
    scenery += pal(hue) * light * (1.0 - 0.85 * low * bone) + pal(hue + 0.18) * halo * 0.05 * (0.4 + body) * wgt;
    lines += light;
    bones += (light + 0.10 * halo * wgt) * bone;
    wake += (light + 0.06 * halo * wgt) * pow(0.8, fi);
    lace += core * glintW;
    // The two coarsest levels ring: waves leave their lines and interfere.
    if (i < 2) {
      field += cos(TAU * (d / lam - uPhase * 0.35)) * exp(-d * 7.0 / thick) * wgt;
    }
    q = rot(hw) * q;
    q *= LSCALE;
    scale *= LSCALE;
  }

  // Scenery: the line-work in the palette's colours. Where a voice lights
  // the lines it takes them over. With nothing playing the notes of the
  // ambient harmony throw their beams in the scenery's own colours.
  var col = scenery * mix(0.36, 0.7 + 0.9 * beam, uIdle) + voiceCol(BASS) * bones * low * 1.4 * heft;
  col = col * (1.0 - 0.85 * max(kick, tune)) + voiceCol(TUNE) * lines * tune * 2.2;
  col += voiceCol(KICK) * wake * kick * (1.2 + 2.0 * ring);
  col += voiceCol(HATS) * (lace * 4.0 + stars * 12.0);

  // Interference between the waves from the two coarsest levels.
  col += pal(hue0 + 0.6) * field * field * 0.035 * (0.4 + 0.5 * full) * sstep(0.03, 0.2, R);

  // Dial rims: a dark groove with a fine line round it, which the tune's
  // notes light sector by sector.
  let rin = R / RATIO;
  let inner = zf >= 0.5;
  let rimR = select(R, rin, inner);
  let rimHue = fmod(zm + select(0.0, 1.0, inner), DIALS) * 3.0 / DIALS - uTime * 0.02 + 0.5;
  let groove = min(R - r, r - rin);
  let edge = abs(r - rimR);
  let rimLine = stroke(edge, 0.0022 * sqrt(clamp(rimR, 0.0, 1.5)), px);
  let tuneCol = mix(pal(rimHue), voiceCol(TUNE), live);
  col *= sstep(0.0, 0.05 * rimR, groove);
  col += (pal(rimHue) * (0.25 + 0.15 * full) + tuneCol * 1.2 * beam) * rimLine;

  // Twelve beads on every rim, one per note. A sounding note of the tune
  // swells and lights its bead. Beads too small to draw fade out.
  let dAng = (fract(pc) - 0.5) * TAU / 12.0;
  let bd = length(vec2f(r * sin(dAng), r * cos(dAng) - rimR));
  let br = rimR * (0.009 + 0.040 * min(sing, 1.3));
  let bcol = pal(fmod(pci * 7.0, 12.0) / 12.0);
  let lit = mix(bcol, voiceCol(TUNE), live);
  let bz = clamp(1.0 - bd * bd / (br * br), 0.0, 1.0);
  let bvis = sstep(0.4 * px, 1.6 * px, br);
  col = mix(col, (bcol * 0.10 + lit * 2.6 * sing) * (0.3 + 0.7 * bz * sqrt(bz)), clamp((br - bd) / px + 0.5, 0.0, 1.0) * bvis);
  col += lit * sing * sing * 0.45 * exp(-bd / (rimR * 0.035)) * sstep(0.0, 0.08, rimR);

  // SNARE: a crack straight across one diameter, a different one each time,
  // lighting the lines it crosses.
  var crack = 0.0;
  var cut = 0.0;
  for (var k = 0; k < 4; k++) {
    let e = snareEnv(k);
    let th = (floor(hash21(vec2f(uSnareN - f32(k), 2.7)) * 12.0) + 0.5) * PI / 12.0;
    let dn = abs(dot(p, vec2f(-sin(th), cos(th))));
    crack += e * (4.5 * stroke(dn, 0.0035, px) + 0.4 * exp(-dn / 0.025));
    cut += e * exp(-dn / 0.09);
  }
  col += voiceCol(SNARE) * (crack + 3.0 * cut * lines);

  // The light at the far end of the tunnel, which the kick fires.
  col += pal(uHarmony) * 0.2 * exp(-r / 0.02) + voiceCol(KICK) * push * 1.2 * exp(-r / 0.035);

  col *= sstep(2.6, 1.0, length(ps));

  // Afterglow: the last frame, pushed a little way out of the tunnel and
  // squeezed (bright things most), so it trails the lines as a dim veil.
  // Decay and squeeze both scale with FR, and both only ever shrink it.
  let pe = prev(unaspect(sway + (ps - sway) * (1.0 - 0.003 * FR)));
  let pm = max(pe.r, max(pe.g, pe.b));
  let echo = pe * pow(0.93, FR) / (1.0 + FR * pm / 0.5);
  return max(col, echo);
}
