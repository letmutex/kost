use crate::style::{push_dimmed, push_header, push_option, push_placeholder};

pub fn render_dep_help() -> String {
    render_dep_help_styled(common::should_colorize_stdout())
}

pub fn render_dep_help_styled(colorize: bool) -> String {
    let mut buffer = String::new();
    buffer.push_str("kost dep - Fast JVM-free Gradle dependency resolver & fetcher\n\n");

    push_header(&mut buffer, "USAGE:", colorize);
    buffer.push_str("\n    kost dep ");
    push_placeholder(&mut buffer, "<SUBCOMMAND>", colorize);
    buffer.push_str("\n    kost dep ");
    push_placeholder(&mut buffer, "[OPTIONS]", colorize);
    buffer.push_str("\n\n");

    push_header(&mut buffer, "SUBCOMMANDS:", colorize);
    buffer.push_str("\n    ");
    push_placeholder(&mut buffer, "fetch", colorize);
    buffer.push_str("       Download full project or specified dependencies to Gradle cache\n    ");
    push_placeholder(&mut buffer, "list", colorize);
    buffer.push_str("        List direct or solved dependencies\n    ");
    push_placeholder(&mut buffer, "tree", colorize);
    buffer.push_str("        Render full solved dependency hierarchy tree\n    ");
    push_placeholder(&mut buffer, "help", colorize);
    buffer.push_str("        Print this help message\n\n");

    render_dep_options(&mut buffer, colorize);
    render_dep_examples(&mut buffer, colorize);
    buffer
}

fn render_common_options(buffer: &mut String, colorize: bool) {
    push_option(buffer, "-p, --project-dir", colorize);
    buffer.push(' ');
    push_placeholder(buffer, "<PATH>", colorize);
    buffer.push_str("   Target Gradle project directory [default: .]\n    ");
    push_option(buffer, "--maven-repo", colorize);
    buffer.push(' ');
    push_placeholder(buffer, "<URL>", colorize);
    buffer.push_str("         Additional Maven repository URL\n    ");
    push_option(buffer, "--gradle-home", colorize);
    buffer.push(' ');
    push_placeholder(buffer, "<PATH>", colorize);
    buffer.push_str("       Custom Gradle user home directory\n    ");
    push_option(buffer, "--offline", colorize);
    buffer.push_str("                  Resolve strictly from local cache\n    ");
    push_option(buffer, "-h, --help", colorize);
    buffer.push_str("                 Print help information\n\n");
}

fn render_dep_options(buffer: &mut String, colorize: bool) {
    push_header(buffer, "OPTIONS:", colorize);
    buffer.push_str("\n    ");
    push_option(buffer, "-h, --help", colorize);
    buffer.push_str("                 Print help information\n\n");
}

fn render_dep_examples(buffer: &mut String, colorize: bool) {
    push_header(buffer, "EXAMPLES:", colorize);
    buffer.push_str("\n    ");
    push_dimmed(buffer, "kost dep fetch", colorize);
    buffer.push_str("                              # Download all repo dependencies\n    ");
    push_dimmed(
        buffer,
        "kost dep fetch androidx.compose.ui:ui:1.6.0",
        colorize,
    );
    buffer.push_str(" # Download specified library to Gradle cache\n    ");
    push_dimmed(buffer, "kost dep tree", colorize);
    buffer.push_str("                               # Visual tree of project dependencies\n    ");
    push_dimmed(buffer, "kost dep list", colorize);
    buffer.push_str("                               # List project dependencies\n");
}

pub fn render_fetch_help() -> String {
    render_fetch_help_styled(common::should_colorize_stdout())
}

pub fn render_fetch_help_styled(colorize: bool) -> String {
    let mut buffer = String::new();
    buffer.push_str(
        "kost dep fetch - Download project or specified dependencies to Gradle cache\n\n",
    );

    push_header(&mut buffer, "USAGE:", colorize);
    buffer.push_str("\n    kost dep fetch ");
    push_placeholder(&mut buffer, "[OPTIONS]", colorize);
    buffer.push(' ');
    push_placeholder(&mut buffer, "[COORDINATES...]", colorize);
    buffer.push_str("\n\n");

    push_header(&mut buffer, "OPTIONS:", colorize);
    buffer.push_str("\n    ");
    push_option(&mut buffer, "--sources", colorize);
    buffer.push_str("                  Download sources JARs\n    ");
    push_option(&mut buffer, "--no-transitive", colorize);
    buffer.push_str("            Only download direct artifacts\n    ");
    render_common_options(&mut buffer, colorize);

    push_header(&mut buffer, "EXAMPLES:", colorize);
    buffer.push_str("\n    ");
    push_dimmed(&mut buffer, "kost dep fetch", colorize);
    buffer.push_str("                              # Download all project dependencies\n    ");
    push_dimmed(
        &mut buffer,
        "kost dep fetch androidx.compose.ui:ui:1.6.0",
        colorize,
    );
    buffer.push_str(" # Download specified library\n    ");
    push_dimmed(&mut buffer, "kost dep fetch --sources", colorize);
    buffer.push_str("                    # Download dependencies with sources\n");
    buffer
}

pub fn render_list_help() -> String {
    render_list_help_styled(common::should_colorize_stdout())
}

pub fn render_list_help_styled(colorize: bool) -> String {
    let mut buffer = String::new();
    buffer.push_str("kost dep list - List direct or solved dependencies\n\n");

    push_header(&mut buffer, "USAGE:", colorize);
    buffer.push_str("\n    kost dep list ");
    push_placeholder(&mut buffer, "[OPTIONS]", colorize);
    buffer.push(' ');
    push_placeholder(&mut buffer, "[COORDINATES...]", colorize);
    buffer.push_str("\n\n");

    push_header(&mut buffer, "OPTIONS:", colorize);
    buffer.push_str("\n    ");
    push_option(&mut buffer, "--direct", colorize);
    buffer.push_str("                   List direct dependencies without solving\n    ");
    push_option(&mut buffer, "--cached-only", colorize);
    buffer.push_str("              List only cached dependencies\n    ");
    push_option(&mut buffer, "--missing-only", colorize);
    buffer.push_str("             List only un-downloaded dependencies\n    ");
    render_common_options(&mut buffer, colorize);

    push_header(&mut buffer, "EXAMPLES:", colorize);
    buffer.push_str("\n    ");
    push_dimmed(&mut buffer, "kost dep list --direct", colorize);
    buffer.push_str("                     # List declared dependencies from build scripts\n    ");
    push_dimmed(&mut buffer, "kost dep list --missing-only", colorize);
    buffer.push_str("               # Show un-downloaded dependencies\n");
    buffer
}

pub fn render_tree_help() -> String {
    render_tree_help_styled(common::should_colorize_stdout())
}

pub fn render_tree_help_styled(colorize: bool) -> String {
    let mut buffer = String::new();
    buffer.push_str("kost dep tree - Render full solved dependency hierarchy tree\n\n");

    push_header(&mut buffer, "USAGE:", colorize);
    buffer.push_str("\n    kost dep tree ");
    push_placeholder(&mut buffer, "[OPTIONS]", colorize);
    buffer.push(' ');
    push_placeholder(&mut buffer, "[COORDINATES...]", colorize);
    buffer.push_str("\n\n");

    push_header(&mut buffer, "OPTIONS:", colorize);
    buffer.push_str("\n    ");
    push_option(&mut buffer, "--depth", colorize);
    buffer.push(' ');
    push_placeholder(&mut buffer, "<N>", colorize);
    buffer.push_str("                Max display depth for dependency tree\n    ");
    push_option(&mut buffer, "--filter", colorize);
    buffer.push(' ');
    push_placeholder(&mut buffer, "<QUERY>", colorize);
    buffer.push_str("           Filter dependency tree matching coordinate\n    ");
    render_common_options(&mut buffer, colorize);

    push_header(&mut buffer, "EXAMPLES:", colorize);
    buffer.push_str("\n    ");
    push_dimmed(&mut buffer, "kost dep tree", colorize);
    buffer.push_str("                              # Visual tree of project dependencies\n    ");
    push_dimmed(&mut buffer, "kost dep tree --depth 2", colorize);
    buffer.push_str("                    # Limit tree to depth 2\n    ");
    push_dimmed(&mut buffer, "kost dep tree --filter compose", colorize);
    buffer.push_str("             # Filter tree for compose dependencies\n");
    buffer
}
