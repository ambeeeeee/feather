// Specs systems tend to have very long
// tuples as their SystemData, and Clippy
// doesn't seem to like this.
#![allow(clippy::type_complexity)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate num_derive;
#[macro_use]
extern crate smallvec;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate derive_deref;
#[macro_use]
extern crate feather_codegen;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate feather_core;

extern crate nalgebra_glm as glm;

use crossbeam::Receiver;
use std::alloc::System;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use specs::{Builder, Dispatcher, DispatcherBuilder, Entity, LazyUpdate, World, WorldExt};

use feather_core::network::packet::implementation::DisconnectPlay;

use crate::chunk_logic::{ChunkHolders, ChunkWorkerHandle};
use crate::worldgen::{
    ComposableGenerator, EmptyWorldGenerator, SuperflatWorldGenerator, WorldGenerator,
};
use feather_core::level;
use feather_core::level::{deserialize_level_file, save_level_file, LevelData, LevelGeneratorType};
use rand::Rng;
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::Path;
use std::process::exit;
use crate::config::Config;

#[global_allocator]
static ALLOC: System = System;

pub mod chunk_logic;
pub mod chunkworker;
pub mod config;
pub mod io;
pub mod joinhandler;
pub mod network;
pub mod physics;
pub mod shutdown;
#[cfg(test)]
pub mod time;
pub mod worldgen;

pub const TPS: u64 = 20;
pub const PROTOCOL_VERSION: u32 = 404;
pub const SERVER_VERSION: &str = "Feather 1.13.2";
pub const TICK_TIME: u64 = 1000 / TPS;

#[derive(Default, Debug)]
pub struct PlayerCount(AtomicUsize);

#[derive(Default, Debug)]
pub struct TickCount(u64);

pub fn main() {
    let config = Arc::new(load_config());
    init_log(&config);

    info!("Starting Feather; please wait...");

    let server_icon = Arc::new(load_server_icon());

    let player_count = Arc::new(PlayerCount(AtomicUsize::new(0)));

    let io_manager = init_io_manager(
        Arc::clone(&config),
        Arc::clone(&player_count),
        Arc::clone(&server_icon),
    );

    let world_name = &config.world.name;
    let world_dir = Path::new(world_name.as_str());
    let level_file = &world_dir.join("level.dat");
    if !world_dir.is_dir() {
        info!(
            "World directory '{}' not found, creating it",
            world_dir.display()
        );
        // Create directory
        std::fs::create_dir(world_dir).unwrap();

        let level = create_level(&config);
        let root = level::Root { data: level };
        let mut level_file = File::create(level_file).unwrap();
        save_level_file(&root, &mut level_file).unwrap();
    }

    info!("Loading {}", level_file.to_str().unwrap());
    let level = load_level(level_file).unwrap_or_else(|e| {
        error!("Error occurred while loading level.dat: {}", e);
        error!("Please ensure that the world directory exists and is not corrupt.");
        exit(1)
    });

    let (mut world, mut dispatcher) = init_world(config, player_count, io_manager, level);

    // Channel used by the shutdown handler to notify the server thread.
    let (shutdown_tx, shutdown_rx) = crossbeam::unbounded();

    shutdown::init(shutdown_tx);

    info!("Initialized world");

    info!("Generating RSA keypair");
    io::init();

    info!("Queuing spawn chunks for loading");
    load_spawn_chunks(&mut world);

    info!("Server started");
    run_loop(&mut world, &mut dispatcher, shutdown_rx);

    info!("Shutting down");

    info!("Saving chunks");
    shutdown::save_chunks(&mut world);
    info!("Saving level.dat");
    shutdown::save_level(&world);
    info!("Saving player data");
    shutdown::save_player_data(&world);

    info!("Goodbye");
    exit(0);
}

/// Loads the configuration file, creating a default
/// one if it does not exist.
fn load_config() -> Config {
    match config::load_from_file("feather.toml") {
        Ok(config) => config,
        Err(e) => match e {
            config::ConfigError::Io(_) => {
                // Use default config
                println!("Config not found - creating it");
                let config = Config::default();
                let mut file = File::create("feather.toml").unwrap();
                file.write_all(config::DEFAULT_CONFIG_STR.as_bytes())
                    .unwrap();
                config
            }
            config::ConfigError::Parse(e) => {
                panic!("Failed to load configuration file: {}", e);
            }
        },
    }
}

fn create_level(config: &Config) -> LevelData {
    let seed = get_seed(config);
    let world_name = &config.world.name;
    debug!("Using seed {} for world '{}'", seed, world_name);

    // TODO: Generate spawn position properly
    LevelData {
        allow_commands: false,
        border_center_x: 0.0,
        border_center_z: 0.0,
        border_damage_per_block: 0.0,
        border_safe_zone: 0.0,
        border_size: 0.0,
        clear_weather_time: 0,
        data_version: 0,
        day_time: 0,
        difficulty: 0,
        difficulty_locked: 0,
        game_type: 0,
        hardcore: false,
        initialized: false,
        last_played: 0,
        raining: false,
        rain_time: 0,
        seed,
        spawn_x: 0,
        spawn_y: 100,
        spawn_z: 0,
        thundering: false,
        thunder_time: 0,
        time: 0,
        version: Default::default(),
        generator_name: config.world.generator.to_string(),
        generator_options: None,
    }
}

fn get_seed(config: &Config) -> i64 {
    let seed_raw = &config.world.seed;
    // Empty seed: random
    // Seed is valid i64: parse
    // Seed is something else: hash
    if seed_raw.is_empty() {
        rand::thread_rng().gen()
    } else {
        match seed_raw.parse::<i64>() {
            Ok(seed_int) => seed_int,
            Err(_) => hash_seed(seed_raw.as_str()),
        }
    }
}

fn hash_seed(seed_raw: &str) -> i64 {
    let mut hasher = DefaultHasher::new();
    seed_raw.hash(&mut hasher);
    hasher.finish() as i64
}
