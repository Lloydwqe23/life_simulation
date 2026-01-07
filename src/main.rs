use macroquad::prelude::*;
use ::rand::Rng;
use noise::{NoiseFn, Perlin};

const GRID_SIZE: usize = 100;
const VISION_RADIUS: i32 = 12;
const MOVE_SPEED: f32 = 0.15;
const REPRODUCTION_THRESHOLD: f32 = 90.0;
const MATING_DISTANCE: f32 = 1.2;
const COOLDOWN_TIME: f32 = 120.0; // Приблизно 2 секунди при 60 FPS
const SPEED_TUNDRA: f32 = 1.0;    // 100% швидкості
const SPEED_FOREST: f32 = 0.6;    // 60% швидкості
const SPEED_MOUNTAIN: f32 = 0.2;

fn window_conf() -> Conf {
    Conf {
        window_title: "Quadrisrah: Proper Reproduction".to_owned(),
        fullscreen: true,
        ..Default::default()
    }
}

#[derive(Clone, Copy)]
enum Terrain { Mountain, Tundra, Forest }

struct Cell {
    terrain: Terrain,
    food_level: f32,
}

struct Agent {
    pos: Vec2,
    energy: f32,
    reproduce_cooldown: f32,
}

struct World {
    cells: Vec<Vec<Cell>>,
    agents: Vec<Agent>,
}

impl World {
    fn new() -> Self {
        let mut rng = ::rand::thread_rng();
        
        // Генеруємо випадковий сід
        let seed = rng.gen::<u32>(); 
        println!("Генерація світу з сідом: {}", seed);
        
        let perlin = Perlin::new(seed);
        let mut cells = Vec::new();

        // Параметр частоти (чим менше число, тим більші об'єкти на карті)
        let frequency = 0.05;

        for x in 0..GRID_SIZE {
            let mut row = Vec::new();
            for y in 0..GRID_SIZE {
                // Використовуємо наш випадковий perlin
                let val = perlin.get([x as f64 * frequency, y as f64 * frequency]);
                
                let terrain = if val > 0.4 {
                    Terrain::Mountain
                } else if val > 0.0 {
                    Terrain::Forest
                } else {
                    Terrain::Tundra
                };

                row.push(Cell {
                    terrain,
                    food_level: 0.0,
                });
            }
            cells.push(row);
        }

        let mut agents = Vec::new();
        // Початкові Валкарай
        for _ in 0..40 {
            agents.push(Agent {
                pos: vec2(rng.gen_range(0.0..GRID_SIZE as f32), rng.gen_range(0.0..GRID_SIZE as f32)),
                energy: 100.0,
                reproduce_cooldown: 0.0,
            });
        }

        World { cells, agents }
    }

    fn update(&mut self) {
        let mut rng = ::rand::thread_rng();
        
        // 1. Поява їжі
        if rng.gen_bool(0.4) {
            let x = rng.gen_range(0..GRID_SIZE);
            let y = rng.gen_range(0..GRID_SIZE);
            if !matches!(self.cells[x][y].terrain, Terrain::Mountain) {
                self.cells[x][y].food_level += 80.0;
            }
        }

        // 2. Логіка Агентів (Рух та Вибір цілі)
        let agent_count = self.agents.len();
        for i in 0..agent_count {
            // Тимчасово витягуємо дані агента, щоб не порушувати правила Rust про запозичення
            let (ready_to_mate, pos, energy) = {
                let a = &self.agents[i];
                (a.energy > REPRODUCTION_THRESHOLD && a.reproduce_cooldown == 0.0, a.pos, a.energy)
            };

            if self.agents[i].reproduce_cooldown > 0.0 {
                self.agents[i].reproduce_cooldown -= 1.0;
            }

            let mut target: Option<Vec2> = None;

            // --- ЛОГІКА ВИБОРУ ЦІЛІ ---
            if ready_to_mate {
                // ШУКАЄМО ПАРТНЕРА (Активний пошук)
                let mut min_dist = VISION_RADIUS as f32 * 1.5; // Партнера бачать трохи далі
                for j in 0..agent_count {
                    if i == j { continue; }
                    let other = &self.agents[j];
                    
                    // Шукаємо іншого агента, який ТАКОЖ готовий
                    if other.energy > REPRODUCTION_THRESHOLD && other.reproduce_cooldown == 0.0 {
                        let dist = pos.distance(other.pos);
                        if dist < min_dist {
                            min_dist = dist;
                            target = Some(other.pos);
                        }
                    }
                }
            }

            // Якщо партнера не знайшли або ми голодні — шукаємо їжу
            if target.is_none() {
                let mut min_dist = VISION_RADIUS as f32;
                for ox in -VISION_RADIUS..=VISION_RADIUS {
                    for oy in -VISION_RADIUS..=VISION_RADIUS {
                        let cx = (pos.x as i32 + ox).clamp(0, GRID_SIZE as i32 - 1) as usize;
                        let cy = (pos.y as i32 + oy).clamp(0, GRID_SIZE as i32 - 1) as usize;
                        if self.cells[cx][cy].food_level > 0.0 {
                            let food_pos = vec2(cx as f32 + 0.5, cy as f32 + 0.5);
                            let dist = pos.distance(food_pos);
                            if dist < min_dist {
                                min_dist = dist;
                                target = Some(food_pos);
                            }
                        }
                    }
                }
            }

            // --- РУХ ---
            let gx = pos.x as usize;
            let gy = pos.y as usize;
            let speed_multiplier = match self.cells[gx][gy].terrain {
                Terrain::Tundra => SPEED_TUNDRA,
                Terrain::Forest => SPEED_FOREST,
                Terrain::Mountain => SPEED_MOUNTAIN,
            };

            if let Some(t) = target {
                let dir = t - pos;
                if dir.length() > 0.1 {
                    self.agents[i].pos += dir.normalize() * (MOVE_SPEED * speed_multiplier);
                }
            } else {
                self.agents[i].pos += vec2(rng.gen_range(-0.1..0.1), rng.gen_range(-0.1..0.1)) * speed_multiplier;
            }

            // Обмеження меж
            self.agents[i].pos.x = self.agents[i].pos.x.clamp(0.0, GRID_SIZE as f32 - 1.0);
            self.agents[i].pos.y = self.agents[i].pos.y.clamp(0.0, GRID_SIZE as f32 - 1.0);
            
            // Метаболізм
            self.agents[i].energy -= 0.15;

            // Поїдання
            let new_gx = self.agents[i].pos.x as usize;
            let new_gy = self.agents[i].pos.y as usize;
            if self.cells[new_gx][new_gy].food_level > 0.0 && self.agents[i].energy < 100.0 {
                let eat = 20.0f32.min(self.cells[new_gx][new_gy].food_level);
                self.cells[new_gx][new_gy].food_level -= eat;
                self.agents[i].energy += eat * 1.5;
            }
        }

        // 3. ФАКТ РОЗМНОЖЕННЯ (Контакт)
        let mut newborns = Vec::new();
        if self.agents.len() > 1 {
            let mut mated = vec![false; self.agents.len()];
            for i in 0..self.agents.len() {
                if mated[i] || self.agents[i].energy < REPRODUCTION_THRESHOLD || self.agents[i].reproduce_cooldown > 0.0 { continue; }
                for j in i+1..self.agents.len() {
                    if mated[j] || self.agents[j].energy < REPRODUCTION_THRESHOLD || self.agents[j].reproduce_cooldown > 0.0 { continue; }
                    
                    if self.agents[i].pos.distance(self.agents[j].pos) < MATING_DISTANCE {
                        mated[i] = true; mated[j] = true;
                        self.agents[i].energy -= 50.0;
                        self.agents[j].energy -= 50.0;
                        self.agents[i].reproduce_cooldown = COOLDOWN_TIME;
                        self.agents[j].reproduce_cooldown = COOLDOWN_TIME;
                        
                        newborns.push(Agent { 
                            pos: self.agents[i].pos, 
                            energy: 60.0, 
                            reproduce_cooldown: COOLDOWN_TIME 
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
        let cell_w = screen_width() / GRID_SIZE as f32;
        let cell_h = screen_height() / GRID_SIZE as f32;
        
        for x in 0..GRID_SIZE {
            for y in 0..GRID_SIZE {
                let cell = &self.cells[x][y];
                let color = match cell.terrain {
                    Terrain::Mountain => Color::new(0.3, 0.3, 0.35, 1.0),
                    Terrain::Forest => Color::new(0.0, 0.3, 0.1, 1.0),
                    Terrain::Tundra => Color::new(0.9, 0.9, 1.0, 1.0),
                };
                draw_rectangle(x as f32 * cell_w, y as f32 * cell_h, cell_w, cell_h, color);
                if cell.food_level > 0.0 { 
                    draw_rectangle(x as f32 * cell_w, y as f32 * cell_h, cell_w, cell_h, Color::new(0.6, 0.1, 0.8, 1.0)); 
                }
            }
        }
        
        for agent in &self.agents {
            // Якщо готовий до розмноження — стає жовтим (або помаранчевим)
            let color = if agent.energy > REPRODUCTION_THRESHOLD && agent.reproduce_cooldown == 0.0 {
                ORANGE 
            } else {
                RED
            };
            draw_circle(agent.pos.x * cell_w, agent.pos.y * cell_h, cell_w * 0.7, color);
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new();
    loop {
        clear_background(BLACK);
        world.update();
        world.draw();
        
        draw_text(&format!("Valkarai: {}", world.agents.len()), 20.0, 30.0, 30.0, GREEN);
        if is_key_down(KeyCode::Escape) { break; }
        next_frame().await
    }
}