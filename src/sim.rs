use macroquad::prelude::*;
use ::rand::Rng;
use noise::{NoiseFn, Perlin};

// --- КОНСТАНТИ ---
const GRID_SIZE: usize = 200;
const MATING_DISTANCE: f32 = 1.2;
const COOLDOWN_TIME: f32 = 150.0;
const REPRODUCTION_THRESHOLD: f32 = 90.0;
const SPEED_PLAINS: f32 = 1.0;
const SPEED_FOREST: f32 = 0.6;
const SPEED_MOUNTAIN: f32 = 0.2;
const SPEED_DESERT: f32 = 0.7;
const SPEED_OCEAN: f32 = 0.0;
pub const INVENTORY_SLOTS: usize = 6;
const INVENTORY_SLOT_CAPACITY: f32 = 100.0;
const BIOME_BASE_SCALE: f64 = 0.006; // larger regions
const BIOME_DETAIL_SCALE: f64 = 0.02;
const BIOME_WARP_SCALE: f64 = 0.01;
const BIOME_WARP_STRENGTH: f64 = 0.2; // subtle warp to avoid streaks

pub fn window_conf() -> Conf {
    Conf {
        window_title: "Quadrisrah: Entity Registry".to_owned(),
        fullscreen: true,
        ..Default::default()
    }
}

// --- СТРУКТУРИ ---
#[derive(Clone, Copy, PartialEq)]
enum Terrain { Mountain, Plains, Forest, Desert, Ocean }
#[derive(Clone, Copy, PartialEq)]
pub enum AgentKind { Valkarai, Zombie }

#[derive(Clone, Copy, PartialEq)]
pub enum AnimalSpecies { Horse, Cow, Pig }

#[derive(Clone, Copy, PartialEq)]
pub enum StatsCategory { Valkarai, Zombie, Animal }

fn animal_reproduction_requirement(species: AnimalSpecies) -> f32 {
    match species {
        AnimalSpecies::Horse => 80.0,
        AnimalSpecies::Cow => 70.0,
        AnimalSpecies::Pig => 60.0,
    }
}

fn animal_reproduction_cost(species: AnimalSpecies) -> f32 {
    match species {
        AnimalSpecies::Horse => 40.0,
        AnimalSpecies::Cow => 30.0,
        AnimalSpecies::Pig => 20.0,
    }
}

fn animal_reproduction_cooldown(species: AnimalSpecies) -> f32 {
    match species {
        AnimalSpecies::Horse => 180.0,
        AnimalSpecies::Cow => 130.0,
        AnimalSpecies::Pig => 100.0,
    }
}

fn animal_energy_drain(species: AnimalSpecies, speed: f32, vision: f32) -> f32 {
    match species {
        // Lowest energy spend among animals
        AnimalSpecies::Horse => 0.06 + speed * 0.30 + vision * 0.004,
        // Higher than Valkarai on average
        AnimalSpecies::Cow => 0.12 + speed * 0.50 + vision * 0.007,
        // Highest energy spend
        AnimalSpecies::Pig => 0.16 + speed * 0.62 + vision * 0.008,
    }
}

fn classify_terrain(height: f64, moisture: f64) -> Terrain {
    if height < -0.18 {
        return Terrain::Ocean;
    }
    if height > 0.28 {
        return Terrain::Mountain;
    }

    if moisture > 0.25 {
        Terrain::Forest
    } else if moisture < -0.15 {
        Terrain::Desert
    } else {
        Terrain::Plains
    }
}

fn fractal_noise(noise: &Perlin, x: f64, y: f64, octaves: usize) -> f64 {
    let mut total = 0.0;
    let mut frequency = 1.0;
    let mut amplitude = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        total += noise.get([x * frequency, y * frequency]) * amplitude;
        max_value += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    total / max_value
}

fn blur_map(map: &mut Vec<Vec<f64>>, iterations: usize) {
    let size = map.len();
    if size == 0 { return; }
    let mut scratch = vec![vec![0.0; size]; size];
    for _ in 0..iterations {
        for x in 0..size {
            for y in 0..size {
                let mut sum = 0.0;
                let mut count = 0.0;
                for ox in -1..=1 {
                    for oy in -1..=1 {
                        let nx = (x as i32 + ox).clamp(0, (size - 1) as i32) as usize;
                        let ny = (y as i32 + oy).clamp(0, (size - 1) as i32) as usize;
                        sum += map[nx][ny];
                        count += 1.0;
                    }
                }
                scratch[x][y] = sum / count;
            }
        }
        for x in 0..size {
            for y in 0..size {
                map[x][y] = scratch[x][y];
            }
        }
    }
}

fn terrain_elevation(t: Terrain) -> f32 {
    match t {
        Terrain::Ocean => 0.0,
        Terrain::Desert => 0.22,
        Terrain::Plains => 0.30,
        Terrain::Forest => 0.46,
        Terrain::Mountain => 0.95,
    }
}

fn terrain_base_color(t: Terrain) -> Color {
    match t {
        Terrain::Mountain => Color::new(0.46, 0.47, 0.50, 1.0),
        Terrain::Forest => Color::new(0.09, 0.42, 0.12, 1.0),
        Terrain::Plains => Color::new(0.43, 0.68, 0.22, 1.0),
        Terrain::Desert => Color::new(0.85, 0.78, 0.46, 1.0),
        Terrain::Ocean => Color::new(0.10, 0.36, 0.74, 1.0),
    }
}

fn shade_color(base: Color, factor: f32) -> Color {
    Color::new(
        (base.r * factor).clamp(0.0, 1.0),
        (base.g * factor).clamp(0.0, 1.0),
        (base.b * factor).clamp(0.0, 1.0),
        base.a,
    )
}

#[derive(Clone)]
struct Cell { terrain: Terrain, food_level: f32, food_saturation: f32 }

pub struct Agent {
    pub pos: Vec2,
    pub energy: f32,
    pub reproduce_cooldown: f32,
    pub speed_gen: f32,
    pub vision_gen: f32,
    pub kind: AgentKind,
    pub health: f32,
    pub damage: f32,
    pub search_dir: Vec2,
    pub inventory: Vec<Item>,
}

#[derive(Clone)]
pub struct Item {
    pub id: u32,
    pub name: String,
    pub quantity: f32,
    pub saturation: f32,
}

impl Agent {
    fn random_search_dir(rng: &mut ::rand::rngs::ThreadRng) -> Vec2 {
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        vec2(angle.cos(), angle.sin())
    }

    fn inventory_slots_left(&self) -> usize {
        INVENTORY_SLOTS.saturating_sub(self.inventory.len())
    }

    fn has_inventory_space(&self) -> bool {
        self.inventory.len() < INVENTORY_SLOTS
    }

    // Store food into inventory slots. Each slot can hold up to INVENTORY_SLOT_CAPACITY.
    // Returns how much of `quantity` was actually stored.
    fn store_food(&mut self, mut quantity: f32, saturation: f32) -> f32 {
        let initial = quantity;
        let eps = 0.001;
        // First try to merge into existing slots with same saturation (within eps)
        for it in self.inventory.iter_mut() {
            if (it.saturation - saturation).abs() < 0.01 && it.quantity < INVENTORY_SLOT_CAPACITY {
                let space = INVENTORY_SLOT_CAPACITY - it.quantity;
                let take = space.min(quantity);
                it.quantity += take;
                quantity -= take;
                if quantity <= eps { break; }
            }
        }

        // Then create new slots if there is space
        while quantity > eps && self.has_inventory_space() {
            let take = quantity.min(INVENTORY_SLOT_CAPACITY);
            let id = ::rand::thread_rng().gen::<u32>();
            self.inventory.push(Item { id, name: "FOOD".to_string(), quantity: take, saturation });
            quantity -= take;
        }

        initial - quantity
    }

    fn consume_stored_food(&mut self) -> Option<Item> {
        if let Some(pos) = self.inventory.iter().position(|it| it.name == "FOOD") {
            return Some(self.inventory.remove(pos));
        }
        None
    }

    fn item_count(&self) -> usize { self.inventory.len() }
}

pub struct World {
    cells: Vec<Vec<Cell>>,
    pub agents: Vec<Agent>,
    pub animals: Vec<Animal>,
}

pub struct Animal {
    pub pos: Vec2,
    pub energy: f32,
    pub reproduce_cooldown: f32,
    pub speed: f32,
    pub vision: f32,
    pub species: AnimalSpecies,
}

impl World {
    pub fn new() -> Self {
        let mut rng = ::rand::thread_rng();
        let area_ratio = (GRID_SIZE as f32 / 200.0).powi(2);
        let base_valkarai = (40.0 * area_ratio).round().max(5.0) as usize;
        let base_animals = (80.0 * area_ratio).round().max(10.0) as usize;
        let base_zombies = (1.0 * area_ratio).ceil().max(1.0) as usize;
        let seed = rng.gen::<u32>();
        let height_noise = Perlin::new(seed);
        let detail_noise = Perlin::new(seed.wrapping_add(1));
        let moisture_noise = Perlin::new(seed.wrapping_add(2));
        let warp_noise = Perlin::new(seed.wrapping_add(3));
        let mut cells = vec![vec![Cell { terrain: Terrain::Plains, food_level: 0.0, food_saturation: 0.0 }; GRID_SIZE]; GRID_SIZE];
        let mut height_map = vec![vec![0.0f64; GRID_SIZE]; GRID_SIZE];
        let mut moisture_map = vec![vec![0.0f64; GRID_SIZE]; GRID_SIZE];

        let scale_factor = (200.0 / GRID_SIZE as f64).clamp(0.6, 2.0);
        let base_scale = BIOME_BASE_SCALE * scale_factor;
        let detail_scale = BIOME_DETAIL_SCALE * scale_factor;
        let warp_scale = BIOME_WARP_SCALE * scale_factor;

        for x in 0..GRID_SIZE {
            for y in 0..GRID_SIZE {
            let base_x = x as f64 * base_scale;
            let base_y = y as f64 * base_scale;

            let warp_x = warp_noise.get([x as f64 * warp_scale + 37.0, y as f64 * warp_scale + 91.0]) * (BIOME_WARP_STRENGTH * 0.65);
            let warp_y = warp_noise.get([x as f64 * warp_scale + 113.0, y as f64 * warp_scale + 53.0]) * (BIOME_WARP_STRENGTH * 0.65);

            let height = fractal_noise(&height_noise, base_x + warp_x, base_y + warp_y, 5);
                let ridge = fractal_noise(&detail_noise, base_x * 1.8 + detail_scale, base_y * 1.8 + detail_scale, 3);
            let height = (height * 0.80 + ridge * 0.20).clamp(-1.0, 1.0);

            let moisture = fractal_noise(&moisture_noise, base_x * 0.9 + warp_y * 0.2, base_y * 0.9 + warp_x * 0.2, 4);
            let moisture = (moisture * 1.05).clamp(-1.0, 1.0);

                height_map[x][y] = height;
                moisture_map[x][y] = moisture;
            }
        }

        // Smooth height/moisture to avoid ribbon-like biomes on large maps.
        blur_map(&mut height_map, 2);
        blur_map(&mut moisture_map, 2);

        for x in 0..GRID_SIZE {
            for y in 0..GRID_SIZE {
                let terrain = classify_terrain(height_map[x][y], moisture_map[x][y]);
                cells[x][y].terrain = terrain;
            }
        }

        // Smooth biome map to remove thin streaks and small noisy patches.
        // Do a few majority-filter iterations over terrain types.
        let mut terrain_map: Vec<Vec<Terrain>> = cells.iter()
            .map(|r| r.iter().map(|c| c.terrain).collect())
            .collect();

        for _iter in 0..3 {
            let mut new_map = terrain_map.clone();
            for x in 0..GRID_SIZE {
                for y in 0..GRID_SIZE {
                    let mut counts = [0u16; 5];
                    for ox in -1..=1 {
                        for oy in -1..=1 {
                            let nx = (x as i32 + ox).clamp(0, (GRID_SIZE - 1) as i32) as usize;
                            let ny = (y as i32 + oy).clamp(0, (GRID_SIZE - 1) as i32) as usize;
                            match terrain_map[nx][ny] {
                                Terrain::Mountain => counts[0] += 1,
                                Terrain::Plains => counts[1] += 1,
                                Terrain::Forest => counts[2] += 1,
                                Terrain::Desert => counts[3] += 1,
                                Terrain::Ocean => counts[4] += 1,
                            }
                        }
                    }
                    let mut best = 0usize;
                    let mut best_v = 0u16;
                    for (i, &v) in counts.iter().enumerate() {
                        if v > best_v { best_v = v; best = i; }
                    }
                    new_map[x][y] = match best {
                        0 => Terrain::Mountain,
                        1 => Terrain::Plains,
                        2 => Terrain::Forest,
                        3 => Terrain::Desert,
                        _ => Terrain::Ocean,
                    };
                }
            }
            terrain_map = new_map;
        }

        // Write smoothed terrain back into cells
        for x in 0..GRID_SIZE {
            for y in 0..GRID_SIZE {
                cells[x][y].terrain = terrain_map[x][y];
            }
        }

        let mut agents = Vec::new();
        let mut animals = Vec::new();
        for _ in 0..base_valkarai {
            let mut p = vec2(rng.gen_range(0.0..GRID_SIZE as f32), rng.gen_range(0.0..GRID_SIZE as f32));
            while cells[p.x as usize][p.y as usize].terrain == Terrain::Ocean {
                p = vec2(rng.gen_range(0.0..GRID_SIZE as f32), rng.gen_range(0.0..GRID_SIZE as f32));
            }
            agents.push(Agent {
                pos: p,
                energy: 100.0, reproduce_cooldown: 0.0,
                speed_gen: rng.gen_range(0.12..0.22), vision_gen: rng.gen_range(10.0..20.0),
                kind: AgentKind::Valkarai, health: 100.0, damage: 10.0,
                search_dir: Agent::random_search_dir(&mut rng),
                inventory: Vec::new(),
            });
        }
        for _ in 0..base_zombies {
            let mut p = vec2(rng.gen_range(0.0..GRID_SIZE as f32), rng.gen_range(0.0..GRID_SIZE as f32));
            while cells[p.x as usize][p.y as usize].terrain == Terrain::Ocean {
                p = vec2(rng.gen_range(0.0..GRID_SIZE as f32), rng.gen_range(0.0..GRID_SIZE as f32));
            }
            agents.push(Agent {
                pos: p, energy: 10000.0, reproduce_cooldown: 0.0,
                speed_gen: 0.15, vision_gen: 15.0, kind: AgentKind::Zombie, health: 300.0, damage: 20.0,
                search_dir: Agent::random_search_dir(&mut rng),
                inventory: Vec::new(),
            });
        }
        // spawn animals across map with terrain preferences
        let mut tries = 0;
        while animals.len() < base_animals && tries < 10000 {
            tries += 1;
            let x = rng.gen_range(0..GRID_SIZE);
            let y = rng.gen_range(0..GRID_SIZE);
            let t = cells[x][y].terrain;
            // spawn probability by terrain
            let prob = match t {
                Terrain::Plains => 0.8,
                Terrain::Forest => 0.45,
                Terrain::Desert => 0.08,
                Terrain::Mountain => 0.03,
                Terrain::Ocean => 0.0,
            };
            if rng.gen_bool(prob) {
                let species = match rng.gen_range(0..3) {
                    0 => AnimalSpecies::Horse,
                    1 => AnimalSpecies::Cow,
                    _ => AnimalSpecies::Pig,
                };
                animals.push(Animal {
                    pos: vec2(x as f32 + 0.5, y as f32 + 0.5),
                    energy: 80.0 + rng.gen_range(0.0..40.0),
                    reproduce_cooldown: 0.0,
                    speed: match species {
                        AnimalSpecies::Horse => rng.gen_range(0.2..0.32),
                        AnimalSpecies::Cow =>   rng.gen_range(0.1..0.2),
                        AnimalSpecies::Pig =>   rng.gen_range(0.08..0.17),
                    },
                    vision: rng.gen_range(6.0..14.0),
                    species,
                });
            }
        }

        World { cells, agents, animals }
    }


    pub fn update(&mut self) {
        let mut rng = ::rand::thread_rng();
        let area_ratio = (GRID_SIZE as f32 / 200.0).powi(2);
        let food_iters = (3.0 * area_ratio).round().max(1.0) as usize;
        for _ in 0..food_iters {
            if rng.gen_bool(0.8) {
                let x = rng.gen_range(0..GRID_SIZE);
                let y = rng.gen_range(0..GRID_SIZE);
                let chance = match self.cells[x][y].terrain {
                    Terrain::Plains => 0.4,
                    Terrain::Forest => 0.6,
                    Terrain::Mountain => 0.1,
                    Terrain::Desert => 0.05,
                    Terrain::Ocean => 0.0,
                };
            if rng.gen_bool(chance) {
                let add = 80.0f32;
                // terrain-based saturation (0.0..1.0)
                let sat = match self.cells[x][y].terrain {
                    Terrain::Plains => 1.0,
                    Terrain::Forest => 0.7,
                    Terrain::Mountain => 0.5,
                    Terrain::Desert => 0.25,
                    Terrain::Ocean => 0.0,
                };
                // weighted average saturation if food already present
                let old = self.cells[x][y].food_level;
                let old_sat = self.cells[x][y].food_saturation;
                let new_level = old + add;
                let new_sat = if old <= 0.0 { sat } else { (old * old_sat + add * sat) / new_level };
                self.cells[x][y].food_level = new_level;
                self.cells[x][y].food_saturation = new_sat;
            }
            }
        };

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

                if target.is_none() {
                    if rng.gen_bool(0.04) {
                        self.agents[i].search_dir = Agent::random_search_dir(&mut rng);
                    }
                    target = Some(pos + self.agents[i].search_dir * (vision * 2.0));
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
                        let mut best_score = f32::MAX; // Чим менше, тим краще
                        let hunger = (100.0 - self.agents[i].energy).clamp(0.0, 100.0) / 100.0;
                        let v_int = vision as i32;
                        
                        for ox in -v_int..=v_int {
                            for oy in -v_int..=v_int {
                                let cx = (pos.x as i32 + ox).clamp(0, GRID_SIZE as i32 - 1) as usize;
                                let cy = (pos.y as i32 + oy).clamp(0, GRID_SIZE as i32 - 1) as usize;

                                if self.cells[cx][cy].food_level > 0.0 {
                                    let d = pos.distance(vec2(cx as f32 + 0.5, cy as f32 + 0.5));
                                    let sat = self.cells[cx][cy].food_saturation.clamp(0.0, 1.0);
                                    if self.cells[cx][cy].terrain == Terrain::Ocean { continue; }

                                    // Higher saturation should make food more attractive.
                                    // Hunger makes Valkarai care a bit less about saturation and more about distance.
                                    let sat_weight = 0.45 + (0.55 * hunger);
                                    let mut score = d / (0.25 + sat * sat_weight);

                                    if score < best_score {
                                        best_score = score;
                                        target = Some(vec2(cx as f32 + 0.5, cy as f32 + 0.5));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let speed_mult = match self.cells[pos.x as usize][pos.y as usize].terrain {
                Terrain::Plains => SPEED_PLAINS,
                Terrain::Forest => SPEED_FOREST,
                Terrain::Mountain => SPEED_MOUNTAIN,
                Terrain::Desert => SPEED_DESERT,
                Terrain::Ocean => 0.1, // Мінімальна швидкість, щоб не застрягнути "всередині" клітинки
            };
            let cur_speed = self.agents[i].speed_gen * speed_mult;

            // 2. Розрахунок бажаного вектора руху
            let mut desired_move = if let Some(dir) = flee_dir {
                dir.normalize() * (cur_speed * 1.3)
            } else if let Some(t) = target {
                let d = t - pos;
                if d.length() > 0.1 { d.normalize() * cur_speed } else { vec2(0.0, 0.0) }
            } else {
                // Wander: легке випадкове блукання
                vec2(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0)).normalize() * (cur_speed * 0.5)
            };

            // 3. ПЕРЕВІРКА ПЕРЕШКОД (Стіни)
            if desired_move.length() > 0.0 {
                let mut next_pos = pos + desired_move;
                
                // Функція перевірки: чи можна агенту стояти на цій клітинці?
                let can_stand_at = |p: Vec2, kind: AgentKind, energy: f32| -> bool {
                    let x = p.x.clamp(0.0, (GRID_SIZE - 1) as f32) as usize;
                    let y = p.y.clamp(0.0, (GRID_SIZE - 1) as f32) as usize;
                    let t = self.cells[x][y].terrain;
                    
                    if t == Terrain::Ocean { return false; } // Океан - стіна для всіх
                    
                    if kind == AgentKind::Valkarai {
                        let is_dire = energy < 40.0 || flee_dir.is_some();
                        if t == Terrain::Desert && !is_dire { return false; } // Пустеля - стіна для ситих Валкараїв
                    }
                    
                    true // Всі інші випадки (включаючи Зомбі на піску) - ОК
                };

                // Якщо прямий шлях заблоковано, пробуємо ковзати (окремо по X та Y)
                if !can_stand_at(next_pos, kind, self.agents[i].energy) {
                    // Пробуємо йти тільки по X
                    let next_x = vec2(pos.x + desired_move.x, pos.y);
                    if can_stand_at(next_x, kind, self.agents[i].energy) {
                        next_pos = next_x;
                    } else {
                        // Пробуємо йти тільки по Y
                        let next_y = vec2(pos.x, pos.y + desired_move.y);
                        if can_stand_at(next_y, kind, self.agents[i].energy) {
                            next_pos = next_y;
                        } else {
                            // Якщо все заблоковано - стоїмо
                            next_pos = pos;
                        }
                    }
                }
                
                self.agents[i].pos = next_pos;
            }

            // 4. Межі світу (про всяк випадок)
            self.agents[i].pos.x = self.agents[i].pos.x.clamp(0.0, (GRID_SIZE - 1) as f32);
            self.agents[i].pos.y = self.agents[i].pos.y.clamp(0.0, (GRID_SIZE - 1) as f32);

            if kind == AgentKind::Valkarai {
                self.agents[i].energy -= 0.1 + (self.agents[i].vision_gen * 0.006) + (self.agents[i].speed_gen * 0.45);
                let (nx, ny) = (self.agents[i].pos.x as usize, self.agents[i].pos.y as usize);
                if self.cells[nx][ny].food_level > 0.0 {
                    let sat = self.cells[nx][ny].food_saturation.clamp(0.0, 1.0);
                    let harvest = 20.0f32.min(self.cells[nx][ny].food_level);

                    // Try to store up to `harvest` into inventory first if agent is full or wants to stockpile
                    let mut stored_amount = 0.0;
                    if self.agents[i].has_inventory_space() {
                        stored_amount = self.agents[i].store_food(harvest, sat);
                        if stored_amount > 0.0 {
                            self.cells[nx][ny].food_level -= stored_amount;
                        }
                    }

                    // If agent still needs food (energy < 100), eat remaining harvest
                    let remaining = harvest - stored_amount;
                    if remaining > 0.0 && self.agents[i].energy < 100.0 {
                        let eat = remaining.min(self.cells[nx][ny].food_level);
                        self.cells[nx][ny].food_level -= eat;
                        self.agents[i].energy += eat * (1.0 + sat);
                    }
                }

                if self.agents[i].energy < 100.0 && self.agents[i].inventory.iter().any(|item| item.name == "FOOD") {
                    if let Some(item) = self.agents[i].consume_stored_food() {
                        self.agents[i].energy += item.quantity * (1.0 + item.saturation);
                    }
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
                        let mut ch = (self.agents[i].health + self.agents[j].health) / 2.0;
                        let mut cd = (self.agents[i].damage + self.agents[j].damage) / 2.0;
                        if rng.gen_bool(0.1) { cs *= rng.gen_range(0.9..1.1); cv *= rng.gen_range(0.9..1.1); ch *= rng.gen_range(0.9..1.1); cd *= rng.gen_range(0.9..1.1); }
                        newborns.push(Agent {
                            pos: self.agents[i].pos, energy: 60.0, reproduce_cooldown: COOLDOWN_TIME,
                            speed_gen: cs.clamp(0.08, 0.3), vision_gen: cv.clamp(8.0, 30.0), kind: AgentKind::Valkarai, health: ch,
                            damage: cd,
                            search_dir: Agent::random_search_dir(&mut rng),
                            inventory: Vec::new(),
                        });
                        break;
                    }
                }
            }
        }
        self.agents.append(&mut newborns);
        self.agents.retain(|a| a.energy > 0.0);

        // --- ANIMALS: movement, eating, reproduction ---
        for a in &mut self.animals {
            if a.reproduce_cooldown > 0.0 { a.reproduce_cooldown -= 1.0; }
            // perceive nearby food
            let pos = a.pos;
            let mut target: Option<Vec2> = None;
            let mut best_score = f32::MAX;
            let hunger = (100.0 - a.energy).clamp(0.0, 100.0) / 100.0;
            let v_int = a.vision as i32;
            for ox in -v_int..=v_int {
                for oy in -v_int..=v_int {
                    let cx = (pos.x as i32 + ox).clamp(0, GRID_SIZE as i32 - 1) as usize;
                    let cy = (pos.y as i32 + oy).clamp(0, GRID_SIZE as i32 - 1) as usize;
                    if self.cells[cx][cy].food_level <= 0.0 { continue; }
                    if self.cells[cx][cy].terrain == Terrain::Ocean { continue; }
                    let d = pos.distance(vec2(cx as f32 + 0.5, cy as f32 + 0.5));
                    let sat = self.cells[cx][cy].food_saturation.clamp(0.0, 1.0);
                    let sat_weight = 0.5 + 0.5 * hunger;
                    let score = d / (0.5 + sat * sat_weight);
                    if score < best_score { best_score = score; target = Some(vec2(cx as f32 + 0.5, cy as f32 + 0.5)); }
                }
            }

            // movement
            let speed_mult = match self.cells[pos.x as usize][pos.y as usize].terrain {
                Terrain::Plains => SPEED_PLAINS,
                Terrain::Forest => SPEED_FOREST,
                Terrain::Mountain => SPEED_MOUNTAIN,
                Terrain::Desert => SPEED_DESERT,
                Terrain::Ocean => 0.0,
            };
            let cur_speed = a.speed * speed_mult;
            let desired = if let Some(t) = target {
                let d = t - pos;
                if d.length() > 0.1 { d.normalize() * cur_speed } else { vec2(0.0, 0.0) }
            } else {
                vec2(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0)).normalize() * (cur_speed * 0.6)
            };

            if desired.length() > 0.0 {
                let mut next_pos = pos + desired;
                // avoid ocean
                let x = next_pos.x.clamp(0.0, (GRID_SIZE - 1) as f32) as usize;
                let y = next_pos.y.clamp(0.0, (GRID_SIZE - 1) as f32) as usize;
                if self.cells[x][y].terrain == Terrain::Ocean {
                    // try slide
                    let next_x = vec2(pos.x + desired.x, pos.y);
                    let nx = next_x.x.clamp(0.0, (GRID_SIZE - 1) as f32) as usize;
                    let ny = next_x.y.clamp(0.0, (GRID_SIZE - 1) as f32) as usize;
                    if self.cells[nx][ny].terrain != Terrain::Ocean { next_pos = next_x; }
                    else {
                        let next_y = vec2(pos.x, pos.y + desired.y);
                        let nx2 = next_y.x.clamp(0.0, (GRID_SIZE - 1) as f32) as usize;
                        let ny2 = next_y.y.clamp(0.0, (GRID_SIZE - 1) as f32) as usize;
                        if self.cells[nx2][ny2].terrain != Terrain::Ocean { next_pos = next_y; }
                        else { next_pos = pos; }
                    }
                }
                a.pos = next_pos;
            }

            // bounds
            a.pos.x = a.pos.x.clamp(0.0, (GRID_SIZE - 1) as f32);
            a.pos.y = a.pos.y.clamp(0.0, (GRID_SIZE - 1) as f32);

            // species-specific energy drain, depends on speed and vision
            a.energy -= animal_energy_drain(a.species, a.speed, a.vision);

            // eating from cell (no inventory)
            let (ax, ay) = (a.pos.x as usize, a.pos.y as usize);
            if self.cells[ax][ay].food_level > 0.0 {
                let sat = self.cells[ax][ay].food_saturation.clamp(0.0, 1.0);
                let eat = 8.0f32.min(self.cells[ax][ay].food_level);
                self.cells[ax][ay].food_level -= eat;
                a.energy += eat * (1.0 + sat);
            }
        }

        // animal reproduction
        let mut newborns_a = Vec::new();
        let mut mated_a = vec![false; self.animals.len()];
        for i in 0..self.animals.len() {
            let req_i = animal_reproduction_requirement(self.animals[i].species);
            if mated_a[i] || self.animals[i].energy < req_i || self.animals[i].reproduce_cooldown > 0.0 { continue; }
            for j in i+1..self.animals.len() {
                if self.animals[i].species != self.animals[j].species { continue; }
                let req_j = animal_reproduction_requirement(self.animals[j].species);
                if mated_a[j] || self.animals[j].energy < req_j || self.animals[j].reproduce_cooldown > 0.0 { continue; }
                if self.animals[i].pos.distance(self.animals[j].pos) < MATING_DISTANCE {
                    mated_a[i] = true; mated_a[j] = true;
                    let cost_i = animal_reproduction_cost(self.animals[i].species);
                    let cost_j = animal_reproduction_cost(self.animals[j].species);
                    self.animals[i].energy -= cost_i;
                    self.animals[j].energy -= cost_j;
                    // Parents now enter cooldown after mating.
                    self.animals[i].reproduce_cooldown = animal_reproduction_cooldown(self.animals[i].species);
                    self.animals[j].reproduce_cooldown = animal_reproduction_cooldown(self.animals[j].species);
                    let cs = (self.animals[i].speed + self.animals[j].speed) / 2.0;
                    let cv = (self.animals[i].vision + self.animals[j].vision) / 2.0;
                    let species = self.animals[i].species;
                    newborns_a.push(Animal {
                        pos: self.animals[i].pos,
                        energy: 40.0,
                        reproduce_cooldown: animal_reproduction_cooldown(species),
                        speed: match species {
                            AnimalSpecies::Horse => (cs * rng.gen_range(0.95..1.05)).clamp(0.14, 0.38),
                            AnimalSpecies::Cow =>   (cs * rng.gen_range(0.95..1.05)).clamp(0.07, 0.26),
                            AnimalSpecies::Pig =>   (cs * rng.gen_range(0.95..1.05)).clamp(0.05, 0.2),
                        },
                        vision: (cv * rng.gen_range(0.95..1.05)).clamp(4.0, 20.0),
                        species,
                    });
                    break;
                }
            }
        }
        self.animals.append(&mut newborns_a);
        // remove dead animals
        self.animals.retain(|a| a.energy > 0.0);
    }

    pub fn draw(&self) {
        let (cw, ch) = (screen_width() / GRID_SIZE as f32, screen_height() / GRID_SIZE as f32);
        let light_dir = vec3(-0.55, -0.35, 0.76).normalize();

        for x in 0..GRID_SIZE {
            for y in 0..GRID_SIZE {
                let cell = &self.cells[x][y];

                let xm = x.saturating_sub(1);
                let xp = (x + 1).min(GRID_SIZE - 1);
                let ym = y.saturating_sub(1);
                let yp = (y + 1).min(GRID_SIZE - 1);

                let e_l = terrain_elevation(self.cells[xm][y].terrain);
                let e_r = terrain_elevation(self.cells[xp][y].terrain);
                let e_u = terrain_elevation(self.cells[x][ym].terrain);
                let e_d = terrain_elevation(self.cells[x][yp].terrain);
                let e_c = terrain_elevation(cell.terrain);

                // Simple hill-shading from terrain elevation gradients.
                let dzdx = e_r - e_l;
                let dzdy = e_d - e_u;
                let normal = vec3(-dzdx * 2.3, -dzdy * 2.3, 1.0).normalize();
                let diffuse = normal.dot(light_dir).max(0.0);
                let ambient = 0.58;
                let height_boost = (e_c - 0.25).max(0.0) * 0.18;
                let shade = (ambient + diffuse * 0.55 + height_boost).clamp(0.45, 1.20);

                let base = terrain_base_color(cell.terrain);
                let shaded = shade_color(base, shade);
                draw_rectangle(x as f32 * cw, y as f32 * ch, cw, ch, shaded);

                if cell.food_level > 0.0 {
                    // color from purple (least) to dark blue (most) by saturation
                    let sat = cell.food_saturation.clamp(0.0, 1.0);
                    let purple = (0.6f32, 0.1f32, 0.8f32);
                    let dark_blue = (0.0f32, 0.0f32, 0.6f32);
                    let r = purple.0 * (1.0 - sat) + dark_blue.0 * sat;
                    let g = purple.1 * (1.0 - sat) + dark_blue.1 * sat;
                    let b = purple.2 * (1.0 - sat) + dark_blue.2 * sat;
                    draw_rectangle(x as f32 * cw, y as f32 * ch, cw, ch, Color::new(r, g, b, 1.0));
                }
            }
        }

        for agent in &self.agents {
            let color = if agent.kind == AgentKind::Zombie { BLACK }
                        else if agent.energy > REPRODUCTION_THRESHOLD && agent.reproduce_cooldown == 0.0 { ORANGE }
                        else { RED };
            draw_circle(agent.pos.x * cw, agent.pos.y * ch, (agent.vision_gen / 15.0) * cw * 0.7, color);
        }

        for animal in &self.animals {
            let (r, g, b) = match animal.species {
                AnimalSpecies::Horse => (0.8, 0.6, 0.2),
                AnimalSpecies::Cow => (0.9, 0.9, 0.7),
                AnimalSpecies::Pig => (0.9, 0.4, 0.6),
            };
            let col = Color::new(r as f32, g as f32, b as f32, 1.0);
            let cell_size = cw.min(ch);
            let radius = ((animal.vision / 15.0) * cell_size * 1.45).max(cell_size * 0.80);
            let cx = animal.pos.x * cw;
            let cy = animal.pos.y * ch;
            let p1 = vec2(cx, cy - radius);
            let p2 = vec2(cx - radius * 0.7, cy + radius * 0.6);
            let p3 = vec2(cx + radius * 0.7, cy + radius * 0.6);
            draw_triangle(p1, p2, p3, col);
        }
    }
}

