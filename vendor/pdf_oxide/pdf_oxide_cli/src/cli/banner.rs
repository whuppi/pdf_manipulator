use super::colors;

/// Print the ASCII art banner to stderr with rust gradient colors.
pub fn print_banner() {
    //   ___  ___  ___    ___       _     _
    //  | _ \|   \| __|  / _ \__ __(_) __| | ___
    //  |  _/| |) | _|  | (_) \ \ /| |/ _` |/ -_)
    //  |_|  |___/|_|    \___//_\_\|_|\__,_|\___|
    //  ^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^^^^^^^^
    //  PDF (gradient)    Oxide (white)
    //  columns 0-15      columns 16+

    let pdf_parts = [
        "  ___  ___  ___ ",
        " | _ \\|   \\| __|",
        " |  _/| |) | _| ",
        " |_|  |___/|_|  ",
    ];

    let oxide_parts = [
        "   ___       _     _",
        "  / _ \\__ __(_) __| | ___",
        " | (_) \\ \\ /| |/ _` |/ -_)",
        "  \\___//_\\_\\|_|\\__,_|\\___|",
    ];

    let color_fns: [fn(&str) -> String; 4] = [
        colors::rust_orange,
        colors::rust_orange,
        colors::rust_dark,
        colors::rust_deep,
    ];

    for i in 0..4 {
        let colored_pdf = color_fns[i](pdf_parts[i]);
        let colored_oxide = colors::white(oxide_parts[i]);
        eprintln!("{colored_pdf}{colored_oxide}");
    }

    let version_line = format!("{:>42}", format!("v{}", pdf_oxide::VERSION));
    eprintln!("{}", colors::dim(&version_line));
    eprintln!();
}
