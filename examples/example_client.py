#!/usr/bin/python3


from zero_modbus_pyclient.pyclient import ZmbClient


client = ZmbClient('ipc:///tmp/zero_modbus_test.socket')
print(client.test())
result = client.get([
    '/example_modbus_tcp/tcp_slave_1/status_value_a',
    '/example_modbus_tcp/tcp_slave_2/status_value_b',
    '/example_modbus_tcp/tcp_slave_2/holding_value_b',
])
print(result)
result = client.set({
    '/example_modbus_tcp/tcp_slave_1/status_value_a': False,
    '/example_modbus_tcp/tcp_slave_2/status_value_b': False,
    '/example_modbus_tcp/tcp_slave_2/holding_value_b': 123,
})
print(result)
result = client.get([
    '/example_modbus_tcp/tcp_slave_1/status_value_a',
    '/example_modbus_tcp/tcp_slave_2/status_value_b',
    '/example_modbus_tcp/tcp_slave_2/holding_value_b',
])
print(result)