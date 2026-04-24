fn main() {
 if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
  let mut res = winres::WindowsResource::new();
  res.set_icon("assets/icon-cli.ico");
  if let Err(err) = res.compile() {
   panic!("failed to embed Windows icon for unvet-app: {err}");
  }
 }
}
