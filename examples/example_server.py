#!/usr/bin/python3


from struct import pack, unpack

from pymodbus.server import StartTcpServer
from pymodbus.device import ModbusDeviceIdentification
from pymodbus.datastore import ModbusSequentialDataBlock
from pymodbus.datastore import ModbusSlaveContext, ModbusServerContext

import logging

logging.basicConfig()
log = logging.getLogger()
log.setLevel(logging.DEBUG)

store = ModbusSlaveContext(
    co=ModbusSequentialDataBlock(0, [True]*10),
    di=ModbusSequentialDataBlock(0, [False]*10),
    hr=ModbusSequentialDataBlock(0, list(range(0, 10))),
    ir=ModbusSequentialDataBlock(0, [0, 1, 2, 3] + list(unpack('>HH', pack('>f', 1.23))) + [6, 7, 8, 9]),
)

context = ModbusServerContext(slaves=store, single=True)

StartTcpServer(context=context, identity=ModbusDeviceIdentification(), address=("localhost", 5020))