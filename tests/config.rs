//! The reference config, which is the eleven real targets `docs/config.md` was measured on.

use std::path::{Path, PathBuf};

use vanadis::{Config, Shell, TargetName, Variant};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn reference() -> Config {
    Config::load(&root().join("docs/examples"), Some(Path::new("/home/ada"))).unwrap()
}

#[test]
fn loads_every_target_the_reference_config_writes() {
    assert_eq!(reference().targets().len(), 11);
}

#[test]
fn reads_the_per_target_override() {
    let config = reference();
    let nvim = config
        .targets()
        .iter()
        .find(|target| target.name() == &TargetName::parse("nvim").unwrap())
        .unwrap();
    assert_eq!(
        nvim.themes()
            .unwrap()
            .theme(Variant::Dark)
            .unwrap()
            .as_str(),
        "gruvbox-dark"
    );
}

#[test]
fn reads_the_reload_command_bat_needs() {
    let config = reference();
    let bat = &config.targets()[0];
    assert_eq!(bat.reload(), ["bat", "cache", "--build"]);
}

#[test]
fn resolves_a_template_path_against_the_config_directory() {
    let config = reference();
    assert_eq!(
        config.targets()[0].template(),
        root().join("docs/examples/templates/bat/PaperColor-Light.tmTheme.in")
    );
}

#[test]
fn expands_an_output_path_against_the_home_directory() {
    let config = reference();
    assert_eq!(
        config.targets()[0].output(),
        Path::new("/home/ada/.config/bat/themes/vanadis.tmTheme")
    );
}

#[test]
fn marks_the_target_a_shell_sources() {
    // The zsh palette is the reference config's one target read from the environment, which
    // is what `vanadis hook zsh` exists for. See `docs/hook.md`.
    let config = reference();
    let zsh = config
        .targets()
        .iter()
        .find(|target| target.name() == &TargetName::parse("zsh").unwrap())
        .unwrap();
    assert_eq!(zsh.shell(), Some(Shell::Zsh));
}
