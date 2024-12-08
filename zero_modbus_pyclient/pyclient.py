import zmq
from json import loads
from uuid import uuid4 as gen_uuid


class ZeroModbusError(RuntimeError):
    def __init__(self, *args):
        RuntimeError.__init__(self, *args)


class ZmbClient:

    def __init__(self, address:str) -> None:
        self.__socket = zmq.Context().socket(zmq.REQ)
        self.__socket.connect(address)
    
    def __do_request(self, body:dict) -> dict|None:
        exception = None
        key, value = 'ERROR', 'ZMQ ERROR'
        try:
            self.__socket.send_json(body)
            recv = self.__socket.recv().decode('ASCII')
            response:dict = loads(str(recv))
            assert type(response) == dict
            assert len(response.keys()) == 1
            key, value = response.popitem()
            assert key in ('ERROR', 'TEST', 'GET', 'SET')
        except Exception as e:
            exception = ZeroModbusError("INVAILED RESPONSE", e)
        if exception: raise exception
        if key == 'ERROR':
            raise ZeroModbusError(response[key])
        return value

    def test(self) -> bool:
        uuid = str(gen_uuid())
        return uuid == self.__do_request({ 'TEST': uuid })

    def get(self, paths:list) -> None:
        return self.__do_request({ 'GET': paths })

    def set(self, pairs:dict) -> None:
        return self.__do_request({ 'SET': pairs })