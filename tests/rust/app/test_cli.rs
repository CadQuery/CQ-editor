use super::*;

fn parse(args: &[&str]) -> Result<StartupOptions, String> {
    parse_args(args.iter().map(|s| s.to_string()))
}

#[test]
fn parses_file_and_python() {
    let options = parse(&["--python", "/opt/py", "part.py"]).unwrap();
    assert_eq!(options.python.as_deref(), Some("/opt/py"));
    assert_eq!(options.file, Some(PathBuf::from("part.py")));
    assert!(parse(&["--bogus"]).is_err());
    assert!(parse(&["a.py", "b.py"]).is_err());
    assert!(parse(&["--python"]).is_err());
}

#[test]
fn window_icon_decodes_the_embedded_logo() {
    // The 256 px frame (PNG, like the 64 to 192 px ones) is the one decoded.
    let logo = image::load_from_memory_with_format(app::LOGO_ICO, image::ImageFormat::Ico)
        .unwrap()
        .to_rgba8();
    assert_eq!(logo.dimensions(), (256, 256));
    let corner = *logo.get_pixel(0, 0);
    assert!(logo.pixels().any(|pixel| *pixel != corner));

    let icon = window_icon();
    assert_eq!((icon.width, icon.height), (128, 128));
    assert_eq!(icon.rgba.len(), 128 * 128 * 4);
}
