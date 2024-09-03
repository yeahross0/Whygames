const MAX_FRAME_TIME: f64 = 0.05;
const EXPECTED_FPS: f64 = 60.0;
const EXPECTED_DELTA: f64 = 1.0 / EXPECTED_FPS;

#[derive(Copy, Clone, Debug)]
pub struct TimeKeeping {
    pub total_elapsed: f64,
    last_timestamp: f64,
    // TODO: Handling this ok?
    //playback_rate: f64,
}

impl TimeKeeping {
    pub fn new(time: f64) -> TimeKeeping {
        TimeKeeping {
            total_elapsed: 0.0,
            last_timestamp: time,
            //playback_rate: 1.0,
        }
    }

    pub fn update(&mut self, time: f64, playback_rate: f64) {
        let elapsed = time - self.last_timestamp;
        // TODO: Multiplication by rate happen on line 1 or 2?
        let capped_elapsed = elapsed.min(MAX_FRAME_TIME) * playback_rate;
        self.total_elapsed += capped_elapsed;
        self.last_timestamp = time;
    }

    pub fn has_more_frames_to_play(self, frame_number: usize) -> bool {
        self.total_elapsed > frame_number as f64 * EXPECTED_DELTA
    }

    // TODO: Should reset last timestamp too?
    pub fn reset(&mut self) {
        self.total_elapsed = 0.0;
    }
}
