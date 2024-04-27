fn next_index(index: usize, sprite_count: usize) -> usize {
    (index + 1) % sprite_count
}

pub fn has_finished_animation(index: usize, should_loop: bool) -> bool {
    index == 0 && !should_loop
}

fn update_counter(frames_until_next_change: &mut usize) {
    *frames_until_next_change -= 1;
}

fn needs_to_change(frames_until_next_change: usize) -> bool {
    frames_until_next_change == 0
}

fn reset_counter(frames_until_next_change: &mut usize, anim_time: usize) {
    *frames_until_next_change = anim_time;
}

pub fn update_frame(
    index: &mut usize,
    frames_until_next_change: &mut usize,
    anim_time: usize,
    sprite_count: usize,
) -> bool {
    update_counter(frames_until_next_change);
    if needs_to_change(*frames_until_next_change) {
        *index = next_index(*index, sprite_count);
        reset_counter(frames_until_next_change, anim_time);
        assert_ne!(*frames_until_next_change, 0);
        true
    } else {
        false
    }
}
