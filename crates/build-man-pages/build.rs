use std::path::Path;

use fish_build_helper::{as_os_strs, fish_doc_dir};

fn main() {
    let doc_dir = fish_doc_dir();
    let sec1_dir = doc_dir.join("man").join("man1");
    let help_dir = doc_dir.join("help");
    std::fs::create_dir_all(&sec1_dir).unwrap();
    std::fs::create_dir_all(&help_dir).unwrap();
    if !cfg!(clippy) {
        build_docs(&sec1_dir, "man");
        build_docs(&help_dir, "text");
        for command in ["fish", "fish_indent", "fish_key_reader"] {
            let path = help_dir.join("cmds").join(format!("{command}.txt"));
            assert!(std::fs::metadata(&path).is_ok_and(|m| m.len() > 0),
                "Sphinx did not generate required help: {}", path.display());
        }
    }
}

fn build_docs(output_dir: &Path, builder: &str) {
    use fish_build_helper::{env_var, workspace_root};
    use std::process::{Command, Stdio};

    let workspace_root = workspace_root();
    let doc_src_dir = workspace_root.join("doc_src");
    let doctrees_dir = fish_doc_dir().join(format!(".doctrees-{builder}"));

    fish_build_helper::rebuild_if_paths_changed([
        &workspace_root.join("CHANGELOG.rst"),
        &workspace_root.join("CONTRIBUTING.rst"),
        &doc_src_dir,
    ]);

    let args = as_os_strs![
        "-j",
        "auto",
        "-q",
        "-b",
        builder,
        "-c",
        &doc_src_dir,
        // doctree path - put this *above* the man1 dir to exclude it.
        // this is ~6M
        "-d",
        &doctrees_dir,
        &doc_src_dir,
        &output_dir,
    ];

    rsconf::rebuild_if_env_changed("FISH_BUILD_DOCS");
    assert_ne!(
        env_var("FISH_BUILD_DOCS"), Some("0".to_owned()),
        "embed-manpages requests generated help; FISH_BUILD_DOCS=0 would silently disable it. \
         Select --no-default-features for the supported minimal profile."
    );

    // Requested embedded documentation is a real build input. Never cache a
    // successful feature-enabled build with missing or empty help data.
    let sphinx_build = match Command::new(option_env!("FISH_SPHINX").unwrap_or("sphinx-build"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            panic!(
                "embed-manpages requires sphinx-build to generate packaged help. \
                 Install Sphinx or explicitly select --no-default-features."
            );
        }
        Err(e) => {
            // Another error - permissions wrong etc
            panic!("Error starting sphinx-build to build man pages: {:?}", e);
        }
        Ok(sphinx_build) => sphinx_build,
    };

    match sphinx_build.wait_with_output() {
        Err(err) => {
            panic!(
                "Error waiting for sphinx-build to build man pages: {:?}",
                err
            );
        }
        Ok(out) => {
            if !out.stderr.is_empty() {
                rsconf::warn!("sphinx-build: {}", String::from_utf8_lossy(&out.stderr));
            }
            assert_eq!(&String::from_utf8_lossy(&out.stdout), "");
            assert!(
                out.status.success(),
                "sphinx-build failed to build the man pages."
            );
        }
    }
}
