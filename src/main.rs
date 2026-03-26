//! A minimal example that outputs "hello world"

use bevy::prelude::*;

fn main() {
    App::new().add_systems(Update, hello_world_system).run();
}

fn hello_world_system()
{
<<<<<<< HEAD
    println!("hello world!!");
=======
    println!("hello world!");
>>>>>>> f0d0442bf45ed462849fa2fe7eabbf5e5404bf8f
}