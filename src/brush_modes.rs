use std::collections::VecDeque;

use crate::{
    paint::{BrushMode, ControlStep, Paint},
    util::Key,
};

pub fn normal_brush_mode(paint: &mut Paint) {
    let (mx, my) = paint.window.mouse;

    if paint.window.pressed_left {
        draw_line(
            paint,
            paint.last_mouse_pos.0 as isize,
            paint.last_mouse_pos.1 as isize,
            mx as isize,
            my as isize,
        );
        paint.main_buffer = paint.screen_buffer.clone();
    }

    paint.last_mouse_pos.0 = mx;
    paint.last_mouse_pos.1 = my;
}

pub fn fill_mode(paint: &mut Paint) {
    if paint.window.clicked_left {
        let (mx, my) = paint.window.mouse;
        flood_fill(paint, mx as isize, my as isize);
        paint.main_buffer = paint.screen_buffer.clone();
    }
}

pub fn multi_mode(paint: &mut Paint) {
    let (mut mx, mut my) = (paint.window.mouse.0 as isize, paint.window.mouse.1 as isize);

    if paint.control_step == ControlStep::ClickSecond {
        paint.screen_buffer = paint.main_buffer.clone();

        if paint.window.keys_pressed.contains(&Key::LShift) {
            let dx = mx - paint.last_mouse_pos.0 as isize;
            let dy = my - paint.last_mouse_pos.1 as isize;

            let abs_dx = dx.abs();
            let abs_dy = dy.abs();

            let stand_len = isize::min(abs_dx, abs_dy);

            if abs_dx > abs_dy * 2 {
                my = paint.last_mouse_pos.1 as isize;
            } else if abs_dy > abs_dx * 2 {
                mx = paint.last_mouse_pos.0 as isize;
            } else {
                let sign_x = if dx >= 0 { 1 } else { -1 };
                let sign_y = if dy >= 0 { 1 } else { -1 };

                mx = paint.last_mouse_pos.0 as isize + stand_len * sign_x;
                my = paint.last_mouse_pos.1 as isize + stand_len * sign_y;
            }
        }

        match paint.brush_mode {
            BrushMode::Line => draw_line(
                paint,
                paint.last_mouse_pos.0 as isize,
                paint.last_mouse_pos.1 as isize,
                mx,
                my,
            ),

            BrushMode::Square => draw_square(
                paint,
                paint.last_mouse_pos.0 as isize,
                paint.last_mouse_pos.1 as isize,
                mx,
                my,
            ),

            BrushMode::Triangle => draw_triangle(
                paint,
                paint.last_mouse_pos.0 as isize,
                paint.last_mouse_pos.1 as isize,
                mx,
                my,
            ),

            BrushMode::Circle => {
                let radius = (((mx - paint.last_mouse_pos.0 as isize) as f32).powi(2)
                    + ((my - paint.last_mouse_pos.1 as isize) as f32).powi(2))
                .sqrt();

                draw_circle(
                    paint,
                    paint.last_mouse_pos.0 as isize,
                    paint.last_mouse_pos.1 as isize,
                    radius as isize,
                );
            }

            _ => unreachable!(),
        }
    }

    if paint.window.clicked_left {
        if paint.control_step == ControlStep::ClickFirst {
            println!("draw {:?} from: {} {}", paint.brush_mode, mx, my);

            paint.last_mouse_pos.0 = mx as u32;
            paint.last_mouse_pos.1 = my as u32;
            paint.control_step = ControlStep::ClickSecond;
        } else {
            println!(
                "drawn {:?} from {} {} to: {} {}",
                paint.brush_mode, paint.last_mouse_pos.0, paint.last_mouse_pos.1, mx, my
            );

            paint.main_buffer = paint.screen_buffer.clone();
            paint.control_step = ControlStep::ClickFirst;
        }
    }
}

fn draw_line(paint: &mut Paint, x0: isize, y0: isize, x1: isize, y1: isize) {
    let mut plot_point = |cx: isize, cy: isize| {
        let half = paint.brush_size as isize / 2;
        let start = -half;
        let end = start + paint.brush_size as isize;

        for dy in start..end {
            for dx in start..end {
                let x = cx + dx;
                let y = cy + dy;
                if x >= 0
                    && y >= 0
                    && (x as u32) < paint.buffer_size.0
                    && (y as u32) < paint.buffer_size.1
                {
                    let idx = y as usize * paint.buffer_size.0 as usize + x as usize;
                    paint.screen_buffer[idx] = paint.color;
                }
            }
        }
    };

    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();

    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };

    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        plot_point(x, y);

        if x == x1 && y == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

fn draw_square(paint: &mut Paint, x0: isize, y0: isize, x1: isize, y1: isize) {
    let (min_x, max_x) = (x0.min(x1), x0.max(x1));
    let (min_y, max_y) = (y0.min(y1), y0.max(y1));

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let is_border_x =
                (x - min_x) < paint.brush_size as isize || (max_x - x) < paint.brush_size as isize;
            let is_border_y =
                (y - min_y) < paint.brush_size as isize || (max_y - y) < paint.brush_size as isize;

            if is_border_x || is_border_y {
                let idx = (y * paint.buffer_size.0 as isize + x) as usize;
                if idx < paint.screen_buffer.len() {
                    paint.screen_buffer[idx as usize] = paint.color;
                }
            }
        }
    }
}

fn draw_triangle(paint: &mut Paint, x0: isize, y0: isize, x1: isize, y1: isize) {
    draw_line(paint, x0, y0, x1, y0);
    draw_line(paint, x1, y0, x1, y1);
    draw_line(paint, x1, y1, x0, y0);
}

fn draw_circle(paint: &mut Paint, x0: isize, y0: isize, radius: isize) {
    let r_min = (radius - paint.brush_size as isize).max(0);
    let r_max = radius;

    for y in (y0 - r_max)..=(y0 + r_max) {
        for x in (x0 - r_max)..=(x0 + r_max) {
            let dx = x - x0;
            let dy = y - y0;
            let dist_sq = dx * dx + dy * dy;

            if x >= 0
                && y >= 0
                && (x as usize) < paint.buffer_size.0 as usize
                && (y as usize) < paint.buffer_size.1 as usize
            {
                if dist_sq >= r_min * r_min && dist_sq <= r_max * r_max {
                    let idx = y as usize * paint.buffer_size.0 as usize + x as usize;
                    paint.screen_buffer[idx] = paint.color;
                }
            }
        }
    }
}

fn flood_fill(paint: &mut Paint, x: isize, y: isize) {
    if x >= paint.buffer_size.0 as isize || y >= paint.buffer_size.1 as isize {
        return;
    }

    let replace_color = paint.screen_buffer[y as usize * paint.buffer_size.0 as usize + x as usize];
    if replace_color == paint.color {
        return;
    }

    let mut queue = VecDeque::new();
    queue.push_back((x, y));

    while let Some((cx, cy)) = queue.pop_front() {
        let idx = (cy * paint.buffer_size.0 as isize + cx) as usize;

        if paint.screen_buffer[idx] != replace_color {
            continue;
        }

        paint.screen_buffer[idx] = paint.color;

        if cx > 0 {
            queue.push_back((cx - 1, cy));
        }

        if cx < paint.buffer_size.0 as isize - 1 {
            queue.push_back((cx + 1, cy));
        }

        if cy > 0 {
            queue.push_back((cx, cy - 1));
        }

        if cy < paint.buffer_size.1 as isize - 1 {
            queue.push_back((cx, cy + 1));
        }
    }
}
