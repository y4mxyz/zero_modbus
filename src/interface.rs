use std::{collections::HashMap, fs::File, fmt};
use serde_yaml::{self, Value};


#[derive(Copy, Clone, PartialEq)]
pub enum ModbusProtocol {
    Rtu,
    Tcp,
}

#[derive(Copy, Clone, PartialEq)]
pub enum RequestFunction {
    Single,
    Multiple,
}

#[derive(Copy, Clone, PartialEq)]
pub enum BlockType {
    Co, Di,
    Hr, Ir,
}

#[derive(Copy, Clone, PartialEq)]
pub enum ValueType {
    Bool,
    U16, I16,
    U32, I32, F32,
}

impl ValueType {

    pub fn size(&self) -> usize {

        match self {
            ValueType::Bool => 1,
            ValueType::U16 => 1,
            ValueType::I16 => 1,
            ValueType::U32 => 2,
            ValueType::I32 => 2,
            ValueType::F32 => 2,
        }

    }

}

#[derive(Clone)]
pub struct ModbusData {
    address: u8,
    block_type: BlockType,
    value_type: ValueType,
    requestfunction: RequestFunction,
}

impl ModbusData {

    pub fn address(&self) -> u8 {

        self.address
        
    }

    pub fn block_type(&self) -> BlockType {

        self.block_type
        
    }

    pub fn value_type(&self) -> ValueType {

        self.value_type
        
    }

    pub fn requestfunction(&self) -> RequestFunction {

        self.requestfunction
        
    }
    
}

#[derive(Clone)]
pub struct SlaveData {
    id: u8,
    co: HashMap<String, ModbusData>,
    di: HashMap<String, ModbusData>,
    hr: HashMap<String, ModbusData>,
    ir: HashMap<String, ModbusData>,
}

impl SlaveData {

    pub fn new(id: u8,
        co: HashMap<String, ModbusData>,
        di: HashMap<String, ModbusData>,
        hr: HashMap<String, ModbusData>,
        ir: HashMap<String, ModbusData>,
    ) -> Self {

        SlaveData {
            id: id,
            co: co,
            di: di,
            hr: hr,
            ir: ir,
        }

    }

    pub fn id(&self) -> u8 {

        self.id

    }

    pub fn find(&self, name: &str) -> Option<ModbusData> {
        if self.co.contains_key(name) {
            return Some(self.co[name].clone());
        }
        if self.di.contains_key(name) {
            return Some(self.di[name].clone());
        }
        if self.hr.contains_key(name) {
            return Some(self.hr[name].clone());
        }
        if self.ir.contains_key(name) {
            return Some(self.ir[name].clone());
        }
        return None

    }

}


#[derive(Clone)]
pub struct Interface {
    modbusprotocol: ModbusProtocol,
    address: String,
    config: u32, // tcp port or serial baudrate
    pub slaves: HashMap<String, SlaveData>,
}

impl Interface {
    
    pub fn modbusprotocol(&self) -> ModbusProtocol {

        self.modbusprotocol
        
    }
    
    pub fn address(&self) -> String {

        self.address.clone()
        
    }
    
    pub fn config(&self) -> u32 {

        self.config
        
    }

}

macro_rules! missing_required_message {
    ($key:expr) => {
        format!("Missing required '{}'", $key).as_str()
    };
}

macro_rules! invailed_type_message {
    ($type:expr, $required:expr) => {
        format!("Invild type of '{}', required {}", $type, $required).as_str()
    };
}

macro_rules! invailed_value_message {
    ($name:expr, $value:expr) => {
        format!("Invaild value of '{}': '{}'", $name, $value).as_str()
    };
}

macro_rules! get_yaml_string {

    ($object:expr, $key:expr) => {

        String::from($object.get($key)
            .expect(missing_required_message!($key))
            .as_str()
            .expect(invailed_type_message!($key, "string"))
        )

    };

}

macro_rules! get_modbus_block_value {

    ($slave_info:expr, $key:expr) => {
    
        match $slave_info.get(&$key) {
            Some(value) => match value.as_sequence() {
                Some(map) => Some(map),
                None => {
                    panic!("Invaild value of data block, required sequence");
                },
            },
            None => None,
        }

    };

}

fn load_data_block(block_type: BlockType, block_infos: &Vec<Value>, map: &mut HashMap<String, ModbusData>) {

    for _block_info in block_infos {

        let block_map = match _block_info
            .as_mapping() {
            Some(map) => match map.len() {
                1 => map,
                _ => {
                    panic!("Invaild data block format");
                },
            },
            None => {
                panic!("Invaild data block format");
            },
        };
        for (_block_name, block_info) in block_map {

            let block_name = _block_name
                .as_str()
                .expect(invailed_type_message!("block name", "string"));

            let (address_key, value_type_key, function_key) = (
                Value::String(String::from("addr")),
                Value::String(String::from("type")),
                Value::String(String::from("func")),
            );
    
            let address_u64 = block_info.get(address_key)
                .expect(missing_required_message!("addr"))
                .as_u64()
                .expect(invailed_type_message!("addr", "string"));
            let address;
            if address_u64 < u8::MAX as u64 {
                address = address_u64 as u8;
            } else {
                panic!("{}", invailed_value_message!("addr", address_u64));
            }
    
            let mut value_type ;
            match block_type {
                BlockType::Co | BlockType::Di => {
                    value_type = ValueType::Bool
                }
                BlockType::Hr | BlockType::Ir => {
                    value_type = ValueType::Bool
                }
            }
            let value_type_option = block_info.get(value_type_key);
            if value_type_option.is_some() {
                let value_type_str = value_type_option
                    .unwrap()
                    .as_str()
                    .expect(invailed_type_message!("type", "string"));
                value_type = match value_type_str.to_lowercase().as_str() {
                    "bool" => ValueType::Bool,
                    "u16" => ValueType::U16,
                    "i16" => ValueType::I16,
                    "u32" => ValueType::U32,
                    "i32" => ValueType::I32,
                    "f32" => ValueType::F32,
                    _ => {
                        panic!("{}", invailed_value_message!("type", value_type_str));
                    }
                };
            }
            
            let mut requestfunction = RequestFunction::Multiple;
            if block_type == BlockType::Co || block_type == BlockType::Hr {
                let function_option = block_info.get(function_key);
                if function_option.is_some() {
                    let function_str = match function_option
                        .unwrap()
                        .as_str() {
                            Some(str) => str,
                            None => {
                                panic!("{}", invailed_type_message!("func", "string"));
                            },
                    };
                    requestfunction = match function_str.to_ascii_lowercase().as_str() {
                        "single" => RequestFunction::Single,
                        "multiple" => RequestFunction::Multiple,
                        _ => {
                            panic!("{}", invailed_value_message!("func", function_str));
                        },
                    }
                }
            }
    
            map.insert(String::from(block_name), ModbusData {
                address: address,
                block_type: block_type,
                value_type: value_type,
                requestfunction: requestfunction,
            });

        }

    }

}

impl Interface {
   
    pub fn from_yaml(yaml_filename: &str) -> Interface {
    
        let yaml_file = File::open(yaml_filename)
            .expect(format!("Could not open file '{}'", yaml_filename).as_str());
    
        let yaml_config: Value = serde_yaml::from_reader(yaml_file)
            .expect(format!("Failed to parse yaml file '{}'", yaml_filename).as_str());

        let protocol_name = get_yaml_string!(&yaml_config, "protocol");
        let protocol_cased = protocol_name.to_lowercase();
        let modbusprotocol = match protocol_cased.as_str() {
            "rtu" => ModbusProtocol::Rtu,
            "tcp" => ModbusProtocol::Tcp,
            _ => {
                panic!("Invailed modbusprotocol '{}'", protocol_name);
            },
        };
    
        let address = get_yaml_string!(&yaml_config, "address");

        let config_key = match modbusprotocol {
            ModbusProtocol::Rtu => "baudrate",
            ModbusProtocol::Tcp => "tcp_port",
        };
        let config_u64 = yaml_config.get(config_key)
            .expect(format!("Missing required '{}' in '{}' modbusprotocol", config_key, protocol_name).as_str())
            .as_u64()
            .expect(invailed_type_message!(config_key, "unsigned integetr"));
        let config = match modbusprotocol {
            ModbusProtocol::Rtu => {
                if config_u64 < u32::MAX as u64 {
                    config_u64 as u32
                } else {
                    panic!("{}", invailed_value_message!("baudrate", config_u64));
                }
            }
            ModbusProtocol::Tcp => {
                if config_u64 < u16::MAX as u64 {
                    config_u64 as u32
                } else {
                    panic!("{}", invailed_value_message!("tcp_port", config_u64));
                }
            }
        };

        let mut interface = Interface{
            modbusprotocol: modbusprotocol,
            address: address.clone(),
            config: config,
            slaves: HashMap::new(),
        };

        let slaves = yaml_config.get("slaves")
            .expect(missing_required_message!("slaves"))
            .as_sequence()
            .expect(invailed_type_message!("slaves", "sequence"));
        for slavedata in slaves {
            let slave_info_map = slavedata.as_mapping()
                .expect(invailed_type_message!("slavedata", "mapping"));
            if slave_info_map.len() != 1 {
                panic!("Invaild slavedata format");
            }
            for (_slave_name, _slave_info) in slave_info_map {

                let slave_name = String::from(_slave_name.as_str()
                    .expect(invailed_type_message!("slavedata name", "string"))
                );
                let slave_info = _slave_info.as_mapping()
                    .expect(invailed_type_message!("slavedata info", "mapping"));
                
                let key_id = Value::String(String::from("id"));
                let id_u64 = slave_info.get(&key_id)
                    .expect(missing_required_message!("id"))
                    .as_u64()
                    .expect(invailed_type_message!("id", "unsigned integetr"));
                let id;
                if id_u64 < u8::MAX as u64 {
                    id = id_u64 as u8;
                } else {
                    panic!("Invaild value of id '{}'", id_u64);
                }

                let (co_key, di_key, hr_key, ir_key) = (
                    Value::String(String::from("co")),
                    Value::String(String::from("di")),
                    Value::String(String::from("hr")),
                    Value::String(String::from("ir")),
                );
                let (co_list, di_list, hr_list, ir_list) = (
                    get_modbus_block_value!(slave_info, co_key),
                    get_modbus_block_value!(slave_info, di_key),
                    get_modbus_block_value!(slave_info, hr_key),
                    get_modbus_block_value!(slave_info, ir_key),
                );
                let (mut co, mut di, mut hr, mut ir) = (
                    HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new()
                );
                match co_list {
                    Some(list) => load_data_block(BlockType::Co, list, &mut co),
                    None => {},
                }
                match di_list {
                    Some(list) => load_data_block(BlockType::Di, list, &mut di),
                    None => {},
                }
                match hr_list {
                    Some(list) => load_data_block(BlockType::Hr, list, &mut hr),
                    None => {},
                }
                match ir_list {
                    Some(list) => load_data_block(BlockType::Ir, list, &mut ir),
                    None => {},
                }

                interface.slaves.insert(slave_name, SlaveData::new(id, co, di, hr, ir));

            }
        }

        interface
    
    }

}


impl fmt::Display for Interface {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        let (protocol_name, config_key) = match self.modbusprotocol {
            ModbusProtocol::Rtu => ("rtu", "baudrate"),
            ModbusProtocol::Tcp => ("tcp", "tcp_port"),
        };

        let mut slaves_info = String::new();
        for (slave_name, slave_info) in &self.slaves {
            slaves_info.push_str(format!("  {}: {}", slave_name, slave_info.id).as_str());
            slaves_info.push_str(format!("\n    co: {}", slave_info.co.len()).as_str());
            slaves_info.push_str(format!("\n    di: {}", slave_info.di.len()).as_str());
            slaves_info.push_str(format!("\n    hr: {}", slave_info.hr.len()).as_str());
            slaves_info.push_str(format!("\n    ir: {}", slave_info.ir.len()).as_str());
            slaves_info.push('\n');
        }
        
        write!(f, "{}\n{}\n{}\nslaves:\n{}",
            format!("modbusprotocol: {}", protocol_name),
            format!("address: {}", self.address),
            format!("{}: {}", config_key, self.config),
            slaves_info,
        )

    }

}

impl fmt::Display for ValueType {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        match self {
            ValueType::Bool => {
                write!(f, "Bool")
            },
            ValueType::U16 => {
                write!(f, "U16")
            },
            ValueType::I16 => {
                write!(f, "I16")
            },
            ValueType::U32 => {
                write!(f, "U32")
            },
            ValueType::I32 => {
                write!(f, "I32")
            },
            ValueType::F32 => {
                write!(f, "F32")
            },
        }

    }

}

impl fmt::Display for BlockType {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        match self {
            &BlockType::Co => {
                write!(f, "Co")
            },
            &BlockType::Di => {
                write!(f, "Di")
            },
            &BlockType::Hr => {
                write!(f, "Hr")
            },
            &BlockType::Ir => {
                write!(f, "Ir")
            },
        }

    }

}