use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let mut root: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);

    while let Some(argument) = args.next() {
        if argument == "--root" {
            match args.next() {
                Some(value) => root = Some(PathBuf::from(value)),
                None => {
                    eprintln!("--root requires a directory");
                    std::process::exit(2);
                }
            }
        } else if let Some(value) = argument.strip_prefix("--root=") {
            root = Some(PathBuf::from(value));
        } else if argument == "--help" || argument == "-h" {
            println!("My Linux {VERSION}");
            println!();
            println!("Usage: my-linux-gui [--root <directory>]");
            println!();
            println!(
                "  --root <directory>  Installation root (default: $MY_LINUX_ROOT or the XDG state directory)"
            );

            return Ok(());
        } else {
            eprintln!("Unknown argument: {argument}");
            std::process::exit(2);
        }
    }

    let root = root.unwrap_or_else(my_linux_gui::default_root);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(format!("{} {VERSION}", my_linux_gui::APP_NAME))
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        my_linux_gui::APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(my_linux_gui::MyLinuxApp::new(cc, root)))),
    )
}

const VERSION: &str = env!("CARGO_PKG_VERSION");
