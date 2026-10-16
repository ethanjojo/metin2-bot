use metin_platform::keyboard_packet;
#[test]
fn keyboard_payload_matches_current_windows_transport() {
    let down = keyboard_packet(0x47, 0x22, true, false);
    assert_eq!(down.message, 0x100);
    assert_eq!(down.flags, 1 | (0x22 << 16));
    let up = keyboard_packet(0x47, 0x22, false, false);
    assert_eq!(up.message, 0x101);
    assert_eq!(up.flags, 1 | (0x22 << 16) | (1 << 30) | (1 << 31));
    let alt = keyboard_packet(0x47, 0x22, true, true);
    assert_eq!(alt.message, 0x104);
    assert_ne!(alt.flags & (1 << 29), 0);
}
