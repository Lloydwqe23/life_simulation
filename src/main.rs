use macroquad::prelude::*;
use ::rand::Rng;
use noise::{NoiseFn, Perlin};

// --- КОНСТАНТИ ---
const GRID_SIZE: usize = 250;
const MATING_DISTANCE: f32 = 1.2;
const COOLDOWN_TIME: f32 = 150.0;
const REPRODUCTION_THRESHOLD: f32 = 90.0;
const SPEED_TUNDRA: f32 = 1.0;
const SPEED_FOREST: f32 = 0.6;
const SPEED_MOUNTAIN: f32 = 0.2;

fn window_conf() -> Conf {
    Conf {
        window_title: "Quadrisrah: Entity Registry".to_owned(),
        fullscreen: true,
        ..Default::default()
    }
}

// --- СТРУКТУРИ ---
#[derive(Clone, Copy, PartialEq)]
enum Terrain { Mountain, Tundra, Forest }
#[derive(Clone, Copy, PartialEq)]
enum AgentKind { Valkarai, Zombie }

struct Cell { terrain: Terrain, food_level: f32 }

struct Agent {
    pos: Vec2,
    energy: f32,
    reproduce_cooldown: f32,
    speed_gen: f32,
    vision_gen: f32,
    kind: AgentKind,
    health: f32,
    damage: f32,
}

struct World {
    cells: Vec<Vec<Cell>>,
    agents: Vec<Agent>,
}

impl World {
    fn new() -> Self {
        let mut rng = ::rand::thread_rng();
        let seed = rng.gen::<u32>();
        let perlin = Perlin::new(seed);
        let mut cells = Vec::new();

        for x in 0..GRID_SIZE {
            let mut row = Vec::new();
            for y in 0..GRID_SIZE {
                let val = perlin.get([x as f64 * 0.05, y as f64 * 0.05]);
                let terrain = if val > 0.4 { Terrain::Mountain } 
                               else if val > 0.0 { Terrain::Forest } 
                               else { Terrain::Tundra };
                row.push(Cell { terrain, food_level: 0.0 });
            }
            cells.push(row);
        }

        let mut agents = Vec::new();
        for _ in 0..40 {
            agents.push(Agent {
                pos: vec2(rng.gen_range(0.0..GRID_SIZE as f32), rng.gen_range(0.0..GRID_SIZE as f32)),
                energy: 100.0, reproduce_cooldown: 0.0,
                speed_gen: rng.gen_range(0.12..0.22), vision_gen: rng.gen_range(10.0..20.0),
                kind: AgentKind::Valkarai, health: 100.0, damage: 10.0,
            });
        }
        agents.push(Agent {
            pos: vec2(50.0, 50.0), energy: 10000.0, reproduce_cooldown: 0.0,
            speed_gen: 0.15, vision_gen: 15.0, kind: AgentKind::Zombie, health: 300.0, damage: 20.0,
        });
        World { cells, agents }
    }

    fn update(&mut self) {
        let mut rng = ::rand::thread_rng();
        if rng.gen_bool(0.8) {
            let x = rng.gen_range(0..GRID_SIZE);
            let y = rng.gen_range(0..GRID_SIZE);
            let chance = match self.cells[x][y].terrain {
                Terrain::Tundra => 0.53, Terrain::Forest => 0.32, Terrain::Mountain => 0.15,
            };
            if rng.gen_bool(chance) { self.cells[x][y].food_level += 80.0; }
        }

        let mut infections = Vec::new();
        let agent_count = self.agents.len();

        for i in 0..agent_count {
            if self.agents[i].reproduce_cooldown > 0.0 { self.agents[i].reproduce_cooldown -= 1.0; }
            let pos = self.agents[i].pos;
            let kind = self.agents[i].kind;
            let vision = self.agents[i].vision_gen;
            
            let mut target: Option<Vec2> = None;
            let mut flee_dir: Option<Vec2> = None;

            if kind == AgentKind::Zombie {
                let mut min_d = vision;
                for j in 0..agent_count {
                    if self.agents[j].kind == AgentKind::Valkarai {
                        let d = pos.distance(self.agents[j].pos);
                        if d < min_d { min_d = d; target = Some(self.agents[j].pos); }
                        if d < MATING_DISTANCE { infections.push(j); }
                    }
                }
            } else {
                for j in 0..agent_count {
                    if self.agents[j].kind == AgentKind::Zombie {
                        let d = pos.distance(self.agents[j].pos);
                        if d < vision * 0.8 { flee_dir = Some(pos - self.agents[j].pos); }
                    }
                }
                if flee_dir.is_none() {
                    if self.agents[i].energy > REPRODUCTION_THRESHOLD && self.agents[i].reproduce_cooldown == 0.0 {
                        let mut min_m = vision * 1.5;
                        for j in 0..agent_count {
                            if i == j || self.agents[j].kind == AgentKind::Zombie { continue; }
                            let d = pos.distance(self.agents[j].pos);
                            if d < min_m && self.agents[j].energy > REPRODUCTION_THRESHOLD && self.agents[j].reproduce_cooldown == 0.0 {
                                min_m = d; target = Some(self.agents[j].pos);
                            }
                        }
                    }
                    if target.is_none() {
                        let mut min_f = vision;
                        let v_int = vision as i32;
                        for ox in -v_int..=v_int {
                            for oy in -v_int..=v_int {
                                let cx = (pos.x as i32 + ox).clamp(0, GRID_SIZE as i32 - 1) as usize;
                                let cy = (pos.y as i32 + oy).clamp(0, GRID_SIZE as i32 - 1) as usize;
                                if self.cells[cx][cy].food_level > 0.0 {
                                    let d = pos.distance(vec2(cx as f32 + 0.5, cy as f32 + 0.5));
                                    if d < min_f { min_f = d; target = Some(vec2(cx as f32 + 0.5, cy as f32 + 0.5)); }
                                }
                            }
                        }
                    }
                }
            }

            let speed_mult = match self.cells[pos.x as usize][pos.y as usize].terrain {
                Terrain::Tundra => SPEED_TUNDRA, Terrain::Forest => SPEED_FOREST, Terrain::Mountain => SPEED_MOUNTAIN,
            };
            let cur_speed = self.agents[i].speed_gen * speed_mult;

            if let Some(dir) = flee_dir { self.agents[i].pos += dir.normalize() * (cur_speed * 1.3); }
            else if let Some(t) = target { let d = t - pos; if d.length() > 0.1 { self.agents[i].pos += d.normalize() * cur_speed; } }
            else { self.agents[i].pos += vec2(rng.gen_range(-0.1..0.1), rng.gen_range(-0.1..0.1)) * speed_mult; }

            self.agents[i].pos.x = self.agents[i].pos.x.clamp(0.0, (GRID_SIZE - 1) as f32);
            self.agents[i].pos.y = self.agents[i].pos.y.clamp(0.0, (GRID_SIZE - 1) as f32);

            if kind == AgentKind::Valkarai {
                self.agents[i].energy -= 0.1 + (self.agents[i].vision_gen * 0.006) + (self.agents[i].speed_gen * 0.45);
                let (nx, ny) = (self.agents[i].pos.x as usize, self.agents[i].pos.y as usize);
                if self.cells[nx][ny].food_level > 0.0 && self.agents[i].energy < 100.0 {
                    let eat = 20.0f32.min(self.cells[nx][ny].food_level);
                    self.cells[nx][ny].food_level -= eat;
                    self.agents[i].energy += eat * 1.5;
                }
            }
        }
        for idx in infections { self.agents[idx].kind = AgentKind::Zombie; self.agents[idx].energy = 10000.0; }
        
        let mut newborns = Vec::new();
        let mut mated = vec![false; self.agents.len()];
        for i in 0..self.agents.len() {
            if self.agents[i].kind == AgentKind::Zombie || mated[i] || self.agents[i].energy < REPRODUCTION_THRESHOLD { continue; }
            for j in i+1..self.agents.len() {
                if self.agents[j].kind == AgentKind::Valkarai && !mated[j] && self.agents[j].energy > REPRODUCTION_THRESHOLD {
                    if self.agents[i].pos.distance(self.agents[j].pos) < MATING_DISTANCE {
                        mated[i] = true; mated[j] = true;
                        self.agents[i].energy -= 50.0; self.agents[j].energy -= 50.0;
                        let mut cs = (self.agents[i].speed_gen + self.agents[j].speed_gen) / 2.0;
                        let mut cv = (self.agents[i].vision_gen + self.agents[j].vision_gen) / 2.0;
                        if rng.gen_bool(0.1) { cs *= rng.gen_range(0.9..1.1); cv *= rng.gen_range(0.9..1.1); }
                        newborns.push(Agent {
                            pos: self.agents[i].pos, energy: 60.0, reproduce_cooldown: COOLDOWN_TIME,
                            speed_gen: cs.clamp(0.08, 0.3), vision_gen: cv.clamp(8.0, 30.0), kind: AgentKind::Valkarai, health: 100.0,
                            damage: 10.0,
                        });
                        break;
                    }
                }
            }
        }
        self.agents.append(&mut newborns);
        self.agents.retain(|a| a.energy > 0.0);
    }

    fn draw(&self) {
        let (cw, ch) = (screen_width() / GRID_SIZE as f32, screen_height() / GRID_SIZE as f32);
        for x in 0..GRID_SIZE {
            for y in 0..GRID_SIZE {
                let cell = &self.cells[x][y];
                let color = match cell.terrain {
                    Terrain::Mountain => Color::new(0.3, 0.3, 0.35, 1.0),
                    Terrain::Forest => Color::new(0.0, 0.3, 0.1, 1.0),
                    Terrain::Tundra => Color::new(0.9, 0.9, 1.0, 1.0),
                };
                draw_rectangle(x as f32 * cw, y as f32 * ch, cw, ch, color);
                if cell.food_level > 0.0 { draw_rectangle(x as f32 * cw, y as f32 * ch, cw, ch, Color::new(0.6, 0.1, 0.8, 1.0)); }
            }
        }
        for agent in &self.agents {
            let color = if agent.kind == AgentKind::Zombie { BLACK } 
                        else if agent.energy > REPRODUCTION_THRESHOLD && agent.reproduce_cooldown == 0.0 { ORANGE } 
                        else { RED };
            draw_circle(agent.pos.x * cw, agent.pos.y * ch, (agent.vision_gen / 15.0) * cw * 0.7, color);
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new();
    let mut paused = false;
    let mut scroll_offset = 0;

    loop {
        clear_background(BLACK);

        if is_key_pressed(KeyCode::Space) {
            paused = !paused;
            scroll_offset = 0; // Скидаємо скрол при вході/виході
        }

        if !paused {
            world.update();
        } else {
            // Керування скролом на паузі (стрілками)
            if is_key_pressed(KeyCode::Down) { scroll_offset += 1; }
            if is_key_pressed(KeyCode::Up) && scroll_offset > 0 { scroll_offset -= 1; }
        }
        
        world.draw();

        // --- UI ЕЛЕМЕНТИ ---
        let v_count = world.agents.iter().filter(|a| a.kind == AgentKind::Valkarai).count();
        let z_count = world.agents.iter().filter(|a| a.kind == AgentKind::Zombie).count();
        draw_text(&format!("Valkarai: {} | Zombies: {}", v_count, z_count), 20.0, 30.0, 30.0, DARKGREEN);
        
        if paused {
            // Напівпрозоре меню
            draw_rectangle(50.0, 50.0, screen_width() - 100.0, screen_height() - 100.0, Color::new(0.0, 0.0, 0.0, 0.85));
            draw_text("ENTITY REGISTRY (PAUSED)", 70.0, 90.0, 40.0, YELLOW);
            draw_text("Use UP/DOWN arrows to scroll", 70.0, 120.0, 20.0, GRAY);
            
            // Заголовки таблиці
            let start_y = 160.0;
            draw_text("#      TYPE        SPEED    VISION    ENERGY", 70.0, start_y, 25.0, WHITE);
            draw_line(70.0, start_y + 5.0, screen_width() - 70.0, start_y + 5.0, 2.0, GRAY);

            // Список істот
            let items_per_page = 20;
            let agents_to_show = world.agents.iter().skip(scroll_offset * items_per_page).take(items_per_page);

            for (i, agent) in agents_to_show.enumerate() {
                let y = start_y + 40.0 + (i as f32 * 30.0);
                let kind_str = if agent.kind == AgentKind::Zombie { "ZOMBIE" } else { "VALKARAI" };
                let kind_col = if agent.kind == AgentKind::Zombie { PURPLE } else { RED };

                draw_text(&format!("{:03}", (scroll_offset * items_per_page) + i + 1), 70.0, y, 20.0, GRAY);
                draw_text(kind_str, 140.0, y, 20.0, kind_col);
                draw_text(&format!("{:.2}", agent.speed_gen), 280.0, y, 20.0, WHITE);
                draw_text(&format!("{:.1}", agent.vision_gen), 380.0, y, 20.0, WHITE);
                draw_text(&format!("{:.0}%", agent.energy.clamp(0.0, 100.0)), 480.0, y, 20.0, GREEN);
            }
        }

        if is_key_down(KeyCode::Escape) { break; }
        next_frame().await
    }
}