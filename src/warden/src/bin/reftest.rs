extern crate gfx_backend_vulkan as back;
extern crate gfx_hal as hal;
extern crate gfx_warden as warden;
extern crate ron;
extern crate serde;

use std::fs::File;
use std::io::Read;

use hal::Instance;
use ron::de::Deserializer;
use serde::de::Deserialize;


fn main() {
    let raw_scene = {
        let mut raw_data = Vec::new();
        File::open("../../reftests/scenes/basic.ron")
            .unwrap()
            .read_to_end(&mut raw_data)
            .unwrap();
        let mut deserializer = Deserializer::from_bytes(&raw_data);
        warden::raw::Scene::deserialize(&mut deserializer)
            .unwrap()
    };

    let instance = back::Instance::create("warden", 1);
    let adapters = instance.enumerate_adapters();
    let mut scene = warden::gpu::Scene::<back::Backend>::new(&adapters[0], &raw_scene);
    scene.run(&["test".to_string()]);
}
