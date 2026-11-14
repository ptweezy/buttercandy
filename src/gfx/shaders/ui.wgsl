// The whole interface is one instanced draw call of quads. Each instance is
// a rounded rectangle evaluated as a signed distance field, optionally
// textured (glyphs, icons, cover art) or made of glass.
//
// Glass is modelled as a real piece of it: a slab lying on the picture, flat
// on top and rounding off towards its rim. Its shape gives a surface normal,
// and the normal gives everything else: how the picture underneath is bent
// (refraction, with a little dispersion), where the light catches the edge
// (specular highlights), and how much of the surroundings the rim reflects
// (Fresnel).

struct Screen {
  size: vec2f,
  srgb_out: f32,
  time: f32,
  // Pointer position in pixels; negative when it is outside the window.
  mouse: vec2f,
  beat: f32,
  pad: f32,
}

@group(0) @binding(0) var<uniform> screen: Screen;
@group(0) @binding(1) var atlas: texture_2d<f32>;
// The finished visualizer frame, three ways: heavily blurred, as it is, and
// at half size (a very light blur).
@group(0) @binding(2) var backdrop: texture_2d<f32>;
@group(0) @binding(3) var art: texture_2d<f32>;
@group(0) @binding(4) var samp: sampler;
@group(0) @binding(5) var frame: texture_2d<f32>;
@group(0) @binding(6) var soft: texture_2d<f32>;

const KIND_RECT: f32 = 0.0;
const KIND_GLASS: f32 = 1.0;
const KIND_GLYPH: f32 = 2.0;
const KIND_IMAGE: f32 = 3.0;
const KIND_SHADOW: f32 = 4.0;
const KIND_ART: f32 = 5.0;

struct Instance {
  @location(0) rect: vec4f,    // x, y, w, h in pixels
  @location(1) uv: vec4f,      // atlas u0, v0, u1, v1; glass: frost, bevel, bend, glow
  @location(2) color: vec4f,   // straight-alpha sRGB
  @location(3) color2: vec4f,  // gradient end colour
  @location(4) grad: vec4f,    // gradient axis: x0, y0, x1, y1 (equal = solid)
  @location(5) clip: vec4f,    // x0, y0, x1, y1
  @location(6) params: vec4f,  // corner radius, border width (glass: pixel scale), kind, extra
  @location(7) border: vec4f,  // border colour
}

struct Varyings {
  @builtin(position) pos: vec4f,
  @location(0) px: vec2f,
  @location(1) uv: vec2f,
  @location(2) @interpolate(flat) rect: vec4f,
  @location(3) @interpolate(flat) color: vec4f,
  @location(4) @interpolate(flat) color2: vec4f,
  @location(5) @interpolate(flat) grad: vec4f,
  @location(6) @interpolate(flat) clip: vec4f,
  @location(7) @interpolate(flat) params: vec4f,
  @location(8) @interpolate(flat) border: vec4f,
  @location(9) @interpolate(flat) glass: vec4f,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> Varyings {
  let corner = vec2f(f32(vi & 1u), f32((vi >> 1u) & 1u));
  let kind = inst.params.z;
  // Grow the quad so antialiased edges and soft shadows are not cut off.
  var pad = 1.0;
  if (kind == KIND_SHADOW) {
    pad = inst.params.w * 2.0 + 1.0;
  } else if (kind == KIND_GLYPH || kind == KIND_IMAGE) {
    pad = 0.0;
  }
  let lo = inst.rect.xy - pad;
  let hi = inst.rect.xy + inst.rect.zw + pad;
  let px = mix(lo, hi, corner);

  var out: Varyings;
  out.pos = vec4f(px.x / screen.size.x * 2.0 - 1.0, 1.0 - px.y / screen.size.y * 2.0, 0.0, 1.0);
  out.px = px;
  out.uv = mix(inst.uv.xy, inst.uv.zw, (px - inst.rect.xy) / max(inst.rect.zw, vec2f(1e-6)));
  out.rect = inst.rect;
  out.color = inst.color;
  out.color2 = inst.color2;
  out.grad = inst.grad;
  out.clip = inst.clip;
  out.params = inst.params;
  out.border = inst.border;
  out.glass = inst.uv;
  return out;
}

// Signed distance to a rounded rectangle (x), and the direction in which it
// grows fastest (yz): the outward normal of the nearest edge.
fn sd_round_rect(p: vec2f, half: vec2f, r: f32) -> vec3f {
  let s = vec2f(select(-1.0, 1.0, p.x >= 0.0), select(-1.0, 1.0, p.y >= 0.0));
  let q = abs(p) - half + r;
  if (q.x > 0.0 && q.y > 0.0) {
    let l = max(length(q), 1e-5);
    return vec3f(l - r, q / l * s);
  }
  if (q.x > q.y) {
    return vec3f(q.x - r, s.x, 0.0);
  }
  return vec3f(q.y - r, 0.0, s.y);
}

fn luma(c: vec3f) -> f32 {
  return dot(c, vec3f(0.299, 0.587, 0.114));
}

// The visualizer behind `px`, blurred over about `radius` pixels: a centre
// tap and two rings on the half-size copy.
fn frosted(px: vec2f, radius: f32) -> vec3f {
  let uv = px / screen.size;
  let o = radius / screen.size;
  var ring = array<vec2f, 12>(
    vec2f(1.0, 0.0), vec2f(0.707, 0.707), vec2f(0.0, 1.0), vec2f(-0.707, 0.707),
    vec2f(-1.0, 0.0), vec2f(-0.707, -0.707), vec2f(0.0, -1.0), vec2f(0.707, -0.707),
    vec2f(0.46, 0.19), vec2f(-0.19, 0.46), vec2f(-0.46, -0.19), vec2f(0.19, -0.46),
  );
  var c = textureSampleLevel(soft, samp, uv, 0.0).rgb * 2.0;
  for (var i = 0; i < 12; i++) {
    c += textureSampleLevel(soft, samp, uv + ring[i] * o, 0.0).rgb;
  }
  return c / 14.0;
}

// The picture behind `px` seen through clear glass that bends light by
// `shift` pixels. Glass bends blue more than red, so each colour is looked
// up a little further along: five taps across the spectrum.
fn refracted(px: vec2f, shift: vec2f, spread: f32) -> vec3f {
  var hue = array<vec3f, 5>(
    vec3f(0.5, 0.0, 0.0), vec3f(0.4, 0.2, 0.0), vec3f(0.1, 0.6, 0.1), vec3f(0.0, 0.2, 0.4), vec3f(0.0, 0.0, 0.5),
  );
  var c = vec3f(0.0);
  for (var i = 0; i < 5; i++) {
    let s = f32(i) * 0.5 - 1.0;
    c += textureSampleLevel(frame, samp, (px + shift * (1.0 + spread * s)) / screen.size, 0.0).rgb * hue[i];
  }
  return c;
}

// A piece of glass lying on the picture: a slab that is flat on top, with
// a rounded lip `bevel` wide around its edge. Nearly all of the bending of
// light happens in that lip, and the highlights sit on it; the flat top
// only softens and tints what is behind. `d` and `g` are the signed
// distance to the outline and the outward direction there; `local` is the
// position relative to the centre and `half` the half-size.
fn glass(in: Varyings, d: f32, g: vec2f, local: vec2f, half: vec2f, radius: f32, tint: vec4f) -> vec3f {
  // One interface pixel, in screen pixels.
  let u = max(in.params.y, 0.5);
  let short = min(half.x, half.y);
  let bevel = clamp(in.glass.y * u, 1.0, short);
  let bend = in.glass.z;
  let glow = in.glass.w;
  let size = screen.size;

  // ---- shape ---------------------------------------------------------------
  // The lip may be wider than the corners are round. Measure it from an
  // outline whose corners are at least as round as it is wide, so the
  // surface has no crease where two sides meet.
  let profile = sd_round_rect(local, half, max(radius, bevel));
  let out = profile.yz;
  // Across the lip the surface follows a squircle, (1 - (1-x)^4)^(1/4):
  // it drops away steeply right at the edge and has all but levelled off a
  // third of the way in, so the lens is thin and strong and meets the flat
  // top without a visible seam. +z faces the viewer.
  let x = clamp(-profile.x / bevel, 0.0, 1.0);
  let q = 1.0 - x;
  let q4 = q * q * q * q;
  let rise = max(1.0 - q4, 1e-4);
  let height = sqrt(sqrt(rise));
  let slope = min(q * q * q / (sqrt(rise) * height), 6.0);
  let n = normalize(vec3f(out * slope, 1.0));

  // ---- refraction ----------------------------------------------------------
  // Snell's law for glass of index 1.5: a ray from the eye bends towards
  // the normal as it enters, then travels down through the slab to the
  // picture. `bend` is how thick the slab is, in lip-widths. In the outer
  // third of the lip the bend is strong enough that the picture folds over
  // on itself: the edge shows a narrow mirrored sliver of what is just
  // inside it, as the edge of a thick piece of glass does.
  let ray = refract(vec3f(0.0, 0.0, -1.0), n, 1.0 / 1.5);
  let path = (height + 0.15) * bevel * bend;
  let shift = ray.xy / max(-ray.z, 0.3) * path;
  let at = in.px + shift;

  // The top is lightly sanded and the lip polished, so the bent sliver
  // stays sharp. Frost 0 is clear, 0.5 a soft haze, 1 opal.
  let frost = clamp(in.glass.x, 0.0, 1.0) * sstep(0.0, 0.45, x);
  let clear = refracted(in.px, shift, 0.07);
  let hazy = frosted(at, 22.0 * frost * u);
  let milky = textureSampleLevel(backdrop, samp, at / size, 0.0).rgb;
  let w_clear = clamp(1.0 - frost / 0.1, 0.0, 1.0);
  let w_milky = clamp((frost - 0.65) / 0.35, 0.0, 1.0);
  var behind = clear * w_clear + hazy * (1.0 - w_clear - w_milky) + milky * w_milky;
  // Glass makes what is behind it a little richer.
  behind = mix(vec3f(luma(behind)), behind, 1.22);

  // ---- tint ----------------------------------------------------------------
  // An even film over everything. A dark film deepens over a bright
  // picture, so that light text on it stays readable whatever the music
  // is doing; over black it lifts the glass just clear of the background.
  let glare = sstep(0.15, 0.7, luma(milky)) * (1.0 - luma(tint.rgb));
  var rgb = mix(behind, tint.rgb, min(tint.a + 0.4 * glare * (1.0 - tint.a), 1.0));

  // ---- light ---------------------------------------------------------------
  // The key light sits up and to the left, and leans towards the pointer.
  var lean = vec2f(0.0);
  var near = 0.0;
  if (screen.mouse.x >= 0.0) {
    lean = (screen.mouse / size - 0.5) * vec2f(0.7, 0.4);
    let m = (in.px - screen.mouse) / (120.0 * u);
    near = exp(-dot(m, m));
  }
  let key = normalize(vec2f(-0.57 + lean.x, -0.82 + lean.y));
  let lit = 1.0 + 0.3 * glow + 0.35 * near + 0.2 * screen.beat;
  // 1 where the edge faces the light, -1 where it faces away.
  let facing = dot(g, key);
  let lip = 1.0 - sstep(0.0, 0.4, x);

  // The lip is a little brighter on the side the light comes from and a
  // little darker on the far side, which is what makes the edge read as
  // rounded rather than drawn.
  rgb *= 1.0 - 0.16 * lip * max(-facing, 0.0);
  rgb += vec3f(0.05) * lip * max(facing, 0.0);

  // What does not pass through is mirrored (Fresnel, Schlick's fit): the
  // steepest part of the lip carries a faint reflection of the picture
  // just outside the glass.
  let fresnel = 0.04 + 0.96 * pow(1.0 - n.z, 5.0);
  let mirrored = textureSampleLevel(soft, samp, (in.px + out * (0.6 * bevel + 2.0 * u)) / size, 0.0).rgb;
  rgb = mix(rgb, mirrored, 0.5 * fresnel * sstep(0.7, 0.45, n.z));

  // The polished edge: a hairline of light, brightest where it faces the
  // key light and again diagonally opposite, faint in between.
  let catchlight = pow(max(facing, 0.0), 2.4) + 0.75 * pow(max(-facing, 0.0), 3.2);
  let depth = max(-d - 0.5, 0.0);
  let hairline = exp(-depth / (0.75 * u)) + 0.06 * exp(-depth / (3.0 * u));
  let rim = in.border.a * hairline * (0.22 + 2.5 * catchlight) * lit;
  rgb = mix(rgb, in.border.rgb, min(rim, 0.9));

  // A breath of light across the top, and a pool of it under the pointer.
  let sheen = clamp(1.0 - (in.px.y - in.rect.y) / (48.0 * u), 0.0, 1.0);
  rgb += vec3f(0.018 * sheen * sheen + 0.03 * glow + 0.03 * near);
  return rgb;
}

@fragment
fn fs_main(in: Varyings) -> @location(0) vec4f {
  if (in.px.x < in.clip.x || in.px.y < in.clip.y || in.px.x > in.clip.z || in.px.y > in.clip.w) {
    discard;
  }
  let kind = in.params.z;
  let half = in.rect.zw * 0.5;
  let local = in.px - (in.rect.xy + half);
  let radius = min(in.params.x, min(half.x, half.y));
  let sd = sd_round_rect(local, half, radius);
  let d = sd.x;
  let coverage = clamp(0.5 - d, 0.0, 1.0);

  // Fill colour, optionally a linear gradient along the given axis.
  var fill = in.color;
  let axis = in.grad.zw - in.grad.xy;
  let axis_len = dot(axis, axis);
  if (axis_len > 0.0) {
    let t = clamp(dot(in.px - in.grad.xy, axis) / axis_len, 0.0, 1.0);
    fill = mix(in.color, in.color2, t);
  }

  var rgb = fill.rgb;
  var alpha = fill.a;

  if (kind == KIND_RECT) {
    let bw = in.params.y;
    if (bw > 0.0) {
      // Blend from the border colour into the fill across the border's inner edge.
      let inner = clamp(0.5 - (d + bw), 0.0, 1.0);
      let b = in.border;
      let pre = mix(b.rgb * b.a, fill.rgb * fill.a, inner);
      alpha = mix(b.a, fill.a, inner);
      rgb = pre / max(alpha, 1e-5);
    }
    alpha *= coverage;
  } else if (kind == KIND_GLASS) {
    rgb = glass(in, d, sd.yz, local, half, radius, fill);
    alpha = coverage * in.params.w;
  } else if (kind == KIND_GLYPH) {
    alpha *= textureSampleLevel(atlas, samp, in.uv, 0.0).a;
  } else if (kind == KIND_IMAGE) {
    let t = textureSampleLevel(atlas, samp, in.uv, 0.0);
    rgb = t.rgb;
    alpha = t.a * fill.a;
  } else if (kind == KIND_ART) {
    rgb = textureSampleLevel(art, samp, in.uv, 0.0).rgb;
    alpha = coverage * fill.a;
  } else {
    // Shadow / glow: a rounded rect with a soft falloff of `extra` pixels.
    let blur = max(in.params.w, 0.001);
    let t = clamp(0.5 - d / (2.0 * blur), 0.0, 1.0);
    alpha *= t * t * (3.0 - 2.0 * t);
  }

  var out = vec4f(rgb * alpha, alpha);
  if (screen.srgb_out > 0.5) {
    out = vec4f(pow(out.rgb, vec3f(2.2)), out.a);
  }
  return out;
}

// smoothstep that is well defined whichever way round the edges are.
fn sstep(a: f32, b: f32, x: f32) -> f32 {
  let t = clamp((x - a) / (b - a), 0.0, 1.0);
  return t * t * (3.0 - 2.0 * t);
}
