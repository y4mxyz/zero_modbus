use log::*;
use std::collections::HashMap;
use tokio::task;
use zmq::{REP, Context, Socket, Message};
use serde_json::{self, json, Value, Map};
use futures::future;

use crate::interface::Interface;
use crate::modbus;


pub struct TaskPlan {
    todo_list: HashMap<String, Vec<(String, (String, Option<Value>))>>,
}

impl TaskPlan {

    pub fn new() -> Self {
        
        TaskPlan {
            todo_list: HashMap::new(),
        }

    }

    pub fn push(&mut self, path: &str, value: Option<Value>) {
    
            if !path.starts_with('/') {
                return;
            }
        
            let path_vec: Vec<&str> = path.split('/').collect();
            if path_vec.len() != 4 {
                return;
            }

            if self.todo_list.contains_key(path_vec[1]) {
                match self.todo_list.get_mut(path_vec[1]) {
                    Some(vec) => vec, None => { return; }
                }.push((path_vec[2].to_string(), (path_vec[3].to_string(), value)));
            } else {
                let new_vec = vec![{(path_vec[2].to_string(), (path_vec[3].to_string(), value))}];
                self.todo_list.insert(path_vec[1].to_string(), new_vec);
            }
    
    }

    pub fn plan(&self) -> Vec<(&String, &Vec<(String, (String, Option<Value>))>)> {

        let mut task_plan:Vec<(&String, &Vec<(String, (String, Option<Value>))>)> = Vec::new();

        for (interface_name, request_info) in &self.todo_list {
            task_plan.push((interface_name, request_info));
        }

        task_plan

    }
    
}


pub struct Server {
    socket: Socket,
    message: Message,
}

macro_rules! send_response {
    ($socket:expr, $message:expr) => {{
        let __response = $message.to_string();
        match $socket.send(__response.as_str(), 0) {
            Ok(_) => {
                info!("Response sent: {}", __response.len());
            }
            Err(e) => {
                error!("Error when send response: {}", e);
            }
        }
    }};
}

impl Server {

    pub fn new(address: &str) -> Self {

        let context = Context::new();
        let server = Server {
            socket: context.socket(REP)
                .expect("Failed to create socket"),
            message: Message::new(),
        };

        server.socket.bind(address)
            .expect(format!("Failed to bind socket to '{}'", address).as_str());
        
        server

    }

    pub fn send_error(&self, error: &str, details: String) {

        send_response!(self.socket, json!({"ERROR": error, "DETAILS": details}));
        
    }

    pub async fn handle_test(&self, body: &Value, device_list: &HashMap<String, Interface>) -> Option<()> {

        let key = String::from(body.as_str()?);
        if device_list.contains_key(&key) {
            send_response!(self.socket, json!({"TEST": key}));
        } else {
            send_response!(self.socket, json!({"TEST": key}));
        }

        Some(())

    }

    pub async fn handle_get(&mut self, body: &Value, device_list: &HashMap<String, Interface>) -> Option<()> {

        let mut planner = TaskPlan::new();
        for path in body.as_array()? {
            planner.push(path.as_str()?, None);
        }
        let plan: Vec<(&String, &Vec<(String, (String, Option<Value>))>)> = planner.plan();

        let mut results_table = Map::new();

        for (interface_name, request_info) in plan {

            info!("Batch read from '{}': {}", interface_name, request_info.len());
            
            let mut tasks = Vec::new();
            
            if device_list.contains_key(interface_name) {
                let handle = task::spawn(
                    modbus::batch_request(device_list.get(interface_name)?.clone(), request_info.clone(), modbus::GetOrSet::Get)
                );
                tasks.push(handle);
            } else {
                return None;
            }

            for results in future::join_all(tasks).await {
                
                match results {
                    Ok(results) => match results {
                        Ok(results) => {
                            for (key, value) in results {
                                results_table.insert(key, value);
                            }
                        },
                        Err(modbus_error) => {
                            self.send_error("MODBUS ERROR", format!("{}", modbus_error));
                        }
                    },
                    Err(_) => {
                        panic!("Task execute error");
                    }
                }
            }

        }
        
        let mut wrapper = Map::new();
        wrapper.insert("GET".to_string(), Value::Object(results_table));

        send_response!(self.socket, Value::Object(wrapper).to_string());

        Some(())

    }

    pub async fn handle_set(&self, body: &Value, device_list: &HashMap<String, Interface>) -> Option<()> {

        let mut planner = TaskPlan::new();
        for (path, value) in body.as_object()? {
            planner.push(path, Some(value.clone()));
        }
        let plan: Vec<(&String, &Vec<(String, (String, Option<Value>))>)> = planner.plan();

        for (interface_name, request_info) in plan {

            info!("Batch write to '{}': {}", interface_name, request_info.len());
            
            let mut tasks = Vec::new();
            
            if device_list.contains_key(interface_name) {
                let handle = task::spawn(
                    modbus::batch_request(device_list.get(interface_name)?.clone(), request_info.clone(), modbus::GetOrSet::Set)
                );
                tasks.push(handle);
            } else {
                return None;
            }

            for results in future::join_all(tasks).await {
                
                match results {
                    Ok(results) => match results {
                        Ok(_) => {},
                        Err(modbus_error) => {
                            self.send_error("MODBUS ERROR", format!("{}", modbus_error));
                        }
                    },
                    Err(_) => {
                        panic!("Task execute error");
                    }
                }
            }

        }

        send_response!(self.socket, "{\"SET\":null}");

        Some(())

    }

    async fn handle_message(&mut self, device_list: &HashMap<String, Interface>) -> Option<()> {
            
        let string = self.message.as_str()?;
    
        let result: Value = match serde_json::from_str(string) {
            Ok(result) => Some(result),
            Err(_) => None
        }?;
    
        let object = result.as_object()?;
    
        if object.len() != 1 {
            return None;
        }
    
        for (method, body) in object {
            
            match method.to_uppercase().as_str() {
                "TEST" => match self.handle_test(body, device_list).await {
                    Some(_) => {}, None => {
                        self.send_error("INVAILED TEST", format!("{}", body));
                    }
                },
                "GET" => match self.handle_get(body, device_list).await {
                    Some(_) => {}, None => {
                        self.send_error("INVAILED GET", format!("{}", body));
                    }
                },
                "SET" => match self.handle_set(body, device_list).await {
                    Some(_) => {}, None => {
                        self.send_error("INVAILED SET", format!("{}", body));
                    }
                },
                _ => {
                    self.send_error("INVAILED METHOD", format!("{}", body));
                }
            }

            break;
    
        }
    
        Some(())
        
    }

    pub async fn forever(&mut self, device_list: &HashMap<String, Interface>) {

        loop {

            self.socket.recv(&mut self.message, 0)
                .expect("Failed to receive message");

            info!("Request received: {}", self.message.len());

            match self.handle_message(device_list).await {
                Some(_) => {},
                None => {
                    self.send_error("INVAILD REQUEST", format!(""));
                    continue;
                }
            };

        }

    }

}