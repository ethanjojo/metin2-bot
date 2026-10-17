//! Explicit local OS notification probe; never attaches to a game.
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let system = args.iter().any(|a| a == "--system");
    let sound = args.iter().any(|a| a == "--sound");
    if !system && !sound {
        eprintln!("Use --system and/or --sound to send one local test.");
        std::process::exit(2);
    }
    let service = metin_desktop::notifications::NotificationService::default();
    service
        .send(metin_desktop::notifications::Notice {
            title: "MetinBot notification test".into(),
            body: "Local notification test. No game input is sent.".into(),
            system,
            sound,
        })
        .expect("notification queue");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while service.status() == "No notifications sent." && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let status = service.status();
    println!("{status}");
    if !status.contains("submitted") {
        std::process::exit(1);
    }
}
