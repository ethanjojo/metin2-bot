use clap::Parser;
use metin_desktop::cli::Cli;

#[test]
fn self_test_requires_report_and_rejects_other_overrides() {
    assert!(Cli::try_parse_from(["metinbot", "--self-test"]).is_err());
    assert!(Cli::try_parse_from(["metinbot", "--self-test-report", "report.json"]).is_err());
    for flag in ["--data-dir", "--import-data", "--log"] {
        assert!(
            Cli::try_parse_from([
                "metinbot",
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
            "metinbot",
            "--self-test",
            "--self-test-report",
            "report.json"
        ])
        .is_ok()
    );
}
