use metin_core::Candidate;
#[derive(Debug, Clone)]
pub struct CandidateTracker {
    required: u32,
    max_gap: f64,
    last: Option<f64>,
    tracks: Vec<(Candidate, u32)>,
}
impl CandidateTracker {
    pub fn new(required: u32, interval: f64) -> Self {
        Self {
            required,
            max_gap: 1.0_f64.max(interval * 1.5),
            last: None,
            tracks: vec![],
        }
    }
    pub fn reset(&mut self) {
        self.last = None;
        self.tracks.clear();
    }
    pub fn update(&mut self, candidates: Vec<Candidate>, now: f64) -> Vec<Candidate> {
        let mut previous = if self
            .last
            .is_some_and(|last| now > last && now - last <= self.max_gap)
        {
            std::mem::take(&mut self.tracks)
        } else {
            vec![]
        };
        self.last = Some(now);
        self.tracks = candidates
            .into_iter()
            .map(|c| {
                let matching = previous
                    .iter()
                    .enumerate()
                    .filter(|(_, (old, _))| {
                        f64::from(old.point[0].abs_diff(c.point[0]))
                            <= 10.0_f64.max(f64::from(old.width) * 0.5)
                            && f64::from(old.point[1].abs_diff(c.point[1]))
                                <= 10.0_f64.max(f64::from(old.height) * 0.5)
                    })
                    .min_by_key(|(_, (old, _))| {
                        old.point[0].abs_diff(c.point[0]) + old.point[1].abs_diff(c.point[1])
                    })
                    .map(|(i, _)| i);
                let hits = matching.map_or(1, |i| (previous.remove(i).1 + 1).min(self.required));
                (c, hits)
            })
            .collect();
        self.tracks
            .iter()
            .filter(|(_, hits)| *hits >= self.required)
            .map(|(c, _)| c.clone())
            .collect()
    }
}
