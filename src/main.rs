use macroquad::prelude::*;

mod sim;

use sim::{window_conf, AgentKind, AnimalSpecies, StatsCategory, World, INVENTORY_SLOTS};

#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new();
    let mut paused = false;
    let mut menu_open = false;
    let mut selected_category: Option<StatsCategory> = None;
    let mut viewing_inventory: Option<usize> = None;
    let mut scroll_offset = 0;

    loop {
        clear_background(BLACK);

        if is_key_pressed(KeyCode::Space) {
            paused = !paused;
            menu_open = false;
            selected_category = None;
            viewing_inventory = None;
            scroll_offset = 0;
        }

        if !paused {
            world.update();
        }

        world.draw();

        // --- UI elements ---
        let v_count = world.agents.iter().filter(|a| a.kind == AgentKind::Valkarai).count();
        let z_count = world.agents.iter().filter(|a| a.kind == AgentKind::Zombie).count();
        draw_text(&format!("Valkarai: {} | Zombies: {} | Animals: {}", v_count, z_count, world.animals.len()), 20.0, 30.0, 30.0, RED);

        if paused {
            // Semi-transparent overlay
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.2));

            if !menu_open {
                // Pause button menu
                let button_width = 300.0;
                let button_height = 80.0;
                let button_x = (screen_width() - button_width) / 2.0;
                let button_y = (screen_height() - button_height) / 2.0;

                draw_rectangle(button_x, button_y, button_width, button_height, Color::new(0.2, 0.2, 0.2, 0.8));
                draw_rectangle_lines(button_x, button_y, button_width, button_height, 3.0, YELLOW);
                draw_text("PAUSED", button_x + 70.0, button_y + 45.0, 35.0, YELLOW);

                // Statistics button
                let stats_btn_x = button_x + 20.0;
                let stats_btn_y = button_y + 100.0;
                let stats_btn_w = 260.0;
                let stats_btn_h = 50.0;

                draw_rectangle(stats_btn_x, stats_btn_y, stats_btn_w, stats_btn_h, Color::new(0.3, 0.6, 0.3, 0.8));
                draw_rectangle_lines(stats_btn_x, stats_btn_y, stats_btn_w, stats_btn_h, 2.0, WHITE);
                draw_text("View Statistics", stats_btn_x + 50.0, stats_btn_y + 35.0, 25.0, WHITE);

                // Check if button clicked
                let mouse_pos = mouse_position();
                if is_mouse_button_pressed(MouseButton::Left) {
                    if mouse_pos.0 >= stats_btn_x && mouse_pos.0 <= stats_btn_x + stats_btn_w &&
                       mouse_pos.1 >= stats_btn_y && mouse_pos.1 <= stats_btn_y + stats_btn_h {
                        menu_open = true;
                        selected_category = None;
                        scroll_offset = 0;
                    }
                }

                draw_text("Press SPACE to resume", button_x - 50.0, button_y + 180.0, 20.0, GRAY);
            } else {
                // Statistics menu
                draw_rectangle(40.0, 40.0, screen_width() - 80.0, screen_height() - 80.0, Color::new(0.0, 0.0, 0.0, 0.95));
                draw_rectangle_lines(40.0, 40.0, screen_width() - 80.0, screen_height() - 80.0, 3.0, YELLOW);

                draw_text("ENTITY REGISTRY (PAUSED)", 60.0, 80.0, 40.0, YELLOW);

                if selected_category.is_none() {
                    draw_text("Choose category", 60.0, 120.0, 28.0, WHITE);
                    let btn_w = 280.0;
                    let btn_h = 62.0;
                    let start_x = (screen_width() - btn_w) / 2.0;
                    let start_y = 190.0;
                    let gap = 90.0;
                    let mouse_pos = mouse_position();

                    let categories = [
                        ("VALKARAI", StatsCategory::Valkarai, Color::new(0.75, 0.2, 0.2, 0.85)),
                        ("ZOMBIES", StatsCategory::Zombie, Color::new(0.35, 0.15, 0.45, 0.85)),
                        ("ANIMALS", StatsCategory::Animal, Color::new(0.2, 0.45, 0.25, 0.85)),
                    ];

                    for (i, (label, cat, col)) in categories.iter().enumerate() {
                        let y = start_y + i as f32 * gap;
                        draw_rectangle(start_x, y, btn_w, btn_h, *col);
                        draw_rectangle_lines(start_x, y, btn_w, btn_h, 2.0, WHITE);
                        draw_text(label, start_x + 72.0, y + 40.0, 34.0, WHITE);
                        if is_mouse_button_pressed(MouseButton::Left) {
                            if mouse_pos.0 >= start_x && mouse_pos.0 <= start_x + btn_w &&
                               mouse_pos.1 >= y && mouse_pos.1 <= y + btn_h {
                                selected_category = Some(*cat);
                                viewing_inventory = None;
                                scroll_offset = 0;
                            }
                        }
                    }
                } else {
                    draw_text("Use UP/DOWN arrows to scroll | LEFT to categories | SPACE to close", 60.0, 110.0, 18.0, GRAY);
                    let start_y = 150.0;
                    let items_per_page = 20;

                    match selected_category.unwrap() {
                        StatsCategory::Valkarai | StatsCategory::Zombie => {
                            draw_text("#      TYPE        SPEED    VISION    ENERGY", 60.0, start_y, 25.0, WHITE);
                            draw_line(60.0, start_y + 5.0, screen_width() - 60.0, start_y + 5.0, 2.0, GRAY);

                            let want_kind = if selected_category == Some(StatsCategory::Zombie) { AgentKind::Zombie } else { AgentKind::Valkarai };
                            let filtered_indices: Vec<usize> = world.agents.iter().enumerate()
                                .filter(|(_, a)| a.kind == want_kind)
                                .map(|(idx, _)| idx)
                                .collect();
                            let start = scroll_offset * items_per_page;
                            let end = (start + items_per_page).min(filtered_indices.len());

                            for (row, global_idx) in filtered_indices[start..end].iter().enumerate() {
                                let agent = &world.agents[*global_idx];
                                let y = start_y + 40.0 + (row as f32 * 30.0);
                                let kind_str = if agent.kind == AgentKind::Zombie { "ZOMBIE" } else { "VALKARAI" };
                                let kind_col = if agent.kind == AgentKind::Zombie { PURPLE } else { RED };

                                draw_text(&format!("{:03}", start + row + 1), 60.0, y, 20.0, GRAY);
                                draw_text(kind_str, 130.0, y, 20.0, kind_col);
                                draw_text(&format!("{:.2}", agent.speed_gen), 270.0, y, 20.0, WHITE);
                                draw_text(&format!("{:.1}", agent.vision_gen), 370.0, y, 20.0, WHITE);
                                draw_text(&format!("{:.0}%", agent.energy.clamp(0.0, 100.0)), 470.0, y, 20.0, GREEN);

                                let inv_btn_w = 120.0;
                                let inv_btn_h = 24.0;
                                let inv_btn_x = screen_width() - inv_btn_w - 60.0;
                                let inv_btn_y = y - 18.0;
                                draw_rectangle(inv_btn_x, inv_btn_y, inv_btn_w, inv_btn_h, Color::new(0.15, 0.15, 0.25, 0.85));
                                draw_rectangle_lines(inv_btn_x, inv_btn_y, inv_btn_w, inv_btn_h, 1.5, WHITE);
                                draw_text("Inventory", inv_btn_x + 18.0, inv_btn_y + 18.0, 18.0, WHITE);

                                let mouse_pos = mouse_position();
                                if is_mouse_button_pressed(MouseButton::Left) {
                                    if mouse_pos.0 >= inv_btn_x && mouse_pos.0 <= inv_btn_x + inv_btn_w &&
                                       mouse_pos.1 >= inv_btn_y && mouse_pos.1 <= inv_btn_y + inv_btn_h {
                                        viewing_inventory = Some(*global_idx);
                                    }
                                }
                            }

                            if is_key_pressed(KeyCode::Down) && end < filtered_indices.len() { scroll_offset += 1; }
                            if is_key_pressed(KeyCode::Up) && scroll_offset > 0 { scroll_offset -= 1; }
                        }
                        StatsCategory::Animal => {
                            draw_text("#      SPECIES     SPEED    VISION    ENERGY", 60.0, start_y, 25.0, WHITE);
                            draw_line(60.0, start_y + 5.0, screen_width() - 60.0, start_y + 5.0, 2.0, GRAY);

                            let start = scroll_offset * items_per_page;
                            let end = (start + items_per_page).min(world.animals.len());
                            for (row, animal) in world.animals[start..end].iter().enumerate() {
                                let y = start_y + 40.0 + (row as f32 * 30.0);
                                let (name, col) = match animal.species {
                                    AnimalSpecies::Horse => ("HORSE", Color::new(0.8, 0.6, 0.2, 1.0)),
                                    AnimalSpecies::Cow => ("COW", Color::new(0.9, 0.9, 0.7, 1.0)),
                                    AnimalSpecies::Pig => ("PIG", Color::new(0.9, 0.4, 0.6, 1.0)),
                                };

                                draw_text(&format!("{:03}", start + row + 1), 60.0, y, 20.0, GRAY);
                                draw_text(name, 130.0, y, 20.0, col);
                                draw_text(&format!("{:.2}", animal.speed), 270.0, y, 20.0, WHITE);
                                draw_text(&format!("{:.1}", animal.vision), 370.0, y, 20.0, WHITE);
                                draw_text(&format!("{:.0}%", animal.energy.clamp(0.0, 100.0)), 470.0, y, 20.0, GREEN);
                            }

                            if is_key_pressed(KeyCode::Down) && end < world.animals.len() { scroll_offset += 1; }
                            if is_key_pressed(KeyCode::Up) && scroll_offset > 0 { scroll_offset -= 1; }
                        }
                    }

                    if is_key_pressed(KeyCode::Left) {
                        selected_category = None;
                        viewing_inventory = None;
                        scroll_offset = 0;
                    }
                }
            }
        }

        // Inventory overlay (when opened for a specific agent)
        if paused {
            if let Some(idx) = viewing_inventory {
                if idx < world.agents.len() {
                    let agent = &world.agents[idx];
                    // subtle overlay
                    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.25));

                    let win_x = 80.0;
                    let win_y = 80.0;
                    let win_w = screen_width() - 160.0;
                    let win_h = screen_height() - 160.0;
                    draw_rectangle(win_x, win_y, win_w, win_h, Color::new(0.05, 0.05, 0.05, 0.85));
                    draw_rectangle_lines(win_x, win_y, win_w, win_h, 3.0, YELLOW);

                    draw_text(&format!("Inventory - #{:03} {}", idx + 1, if agent.kind == AgentKind::Zombie { "(ZOMBIE)" } else { "(VALKARAI)" }), win_x + 20.0, win_y + 40.0, 30.0, WHITE);
                    draw_text(&format!("Slots: {}/{}", agent.inventory.len(), INVENTORY_SLOTS), win_x + 20.0, win_y + 66.0, 18.0, GRAY);

                    // Items listing
                    let list_x = win_x + 30.0;
                    let mut list_y = win_y + 80.0;
                    if agent.inventory.is_empty() {
                        draw_text("(empty)", list_x, list_y, 22.0, GRAY);
                    } else {
                        for item in &agent.inventory {
                            draw_text(&format!("{}  {:.0} food  sat {:.2}  (id={})", item.name, item.quantity, item.saturation, item.id), list_x, list_y, 22.0, WHITE);
                            list_y += 28.0;
                        }
                    }

                    // Close button
                    let close_w = 160.0;
                    let close_h = 40.0;
                    let close_x = win_x + win_w - close_w - 20.0;
                    let close_y = win_y + win_h - close_h - 20.0;
                    draw_rectangle(close_x, close_y, close_w, close_h, Color::new(0.4, 0.15, 0.15, 0.9));
                    draw_rectangle_lines(close_x, close_y, close_w, close_h, 2.0, WHITE);
                    draw_text("Close", close_x + 50.0, close_y + 26.0, 26.0, WHITE);

                    let mpos = mouse_position();
                    if is_mouse_button_pressed(MouseButton::Left) {
                        if mpos.0 >= close_x && mpos.0 <= close_x + close_w && mpos.1 >= close_y && mpos.1 <= close_y + close_h {
                            viewing_inventory = None;
                        }
                    }
                } else {
                    viewing_inventory = None;
                }
            }
        }

        if is_key_down(KeyCode::Escape) { break; }
        next_frame().await
    }
}
