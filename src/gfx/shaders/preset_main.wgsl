
// The one pass of an ordinary preset: scene() is the picture, and the frame
// before is the picture it drew then.
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
  loadGlobals();
  var c = scene(in.uv);
  // Feedback presets re-read this buffer: never let a NaN or a runaway in.
  c = select(c, vec3f(0.0), c != c);
  return vec4f(clamp(c, vec3f(0.0), vec3f(32.0)), 1.0);
}
