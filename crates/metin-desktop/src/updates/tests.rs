use super::*;

#[test]
fn helper_rechecks_the_exact_official_asset_and_never_trusts_a_serialized_hash() {
    let current = Version::new(0, 1, 0);
    let official = serde_json::to_vec(&vec![release("0.2.0", false)]).unwrap();
    let candidate = parse_releases(&official, &current, UpdateChannel::Stable)
        .unwrap()
        .unwrap();
    assert_eq!(
        reconfirm_from_metadata(&official, &candidate, &current),
        Ok(())
    );
    for field in ["version_text", "asset_name", "asset_url", "size", "digest"] {
        let mut forged = serde_json::to_value(&candidate).unwrap();
        forged[field] = match field {
            "size" => 4.into(),
            "digest" => serde_json::to_value([0_u8; 32]).unwrap(),
            _ => "forged".into(),
        };
        let forged: UpdateCandidate = serde_json::from_value(forged).unwrap();
        assert_eq!(
            reconfirm_from_metadata(&official, &forged, &current),
            Err(UpdateError::Integrity),
            "{field}"
        );
    }
    assert_eq!(
        reconfirm_from_metadata(b"[]", &candidate, &current),
        Err(UpdateError::Integrity)
    );
    let newer =
        serde_json::to_vec(&vec![release("0.3.0", false), release("0.2.0", false)]).unwrap();
    assert_eq!(
        reconfirm_from_metadata(&newer, &candidate, &current),
        Ok(())
    );
}

fn release(version: &str, preview: bool) -> serde_json::Value {
    serde_json::json!({
        "tag_name":format!("v{version}"), "draft":false, "prerelease":preview,
        "published_at":"2026-11-11T12:00:00Z",
        "html_url":format!("https://github.com/ethanjojo/metin2-bot/releases/tag/v{version}"),
        "body":"Release notes", "assets":[{
            "name":format!("CompanionSetup-{version}-windows-x64.exe"),
            "browser_download_url":format!("https://github.com/ethanjojo/metin2-bot/releases/download/v{version}/CompanionSetup-{version}-windows-x64.exe"),
            "size":3, "state":"uploaded",
            "digest":"sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        }]
    })
}

#[test]
fn published_new_companion_release_is_selected() {
    let json = serde_json::to_vec(&vec![release("0.2.0", false)]).unwrap();
    assert_eq!(
        select_release(&json, "0.1.0", false).unwrap(),
        Some("0.2.0".into())
    );
}

#[test]
fn stable_never_selects_preview_same_version_or_downgrade() {
    let releases = vec![
        release("0.3.0-rc.1", true),
        release("0.1.0", false),
        release("0.0.9", false),
    ];
    let json = serde_json::to_vec(&releases).unwrap();
    assert_eq!(select_release(&json, "0.1.0", false).unwrap(), None);
    assert_eq!(
        select_release(&json, "0.1.0", true).unwrap(),
        Some("0.3.0-rc.1".into())
    );
}

#[test]
fn forged_or_unverifiable_asset_never_becomes_candidate() {
    let mutations: &[(&str, serde_json::Value)] = &[
        ("digest", serde_json::Value::Null),
        ("digest", "sha256:abc".into()),
        ("digest", format!("sha512:{}", "a".repeat(64)).into()),
        ("size", 0.into()),
        ("size", (512_u64 * 1024 * 1024 + 1).into()),
        ("state", "new".into()),
        ("name", "MetinBotSetup-0.2.0-windows-x64.exe".into()),
        ("browser_download_url", "https://github.com/attacker/metin2-bot/releases/download/v0.2.0/CompanionSetup-0.2.0-windows-x64.exe".into()),
        ("browser_download_url", "https://github.com@evil.test/ethanjojo/metin2-bot/releases/download/v0.2.0/CompanionSetup-0.2.0-windows-x64.exe".into()),
    ];
    for (field, value) in mutations {
        let mut forged = release("0.2.0", false);
        forged["assets"][0][field] = value.clone();
        let json = serde_json::to_vec(&vec![forged]).unwrap();
        assert!(
            select_release(&json, "0.1.0", false).unwrap().is_none(),
            "accepted {field}={value}"
        );
    }
}

#[test]
fn forged_release_identity_never_becomes_candidate() {
    for (field, value) in [
        ("draft", true.into()),
        ("prerelease", true.into()),
        ("published_at", serde_json::Value::Null),
        ("tag_name", "v0.2.0/../../payload".into()),
        (
            "html_url",
            "https://github.com/attacker/metin2-bot/releases/tag/v0.2.0".into(),
        ),
    ] {
        let mut forged = release("0.2.0", false);
        forged[field] = value;
        let json = serde_json::to_vec(&vec![forged]).unwrap();
        assert!(
            select_release(&json, "0.1.0", false).unwrap().is_none(),
            "accepted {field}"
        );
    }
}

#[test]
fn semantic_order_beats_api_order_and_build_metadata_does_not_upgrade() {
    let json = serde_json::to_vec(&vec![
        release("0.2.0", false),
        release("0.10.0", false),
        release("0.3.0", false),
    ])
    .unwrap();
    assert_eq!(
        select_release(&json, "0.1.0", false).unwrap(),
        Some("0.10.0".into())
    );
    let json = serde_json::to_vec(&vec![release("0.1.0+other", false)]).unwrap();
    assert!(
        select_release(&json, "0.1.0+local", false)
            .unwrap()
            .is_none()
    );
}

#[test]
fn redirect_boundary_rejects_other_hosts_credentials_ports_and_insecure_urls() {
    assert!(allowed_asset_redirect(
        "https://release-assets.githubusercontent.com/github-production-release-asset/1/asset?sig=123"
    ));
    for url in [
        "http://release-assets.githubusercontent.com/github-production-release-asset/1/a",
        "https://release-assets.githubusercontent.com.evil.test/github-production-release-asset/1/a",
        "https://release-assets.githubusercontent.com:443/github-production-release-asset/1/a",
        "https://user@release-assets.githubusercontent.com/github-production-release-asset/1/a",
        "https://api.github.com/repos/ethanjojo/metin2-bot",
        "https://release-assets.githubusercontent.com/other/1/a",
        "https://release-assets.githubusercontent.com/github-production-release-asset/1/a#fragment",
    ] {
        assert!(!allowed_asset_redirect(url), "accepted {url}");
    }
}

#[test]
fn download_accepts_exact_bytes_and_keeps_a_verified_guard() {
    let candidate = fixture_candidate();
    let generation = std::sync::atomic::AtomicU64::new(1);
    let guard = download_stream(
        std::io::Cursor::new(b"abc"),
        &candidate,
        &generation,
        1,
        |_| {},
    )
    .unwrap();
    assert!(
        guard
            .installer_path()
            .ends_with("CompanionSetup-0.2.0-windows-x64.exe")
    );
    assert_eq!(std::fs::read(guard.installer_path()).unwrap(), b"abc");
}

#[test]
fn download_rejects_truncation_extra_data_wrong_hash_and_cancellation() {
    let candidate = fixture_candidate();
    let generation = std::sync::atomic::AtomicU64::new(1);
    for bytes in [b"ab".as_slice(), b"abcd", b"xyz"] {
        assert!(
            download_stream(
                std::io::Cursor::new(bytes),
                &candidate,
                &generation,
                1,
                |_| {}
            )
            .is_err()
        );
    }
    generation.store(2, std::sync::atomic::Ordering::Release);
    assert!(matches!(
        download_stream(
            std::io::Cursor::new(b"abc"),
            &candidate,
            &generation,
            1,
            |_| {}
        ),
        Err(UpdateError::Cancelled)
    ));
}

#[cfg(windows)]
#[test]
fn ready_installer_cannot_be_written_deleted_or_replaced_until_guard_drops() {
    let candidate = fixture_candidate();
    let generation = std::sync::atomic::AtomicU64::new(1);
    let guard = download_stream(
        std::io::Cursor::new(b"abc"),
        &candidate,
        &generation,
        1,
        |_| {},
    )
    .unwrap();
    let path = guard.installer_path();
    assert!(std::fs::OpenOptions::new().write(true).open(path).is_err());
    assert!(std::fs::remove_file(path).is_err());
    assert!(std::fs::rename(path, path.with_extension("old")).is_err());
    let directory = path.parent().unwrap();
    let moved = directory.with_extension("moved");
    if std::fs::rename(directory, &moved).is_ok() {
        std::fs::rename(&moved, directory).unwrap();
        panic!(
            "The locked installer path must not be substituted by renaming its private directory"
        );
    }
}

fn fixture_candidate() -> UpdateCandidate {
    candidate_from_release(
        serde_json::from_value(release("0.2.0", false)).unwrap(),
        &semver::Version::parse("0.1.0").unwrap(),
        UpdateChannel::Stable,
    )
    .unwrap()
}

struct LocalSource {
    releases: Vec<u8>,
    installer: Vec<u8>,
}
impl UpdateSource for LocalSource {
    fn releases(&mut self) -> Result<Vec<u8>, UpdateError> {
        Ok(self.releases.clone())
    }
    fn installer(
        &mut self,
        _: &UpdateCandidate,
    ) -> Result<Box<dyn std::io::Read + Send>, UpdateError> {
        Ok(Box::new(std::io::Cursor::new(self.installer.clone())))
    }
}

fn wait_for_result(service: &mut UpdateService) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while matches!(
        service.state(),
        UpdateState::Checking | UpdateState::Downloading { .. }
    ) {
        service.poll();
        assert!(
            std::time::Instant::now() < deadline,
            "worker timed out: {:?}",
            service.state()
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[test]
fn one_worker_checks_downloads_and_transfers_locked_artifact_only_once() {
    let source = LocalSource {
        releases: serde_json::to_vec(&vec![release("0.2.0", false)]).unwrap(),
        installer: b"abc".to_vec(),
    };
    let mut service = UpdateService::from_source("0.1.0", Box::new(source)).unwrap();
    service.check(UpdateChannel::Stable).unwrap();
    wait_for_result(&mut service);
    assert!(matches!(service.state(), UpdateState::Available(c) if c.version() == "0.2.0"));
    assert!(service.take_verified_installer().is_err());
    service.download().unwrap();
    wait_for_result(&mut service);
    assert!(matches!(service.state(), UpdateState::Ready(_)));
    let installer = service.take_verified_installer().unwrap();
    assert_eq!(std::fs::read(installer.installer_path()).unwrap(), b"abc");
    assert!(service.take_verified_installer().is_err());
    let directory = installer.installer_path().parent().unwrap().to_owned();
    drop(installer);
    assert!(!directory.exists(), "verified artifact directory leaked");
}

#[test]
fn offline_or_rate_limited_source_never_produces_a_ready_installer() {
    struct Offline;
    impl UpdateSource for Offline {
        fn releases(&mut self) -> Result<Vec<u8>, UpdateError> {
            Err(UpdateError::HttpStatus(403))
        }
        fn installer(
            &mut self,
            _: &UpdateCandidate,
        ) -> Result<Box<dyn std::io::Read + Send>, UpdateError> {
            panic!("installer must not be requested")
        }
    }
    let mut service = UpdateService::from_source("0.1.0", Box::new(Offline)).unwrap();
    service.check(UpdateChannel::Stable).unwrap();
    wait_for_result(&mut service);
    assert!(matches!(
        service.state(),
        UpdateState::Failed(UpdateError::HttpStatus(403))
    ));
    assert!(service.take_verified_installer().is_err());
}

#[test]
fn cancelled_old_check_cannot_replace_a_newer_channel_result() {
    struct BlockingSource {
        entered: std::sync::mpsc::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
        calls: usize,
    }
    impl UpdateSource for BlockingSource {
        fn releases(&mut self) -> Result<Vec<u8>, UpdateError> {
            self.calls += 1;
            if self.calls == 1 {
                self.entered.send(()).unwrap();
                self.release
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap();
                Ok(serde_json::to_vec(&vec![release("0.2.0", false)]).unwrap())
            } else {
                Ok(b"[]".to_vec())
            }
        }
        fn installer(
            &mut self,
            _: &UpdateCandidate,
        ) -> Result<Box<dyn std::io::Read + Send>, UpdateError> {
            panic!("no download expected")
        }
    }
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let mut service = UpdateService::from_source(
        "0.1.0",
        Box::new(BlockingSource {
            entered: entered_tx,
            release: release_rx,
            calls: 0,
        }),
    )
    .unwrap();
    service.check(UpdateChannel::Stable).unwrap();
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap();
    service.cancel();
    assert!(matches!(service.state(), UpdateState::Cancelled));
    service.check(UpdateChannel::Preview).unwrap();
    release_tx.send(()).unwrap();
    wait_for_result(&mut service);
    assert!(matches!(service.state(), UpdateState::UpToDate));
    assert!(service.take_verified_installer().is_err());
}

#[test]
fn cancelling_a_ready_download_removes_its_private_file_and_install_authority() {
    let source = LocalSource {
        releases: serde_json::to_vec(&vec![release("0.2.0", false)]).unwrap(),
        installer: b"abc".to_vec(),
    };
    let mut service = UpdateService::from_source("0.1.0", Box::new(source)).unwrap();
    service.check(UpdateChannel::Stable).unwrap();
    wait_for_result(&mut service);
    service.download().unwrap();
    wait_for_result(&mut service);
    let directory = service
        .ready
        .as_ref()
        .unwrap()
        .installer_path()
        .parent()
        .unwrap()
        .to_owned();
    service.cancel();
    assert!(matches!(service.state(), UpdateState::Cancelled));
    assert!(service.take_verified_installer().is_err());
    assert!(
        !directory.exists(),
        "cancelled ready installer was retained"
    );
}

#[test]
fn cancelling_an_inflight_download_discards_late_bytes_and_allows_next_check() {
    struct BlockingReader {
        entered: Option<std::sync::mpsc::Sender<()>>,
        release: std::sync::mpsc::Receiver<()>,
        bytes: std::io::Cursor<Vec<u8>>,
    }
    impl std::io::Read for BlockingReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if let Some(entered) = self.entered.take() {
                entered.send(()).unwrap();
                self.release
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap();
            }
            self.bytes.read(buffer)
        }
    }
    struct Source {
        reader: Option<BlockingReader>,
        checks: usize,
    }
    impl UpdateSource for Source {
        fn releases(&mut self) -> Result<Vec<u8>, UpdateError> {
            self.checks += 1;
            if self.checks == 1 {
                Ok(serde_json::to_vec(&vec![release("0.2.0", false)]).unwrap())
            } else {
                Ok(b"[]".to_vec())
            }
        }
        fn installer(
            &mut self,
            _: &UpdateCandidate,
        ) -> Result<Box<dyn std::io::Read + Send>, UpdateError> {
            Ok(Box::new(self.reader.take().unwrap()))
        }
    }
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let source = Source {
        reader: Some(BlockingReader {
            entered: Some(entered_tx),
            release: release_rx,
            bytes: std::io::Cursor::new(b"abc".to_vec()),
        }),
        checks: 0,
    };
    let mut service = UpdateService::from_source("0.1.0", Box::new(source)).unwrap();
    service.check(UpdateChannel::Stable).unwrap();
    wait_for_result(&mut service);
    service.download().unwrap();
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap();
    service.cancel();
    service.check(UpdateChannel::Preview).unwrap();
    release_tx.send(()).unwrap();
    wait_for_result(&mut service);
    assert!(matches!(service.state(), UpdateState::UpToDate));
    assert!(service.take_verified_installer().is_err());
}

#[test]
fn bounded_ui_fixtures_match_real_typed_states_and_ready_guard_is_verified() {
    assert!(matches!(
        UpdateService::fixture(UpdateFixture::Idle).unwrap().state(),
        UpdateState::Idle
    ));
    assert!(
        matches!(UpdateService::fixture(UpdateFixture::Available).unwrap().state(), UpdateState::Available(c) if c.version() == "0.2.0" && c.size() == 32)
    );
    assert!(matches!(
        UpdateService::fixture(UpdateFixture::Downloading)
            .unwrap()
            .state(),
        UpdateState::Downloading {
            downloaded: 16,
            total: 32,
            ..
        }
    ));
    assert!(matches!(
        UpdateService::fixture(UpdateFixture::Failed)
            .unwrap()
            .state(),
        UpdateState::Failed(UpdateError::Network)
    ));
    let mut ready = UpdateService::fixture(UpdateFixture::Ready).unwrap();
    assert!(matches!(ready.state(), UpdateState::Ready(c) if c.size() == 32));
    let artifact = ready.take_verified_installer().unwrap();
    assert_eq!(
        std::fs::read(artifact.installer_path()).unwrap(),
        b"Companion updater smoke fixture\n"
    );
    #[cfg(windows)]
    assert!(
        std::fs::OpenOptions::new()
            .write(true)
            .open(artifact.installer_path())
            .is_err()
    );
}
