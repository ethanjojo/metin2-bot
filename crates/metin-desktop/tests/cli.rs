use clap::Parser;
use metin_desktop::cli::Cli;

#[test]
fn migration_commands_are_explicit_headless_operations_with_required_reports() {
    for operation in ["--migrate-data", "--finish-upgrade"] {
        let mut command = vec!["companion", operation];
        if operation == "--finish-upgrade" {
            command.push("previous");
        }
        assert!(Cli::try_parse_from(&command).is_err());
        command.extend(["--migration-report", "report.json"]);
        if operation == "--finish-upgrade" {
            command.extend(["--previous-inventory", "snapshot.json"]);
        }
        assert!(Cli::try_parse_from(&command).is_ok());
        for extra in [
            vec!["--self-test", "--self-test-report", "self.json"],
            vec![
                "--input-driver",
                "status",
                "--input-driver-report",
                "driver.json",
            ],
            vec!["--log", "run.log"],
        ] {
            assert!(Cli::try_parse_from(command.iter().copied().chain(extra)).is_err());
        }
    }
    assert!(Cli::try_parse_from(["companion", "--migration-report", "report.json"]).is_err());
    assert!(
        Cli::try_parse_from([
            "companion",
            "--migrate-data",
            "--finish-upgrade",
            "previous",
            "--migration-report",
            "report.json"
        ])
        .is_err()
    );
}

#[test]
fn migration_report_cannot_truncate_a_real_settings_file() {
    let root = tempfile::tempdir().unwrap();
    let settings = root.path().join("ui-settings.json");
    std::fs::write(&settings, b"{}").unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_companion"))
        .arg("--migrate-data")
        .arg("--data-dir")
        .arg(root.path())
        .arg("--migration-report")
        .arg(&settings)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(std::fs::read(&settings).unwrap(), b"{}");
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn input_driver_management_requires_report_and_excludes_app_startup() {
    assert!(Cli::try_parse_from(["companion", "--input-driver", "install"]).is_err());
    assert!(Cli::try_parse_from(["companion", "--input-driver-report", "report.json"]).is_err());
    for operation in ["install", "status", "preview", "remove"] {
        assert!(
            Cli::try_parse_from([
                "companion",
                "--input-driver",
                operation,
                "--input-driver-report",
                "report.json"
            ])
            .is_ok()
        );
        for flag in ["--data-dir", "--log", "--import-data"] {
            assert!(
                Cli::try_parse_from([
                    "companion",
                    "--input-driver",
                    operation,
                    "--input-driver-report",
                    "report.json",
                    flag,
                    "path"
                ])
                .is_err()
            );
        }
    }
    assert!(
        Cli::try_parse_from([
            "companion",
            "--input-driver",
            "install",
            "--input-driver-report",
            "report.json",
            "--self-test",
            "--self-test-report",
            "test.json"
        ])
        .is_err()
    );
}

#[test]
fn self_test_requires_report_and_rejects_other_overrides() {
    assert!(Cli::try_parse_from(["companion", "--self-test"]).is_err());
    assert!(Cli::try_parse_from(["companion", "--self-test-report", "report.json"]).is_err());
    for flag in ["--data-dir", "--import-data", "--log"] {
        assert!(
            Cli::try_parse_from([
                "companion",
                "--self-test",
                "--self-test-report",
                "report.json",
                flag,
                "path"
            ])
            .is_err()
        );
    }
    assert!(
        Cli::try_parse_from([
            "companion",
            "--self-test",
            "--self-test-report",
            "report.json"
        ])
        .is_ok()
    );
}

#[test]
fn cli_banner_uses_product_name_and_companion_executable_usage() {
    use clap::CommandFactory;
    let mut command = Cli::command();
    assert!(command.render_long_version().starts_with("Companion "));
    assert!(
        command
            .render_long_help()
            .to_string()
            .contains("Usage: companion")
    );
}

#[test]
fn upgrade_inventory_is_required_and_exclusive_to_upgrade() {
    assert!(
        Cli::try_parse_from([
            "companion",
            "--finish-upgrade",
            "previous",
            "--migration-report",
            "report.json"
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "companion",
            "--finish-upgrade",
            "previous",
            "--previous-inventory",
            "snapshot.json",
            "--migration-report",
            "report.json"
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "companion",
            "--migrate-data",
            "--previous-inventory",
            "snapshot.json",
            "--migration-report",
            "report.json"
        ])
        .is_err()
    );
}
