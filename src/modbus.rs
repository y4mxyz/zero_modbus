use log::*;
use core::fmt;
use tokio_modbus::client::{rtu, tcp, Context, Reader, Writer};
use tokio_modbus::slave::{SlaveContext, Slave};
use tokio_modbus::ExceptionCode;
use tokio_serial::{self, SerialStream};
use serde_json::{self, Number, Value};

use crate::interface::{BlockType, RequestFunction, Interface, ModbusData, ModbusProtocol, ValueType};


pub enum ModbusError {
    ModbusError(String),
    ModbusException(ExceptionCode),
    DataSizeNotMatch(usize),
    DataConvertError(ValueType),
    SlaveNotFound(String),
    ValueNotDefined(String),
    WriteInputValue(BlockType),
    InvailedValueInput(Value),
}


#[derive(Clone, Copy, PartialEq)]
pub enum GetOrSet {
    Get,
    Set,
}

enum ModbusFunction {
    ReadCoils,
    ReadDiscreteInputs,
    ReadHodingRegisters,
    ReadInputRegisters,
    WriteSingleCoil,
    WriteSingleRegister,
    WriteMultipleCoils,
    WriteMultipleRegisters,
}


fn response_to_value(response: &Vec<u16>, value_type: ValueType) -> Result<Value, ModbusError> {

    match value_type {
        ValueType::Bool => {
            if response.len() == 1 {
                Ok(Value::Bool(response[0] != 0))
            } else {
                Err(ModbusError::DataSizeNotMatch(response.len()))
            }
        },
        ValueType::U16 => {
            if response.len() == 1 {
                Ok(Value::Number(match Number::from_u128(response[0] as u128) {
                    Some(number) => number, None =>
                        return Err(ModbusError::DataConvertError(ValueType::U16)),
                }))
            } else {
                Err(ModbusError::DataSizeNotMatch(response.len()))
            }
        },
        ValueType::I16 => {
            if response.len() == 1 {
                Ok(Value::Number(match Number::from_i128(response[0] as i16 as i128) {
                    Some(number) => number, None =>
                        return Err(ModbusError::DataConvertError(ValueType::I16)),
                }))
            } else {
                Err(ModbusError::DataSizeNotMatch(response.len()))
            }
        },
        ValueType::U32 => {
            if response.len() == 2 {
                let num_u32 = ((response[0] as u32) << 16) | (response[1] as u32);
                Ok(Value::Number(match Number::from_u128(num_u32 as u128) {
                    Some(number) => number, None =>
                        return Err(ModbusError::DataConvertError(ValueType::U32)),
                }))
            } else {
                Err(ModbusError::DataSizeNotMatch(response.len()))
            }
        },
        ValueType::I32 => {
            if response.len() == 2 {
                let num_i32 = ((response[0] as i32) << 16) | (response[1] as i32);
                Ok(Value::Number(match Number::from_u128(num_i32 as i32 as u128) {
                    Some(number) => number, None =>
                        return Err(ModbusError::DataConvertError(ValueType::I32)),
                }))
            } else {
                Err(ModbusError::DataSizeNotMatch(response.len()))
            }
        },
        ValueType::F32 => {
            if response.len() == 2 {
                let num_f32 = f32::from_bits(((response[0] as u32) << 16) | (response[1] as u32));
                Ok(Value::Number(match Number::from_f64(num_f32 as f64) {
                    Some(number) => number, None =>
                        return Err(ModbusError::DataConvertError(ValueType::F32)),
                }))
            } else {
                Err(ModbusError::DataSizeNotMatch(response.len()))
            }
        },
    }
}

fn value_to_bytes(_value: &Option<Value>, value_type: ValueType, count: u16) -> Option<[u16; 2]> {

    let value = match _value {
        Some(value) => value,
        None => return None,
    };

    match count {
        1 => {
            match value_type {
                ValueType::Bool => {
                    if value.as_bool()? {
                        Some([0, 1])
                    } else {
                       Some([0, 0])
                    }
                },
                ValueType::U16 => {
                    let num_u64 = value.as_u64()?;
                    if num_u64 > u16::MAX as u64 {
                        None
                    } else {
                        Some([0, num_u64 as u16])
                    }
                },
                ValueType::I16 => {
                    let num_i64 = value.as_i64()?;
                    let num_u64 = num_i64.abs() as u64;
                    if num_u64 > i16::MAX.abs() as u64 {
                        None
                    } else {
                        Some([0, num_i64 as u16])
                    }
                },
                _ => None
            }
        },
        2 => {
            match value_type {
                ValueType::Bool => {
                    if value.as_bool()? {
                        Some([0, 1])
                    } else {
                       Some([0, 0])
                    }
                },
                ValueType::U16 => {
                    let num_u64 = value.as_u64()?;
                    if num_u64 > u16::MAX as u64 {
                        None
                    } else {
                        Some([0, num_u64 as u16])
                    }
                },
                ValueType::I16 => {
                    let num_i64 = value.as_i64()?;
                    let num_u64 = num_i64.abs() as u64;
                    if num_u64 > i16::MAX.abs() as u64 {
                        None
                    } else {
                        Some([0, num_i64 as u16])
                    }
                },
                ValueType::U32 => {
                    let num_u64 = value.as_u64()?;
                    if num_u64 > u32::MAX as u64 {
                        None
                    } else {
                        Some([((num_u64 & 0xFFFF0000) >> 16) as u16, (num_u64 & 0xFFFF) as u16])
                    }
                },
                ValueType::I32 => {
                    let num_i64 = value.as_i64()?;
                    let num_u64 = num_i64.abs() as u64;
                    if num_u64 > i32::MAX.abs() as u64 {
                        None
                    } else {
                        Some([
                            ((num_i64 as u64 & 0xFFFF0000) >> 16) as u16,
                            (num_i64 as u64 & 0xFFFF) as u16]
                        )
                    }
                },
                ValueType::F32 => {
                    let num_f64 = value.as_f64()?;
                    let num_i64 = num_f64.ceil() as i64;
                    let num_u64 = num_i64.abs() as u64;
                    if num_u64 > i32::MAX.abs() as u64 {
                        None
                    } else {
                        Some([
                            (((num_f64 as f32).to_bits() as u32 & 0xFFFF0000) >> 16) as u16,
                            ((num_f64 as f32).to_bits() as u32 & 0xFFFF) as u16]
                        )
                    }
                },
            }
        }
        _ => None
    }

}

impl ModbusFunction {
    
    pub fn inference(modbus_data: &ModbusData, get_or_set: GetOrSet) -> Option<(Self, u16)> {

        let modbus_function = match modbus_data.block_type() {
            BlockType::Co => {
                match get_or_set {
                    GetOrSet::Get => Some(ModbusFunction::ReadCoils),
                    GetOrSet::Set => match modbus_data.requestfunction() {
                        RequestFunction::Single => Some(ModbusFunction::WriteSingleCoil),
                        RequestFunction::Multiple => Some(ModbusFunction::WriteMultipleCoils),
                    }
                }
            }
            BlockType::Di => {
                match get_or_set {
                    GetOrSet::Get => Some(ModbusFunction::ReadDiscreteInputs),
                    _ => None
                }
            }
            BlockType::Hr => {
                match get_or_set {
                    GetOrSet::Get => Some(ModbusFunction::ReadHodingRegisters),
                    GetOrSet::Set => match modbus_data.requestfunction() {
                        RequestFunction::Single => Some(ModbusFunction::WriteSingleRegister),
                        RequestFunction::Multiple => Some(ModbusFunction::WriteMultipleRegisters),
                    }
                }
            }
            BlockType::Ir => {
                match get_or_set {
                    GetOrSet::Get => Some(ModbusFunction::ReadInputRegisters),
                    _ => None
                }
            }
        }?;

        let access_size = match modbus_data.value_type() {
            ValueType::Bool | ValueType::U16 | ValueType::I16 => 1,
            ValueType::U32 | ValueType::I32 | ValueType::F32 => 2,
        };

        Some((modbus_function, access_size))
        

    }

    pub async fn do_request(&self, context: &mut Context, address: u8, access_size: u16, value_type: ValueType, value: &Option<Value>) -> Result<Value, ModbusError> {
        
        match self {
            Self::ReadCoils => {
                match context.read_coils(address as u16, access_size).await {
                    Ok(modbus_response) => {
                        match modbus_response {
                            Ok(response) => {
                                if response.len() == 1 {
                                    Ok(Value::Bool(response[0]))
                                } else {
                                    Err(ModbusError::DataSizeNotMatch(response.len()))
                                }
                            },
                            Err(err) => Err(ModbusError::ModbusException(err)),
                        }
                    } Err(err) => {
                        Err(ModbusError::ModbusError(err.to_string()))
                    }
                }
            },
            Self::ReadDiscreteInputs => {
                match context.read_discrete_inputs(address as u16, access_size).await {
                    Ok(modbus_response) => {
                        match modbus_response {
                            Ok(response) => {
                                if response.len() == 1 {
                                    Ok(Value::Bool(response[0]))
                                } else {
                                    Err(ModbusError::DataSizeNotMatch(response.len()))
                                }
                            },
                            Err(err) => Err(ModbusError::ModbusException(err)),
                        }
                    } Err(err) => {
                        Err(ModbusError::ModbusError(err.to_string()))
                    }
                }
            },
            Self::ReadHodingRegisters => {
                match context.read_holding_registers(address as u16, access_size).await {
                    Ok(modbus_response) => {
                        match modbus_response {
                            Ok(response) => response_to_value(&response, value_type),
                            Err(err) => Err(ModbusError::ModbusException(err)),
                        }
                    } Err(err) => Err(ModbusError::ModbusError(err.to_string())),
                }
            },
            Self::ReadInputRegisters => {
                match context.read_input_registers(address as u16, access_size).await {
                    Ok(modbus_response) => {
                        match modbus_response {
                            Ok(response) => response_to_value(&response, value_type),
                            Err(err) => Err(ModbusError::ModbusException(err)),
                        }
                    } Err(err) => Err(ModbusError::ModbusError(err.to_string())),
                }
            },
            Self::WriteSingleCoil => {
                match context.write_single_coil(address as u16, match value {
                    Some(value) => match value.as_bool() {
                        Some(coil) => coil,
                        None => { return Err(ModbusError::InvailedValueInput(value.clone())); },
                    },
                    None => { return Err(ModbusError::InvailedValueInput(Value::Null)); }
                }).await {
                    Ok(modbus_response) => {
                        match modbus_response {
                            Ok(_) => Ok(Value::Null),
                            Err(err) => Err(ModbusError::ModbusException(err)),
                        }
                    } Err(err) => Err(ModbusError::ModbusError(err.to_string())),
                }
            },
            Self::WriteMultipleCoils => {
                let mut coil_array = [false];
                match context.write_multiple_coils(address as u16, match value {
                    Some(value) => match value.as_bool() {
                        Some(coil) => {
                            coil_array[0] = coil;
                            &coil_array
                        },
                        None => { return Err(ModbusError::InvailedValueInput(value.clone())); },
                    },
                    None => { return Err(ModbusError::InvailedValueInput(Value::Null)); }
                }).await {
                    Ok(modbus_response) => {
                        match modbus_response {
                            Ok(_) => Ok(Value::Null),
                            Err(err) => Err(ModbusError::ModbusException(err)),
                        }
                    } Err(err) => Err(ModbusError::ModbusError(err.to_string())),
                }
            },
            Self::WriteSingleRegister => {
                let words = match value_to_bytes(value, value_type, access_size) {
                    Some(words) => words,
                    None => return Err(ModbusError::InvailedValueInput(Value::Null)),
                };
                match access_size {
                    1 => match context.write_single_register(address as u16, words[1]).await {
                        Ok(modbus_response) => {
                            match modbus_response {
                                Ok(_) => Ok(Value::Null),
                                Err(err) => Err(ModbusError::ModbusException(err)),
                            }
                        } Err(err) => Err(ModbusError::ModbusError(err.to_string())),
                    },
                    2 => {
                        match context.write_single_register(address as u16, words[0]).await {
                            Ok(modbus_response) => {
                                match modbus_response {
                                    Ok(_) => {},
                                    Err(err) => return Err(ModbusError::ModbusException(err)),
                                }
                            }, Err(err) => return Err(ModbusError::ModbusError(err.to_string())),
                        }
                        match context.write_single_register((address+1) as u16, words[1]).await {
                            Ok(modbus_response) => {
                                match modbus_response {
                                    Ok(_) => Ok(Value::Null),
                                    Err(err) => Err(ModbusError::ModbusException(err)),
                                }
                            } Err(err) => Err(ModbusError::ModbusError(err.to_string())),
                        }
                    },
                    _ => panic!("Access size not match"),
                }
            },
            Self::WriteMultipleRegisters => {
                let words = match value_to_bytes(value, value_type, access_size) {
                    Some(words) => words,
                    None => return Err(ModbusError::InvailedValueInput(match value {
                        Some(value) => value.clone(), None => Value::Null,
                    })),
                };
                let single_word = [words[1]];
                match context.write_multiple_registers(address as u16, match access_size {
                    1 => &single_word,
                    2 => &words,
                    _ => return Err(ModbusError::InvailedValueInput(match value {
                        Some(value) => value.clone(), None => Value::Null,
                    })),
                }).await {
                    Ok(modbus_response) => {
                        match modbus_response {
                            Ok(_) => Ok(Value::Null),
                            Err(err) => Err(ModbusError::ModbusException(err)),
                        }
                    } Err(err) => Err(ModbusError::ModbusError(err.to_string())),
                }
            },
        }
        
    }

}

async fn build_rtu_session(serial_port: String, baudrate: u32) -> Result<Context, String> {

    let builder = tokio_serial::new(&serial_port, baudrate)
        .parity(tokio_serial::Parity::None)
        .stop_bits(tokio_serial::StopBits::One)
        .data_bits(tokio_serial::DataBits::Eight)
        .timeout(std::time::Duration::from_millis(1000));

    let serial: SerialStream = match SerialStream::open(&builder) {
        Ok(serial) => serial,
        Err(e) => return Err(format!("Failed to open {:?}: {:?}", serial_port, e)),
    };

    Ok(rtu::attach(serial))

}

async fn build_tcp_session(host_addr: String, port: u32) -> Result<Context, String> {

    let mut addr = String::from(host_addr);
    addr.push_str(":");
    addr.push_str(&format!("{}", port));
    
    let socket_addr = match addr.parse() {
        Ok(socket_addr) => socket_addr,
        Err(e) => return Err(format!("Failed to parse socket-addr: {:?}: {:?}", addr, e)),
    };

    match tcp::connect(socket_addr).await {
        Ok(socket) => Ok(socket),
        Err(e) => Err(format!("Failed to connect to {:?}: {:?}", addr, e)),
    }

}


pub async fn batch_request(interface: Interface, request_info: Vec<(String, (String, Option<Value>))>, get_or_set: GetOrSet) -> Result<Vec<(String, Value)>, ModbusError> {

    let mut context = match interface.modbusprotocol() {
        ModbusProtocol::Rtu => {
            match build_rtu_session(interface.address(), interface.config()).await {
                Ok(context) => context, Err(info) => {
                    let msg = format!("Failed to create rtu session: {}", info);
                    error!("ModbusError: {}", msg);
                    return Err(ModbusError::ModbusError(msg));
                }
            }
        },
        ModbusProtocol::Tcp => {
            match build_tcp_session(interface.address(), interface.config()).await {
                Ok(context) => context, Err(info) => {
                    let msg = format!("Failed to create tcp session: {}", info);
                    error!("ModbusError: {}", msg);
                    return Err(ModbusError::ModbusError(msg));
                }
            }
        },
    };

    let mut results = Vec::new();

    for (slave_name, (value_name, value)) in &request_info {
        
        let slave = match interface.slaves.get(slave_name) {
            Some(slave) => slave, None => {
                warn!("SlaveNotFound: {}", slave_name);
                return Err(ModbusError::SlaveNotFound(slave_name.to_string()));
            }
        };
        let modbus_data = match slave.find(&value_name) {
            Some(modbus_data) => modbus_data, None => {
                let info = format!("{} in {}", value_name, slave_name);
                warn!("DataNotFound: {}", info);
                return Err(ModbusError::ValueNotDefined(info));
            }
        };
        context.set_slave(Slave(slave.id()));
        
        let (modbus_function, access_size) = match ModbusFunction::inference(&modbus_data, get_or_set) {
            Some(pair) => pair, None => {
                warn!("WriteInputValue: {}", modbus_data.block_type());
                return Err(ModbusError::WriteInputValue(modbus_data.block_type()));
            }
        };
        
        match modbus_function.do_request(&mut context, modbus_data.address(), access_size, modbus_data.value_type(), value).await {
            Ok(response) => {
                if get_or_set == GetOrSet::Get {
                    results.push((value_name.clone(), response));
                }
            },
            Err(modbus_error) => {
                warn!("modbus error: {}", modbus_error);
                return Err(modbus_error);
            },
        }
        
    }
    
    Ok(results)

}

impl fmt::Display for ModbusError {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        match self {
            ModbusError::ModbusError(info) => {
                write!(f, "ModbusError: {}", info)
            },
            ModbusError::ModbusException(info) => {
                write!(f, "ModbusException: {}", info)
            },
            ModbusError::DataSizeNotMatch(info) => {
                write!(f, "DataSizeNotMatch: {}", info)
            },
            ModbusError::DataConvertError(info) => {
                write!(f, "DataConvertError: {}", info)
            },
            ModbusError::SlaveNotFound(info) => {
                write!(f, "SlaveNotFound: {}", info)
            },
            ModbusError::ValueNotDefined(info) => {
                write!(f, "ValueNotDefined: {}", info)
            },
            ModbusError::WriteInputValue(info) => {
                write!(f, "WriteInputValue: {}", info)
            },
            ModbusError::InvailedValueInput(info) => {
                write!(f, "InvailedValueInput: {}", info)
            },
        }

    }
    
}