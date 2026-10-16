use metin_platform::{Frame, StopToken};
#[test]
fn validates_bgr_buffer_and_cancellation_generations() {
    assert!(Frame::new(2, 2, vec![0; 11]).is_err());
    assert!(Frame::new(0, 2, vec![]).is_err());
    let f = Frame::new(2, 2, vec![1; 12]).unwrap();
    assert_eq!(f.bgr.len(), 12);
    let stop = StopToken::default();
    let generation = stop.generation();
    assert!(!stop.cancelled(generation));
    stop.stop();
    assert!(stop.cancelled(generation));
    assert!(!stop.cancelled(stop.generation()));
}
