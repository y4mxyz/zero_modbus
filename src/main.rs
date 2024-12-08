use std::{collections::HashMap, env};
use simple_logger::SimpleLogger;
use log::*;

pub mod interface;
pub mod modbus;
pub mod server;
use interface::Interface;
use server::Server;


#[tokio::main]
async fn main() {

    SimpleLogger::new().init().expect("Failed to init logger");
    
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: {} zmq_address device_1_name:<device_1.yaml> device_2_name:<device_2.yaml> ...\n", args[0]);
    }

    let mut device_list: HashMap<String, Interface> = HashMap::new();
    for arg in &args[2..] {

        let arg_parts: Vec<&str> = arg.split(':').collect();
        if arg_parts.len() != 3 {
            panic!("Invaild arg format: '{}'", arg);
        }

        let (device_name, file_name) = (arg_parts[0], arg_parts[1]);
        device_list.insert(String::from(device_name),Interface::from_yaml(file_name));
        info!("Config file '{}' loaded.", file_name);
        info!("- {}:", device_name);
        let key = String::from(device_name);
        for line in format!("{}", &device_list[&key]).split('\n') {
            if line.len() > 1 {
                info!(" - {}", line);
            }
        };

    }

    Server::new(&args[1]).forever(&device_list).await;
    
}
