
// The two passes of a preset that keeps hidden state. First state() writes
// four numbers a pixel, which are kept and never shown; then show() makes
// the picture from them.
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
  loadGlobals();
  var s = state(in.uv);
  s = select(s, vec4f(0.0), s != s);
  return clamp(s, vec4f(-60000.0), vec4f(60000.0));
}

@fragment
fn fs_show(in: VsOut) -> @location(0) vec4f {
  loadGlobals();
  var c = show(in.uv);
  c = select(c, vec3f(0.0), c != c);
  return vec4f(clamp(c, vec3f(0.0), vec3f(32.0)), 1.0);
}
